use std::error::Error;
use std::fmt;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use cronrunner::crontab::Crontab;
use cronrunner::parser::{Kind, Parser};
use cronrunner::tokens::{CronJob, Token};

use super::job::Job;

const SYSTEM_CRONTAB_DIRECTORY: &str = "/etc/cron.d";

/// File source for a crontab.
///
/// This encodes the kind (normal crontab vs. system crontab). `kind`
/// is private, so the only way to instantiate it is through the
/// explicit [`InputFile::from_crontab()`] and
/// [`InputFile::from_system()`] methods, making it hard to misuse.
#[derive(Debug, Eq, PartialEq)]
pub struct InputFile {
    /// Whether it's a normal crontab or a system crontab.
    kind: Kind,
    /// The path to the file. It _must_ be private, otherwise it could
    /// make `kind` lie if it was mutable.
    path: PathBuf,
}

impl InputFile {
    /// Create and instance from a regular crontab file.
    pub fn from_crontab(path: PathBuf) -> Self {
        Self {
            kind: Kind::User,
            path,
        }
    }

    /// Create and instance from a system crontab file.
    pub fn from_system(path: PathBuf) -> Self {
        Self {
            kind: Kind::System,
            path,
        }
    }
}

impl InputFile {
    /// The path to the file.
    ///
    /// We only allow non-mutable references because if we allowed
    /// mutation, it could make `kind` lie if the underlying changes.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

/// A normalized crontab file source that knows its kind.
///
/// It's the counterpart of [`InputFile`] once read and validated.
#[derive(Debug)]
struct CrontabFile {
    /// Whether it's a normal crontab or a system crontab.
    kind: Kind,
    /// Path as given by user, used in error messages.
    path: PathBuf,
    /// Canonicalized path, used as document identifier and for dedup.
    canonical_path: PathBuf,
    /// Contents of the file, used by the parser.
    contents: String,
}

impl CrontabFile {
    /// Derive document identifier (bytes) from canonical path.
    fn document_id(&self) -> &[u8] {
        self.canonical_path.as_os_str().as_bytes()
    }

    /// Parse a crontab file into a [`Crontab`].
    ///
    /// This method abstracts away the parsing split on kind, it knows
    /// how to parse itself.
    fn parse(&self) -> Crontab {
        let document_id = self.document_id();
        let tokens: Vec<Token> = match self.kind {
            Kind::User => Parser::parse_with_document_id(&self.contents, document_id),
            Kind::System => Parser::parse_system_with_document_id(&self.contents, document_id),
        };
        Crontab::new(tokens)
    }
}

impl TryFrom<&InputFile> for CrontabFile {
    type Error = std::io::Error;

    /// Convert an [`InputFile`] into a [`CrontabFile`].
    ///
    /// Same idea as [`CrontabFile::parse()`]: [`CrontabFile`] knows how
    /// to build itself from [`InputFile`], and it keeps the `kind`
    /// shenanigans encapsulated.
    fn try_from(file: &InputFile) -> Result<Self, Self::Error> {
        let path = file.path();
        let canonical_path = std::fs::canonicalize(path)?;
        let contents = std::fs::read_to_string(&canonical_path)?;

        Ok(Self {
            kind: file.kind,
            path: path.clone(),
            canonical_path,
            contents,
        })
    }
}

// TODO: We should align those with `crontab::ReadError`.
#[derive(Debug)]
pub enum CrontabSourcesError {
    DirectoryRead { path: PathBuf, source: io::Error },
    FileRead { path: PathBuf, source: io::Error },
    DuplicateFile { path: PathBuf, first_path: PathBuf },
}

impl fmt::Display for CrontabSourcesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectoryRead { path, source } => write!(
                f,
                "Cannot read system crontab directory '{}': {source}",
                path.display()
            ),
            Self::FileRead { path, source } => {
                write!(f, "Cannot read crontab file '{}': {source}", path.display())
            }
            Self::DuplicateFile { path, first_path } => write!(
                f,
                "Crontab file '{}' refers to the same document as '{}'",
                path.display(),
                first_path.display()
            ),
        }
    }
}

impl Error for CrontabSourcesError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::DirectoryRead { source, .. } => Some(source),
            Self::FileRead { source, .. } => Some(source),
            Self::DuplicateFile { .. } => None,
        }
    }
}

/// Abstraction over a set of [`Crontab`]s.
///
/// We allow sourcing crontabs from multiple files; we don't work with
/// one [`Crontab`], but with multiple. [`CrontabSources`] lets us work
/// with this set of crontabs as if we were working with one,
/// abstracting all the `.iter()`s away.
///
/// [`CrontabSources`] is constructed from a set of [`InputFile`]s. And
/// uses its own file representation under the hood ([`CrontabFile`]).
#[derive(Debug)]
pub struct CrontabSources {
    sources: Vec<Crontab>,
}

impl CrontabSources {
    pub fn prepend(&mut self, source: Crontab) {
        let mut sources = Vec::with_capacity(self.sources.len() + 1);
        sources.push(source);
        sources.append(&mut self.sources);
        *self = sources.into();
    }

    pub fn has_runnable_jobs(&self) -> bool {
        self.sources.iter().any(Crontab::has_runnable_jobs)
    }

    pub fn documents(&self) -> &[Crontab] {
        &self.sources
    }

    pub fn jobs(&self) -> Vec<&CronJob> {
        self.sources.iter().flat_map(Crontab::jobs).collect()
    }

    pub fn select(&mut self, selection: &Job) -> Option<(&mut Crontab, CronJob)> {
        self.sources.iter_mut().find_map(|source| {
            let job = match selection {
                Job::Uid(uid) => source.get_job_from_uid(*uid),
                Job::Fingerprint(fingerprint) => source.get_job_from_fingerprint(*fingerprint),
                Job::Tag(tag) => source.get_job_from_tag(tag),
            }
            .cloned()?;

            Some((source, job))
        })
    }

    pub fn to_json(&self) -> String {
        // Aggregate all tokens from all crontabs...
        let tokens = self
            .sources
            .iter()
            .flat_map(|source| source.tokens.iter().cloned())
            .collect();
        // ...into a synthetic unique `Crontab` we can export.
        Crontab::new(tokens).to_json()
    }
}

pub fn system_crontab_files() -> Result<Vec<InputFile>, CrontabSourcesError> {
    system_crontab_files_from(SYSTEM_CRONTAB_DIRECTORY.into())
}

impl TryFrom<&[InputFile]> for CrontabSources {
    type Error = CrontabSourcesError;

    /// Create an instance from a list of [`InputFile`]s.
    fn try_from(files: &[InputFile]) -> Result<Self, Self::Error> {
        let files: Vec<CrontabFile> = try_read_files(files)?;
        if let Some(error) = find_duplicate_files(&files) {
            return Err(error);
        }
        let crontabs: Vec<Crontab> = files.iter().map(CrontabFile::parse).collect();
        Ok(crontabs.into())
    }
}

fn try_read_files(files: &[InputFile]) -> Result<Vec<CrontabFile>, CrontabSourcesError> {
    files
        .iter()
        .map(|file| {
            CrontabFile::try_from(file).map_err(|source| CrontabSourcesError::FileRead {
                path: file.path.clone(),
                source,
            })
        })
        .collect()
}

fn system_crontab_files_from(directory: PathBuf) -> Result<Vec<InputFile>, CrontabSourcesError> {
    let entries =
        std::fs::read_dir(&directory).map_err(|source| CrontabSourcesError::DirectoryRead {
            path: directory.clone(),
            source,
        })?;
    let mut paths = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|source| CrontabSourcesError::DirectoryRead {
            path: directory.clone(),
            source,
        })?;
        if !is_system_crontab_filename(&entry.file_name()) {
            continue;
        }

        let path = entry.path();
        let metadata =
            std::fs::metadata(&path).map_err(|source| CrontabSourcesError::FileRead {
                path: path.clone(),
                source,
            })?;
        if metadata.is_file() {
            paths.push(path);
        }
    }

    paths.sort();
    Ok(paths.into_iter().map(InputFile::from_system).collect())
}

fn is_system_crontab_filename(filename: &std::ffi::OsStr) -> bool {
    let Some(filename) = filename.to_str() else {
        return false;
    };
    !filename.is_empty()
        && filename
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn find_duplicate_files(files: &[CrontabFile]) -> Option<CrontabSourcesError> {
    for (index, file) in files.iter().enumerate() {
        if let Some(first) = files[..index]
            .iter()
            .find(|first| first.canonical_path == file.canonical_path)
        {
            return Some(CrontabSourcesError::DuplicateFile {
                path: file.path.clone(),
                first_path: first.path.clone(),
            });
        }
    }
    None
}

impl From<Crontab> for CrontabSources {
    /// Create an instance from a single [`Crontab`] entry.
    fn from(crontab: Crontab) -> Self {
        Self {
            sources: vec![crontab],
        }
    }
}

impl From<Vec<Crontab>> for CrontabSources {
    /// Create an instance from a list of [`Crontab`] entries.
    fn from(mut sources: Vec<Crontab>) -> Self {
        let mut next_job_uid = 1;
        let mut section_uid_offset = 0;

        for source in &mut sources {
            let max_local_section_uid = source
                .tokens
                .iter()
                .filter_map(|token| match token {
                    Token::CronJob(job) => job.section.as_ref(),
                    Token::IgnoredJob(job) => job.section.as_ref(),
                    _ => None,
                })
                .map(|section| section.uid)
                .max()
                .unwrap_or(0);

            for token in &mut source.tokens {
                let section = match token {
                    Token::CronJob(job) => {
                        job.uid = next_job_uid;
                        next_job_uid += 1;
                        job.section.as_mut()
                    }
                    Token::IgnoredJob(job) => job.section.as_mut(),
                    _ => None,
                };

                if let Some(section) = section {
                    section.uid += section_uid_offset;
                }
            }

            section_uid_offset += max_local_section_uid;
        }

        Self { sources }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use cronrunner::parser::{Kind, Parser};
    use cronrunner::tokens::{CronJob, JobSection};

    use super::*;

    fn fresh_system_directory(name: &str) -> PathBuf {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/tmp/sources_tests")
            .join(name);
        if directory.exists() {
            std::fs::remove_dir_all(&directory).unwrap();
        }
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn file_paths_are_canonicalized() {
        let relative = PathBuf::from("tests/fixtures/crontab_file_one.cron");
        let absolute = std::fs::canonicalize(&relative).unwrap();
        let temporary_directory =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/tmp/sources_tests");
        std::fs::create_dir_all(&temporary_directory).unwrap();
        let symlink = temporary_directory.join("crontab-file-one-link.cron");
        if symlink.exists() {
            std::fs::remove_file(&symlink).unwrap();
        }
        std::os::unix::fs::symlink(&absolute, &symlink).unwrap();

        let files = try_read_files(&[
            InputFile::from_crontab(relative),
            InputFile::from_crontab(absolute.clone()),
            InputFile::from_crontab(symlink),
        ])
        .unwrap();

        assert_eq!(files[0].canonical_path, absolute);
        assert_eq!(files[1].canonical_path, files[0].canonical_path);
        assert_eq!(files[2].canonical_path, files[0].canonical_path);
    }

    #[test]
    fn duplicate_file_error_retains_both_supplied_paths() {
        let canonical_path = PathBuf::from("/tmp/example.cron");
        let files = [
            CrontabFile {
                kind: Kind::User,
                path: PathBuf::from("example.cron"),
                canonical_path: canonical_path.clone(),
                contents: String::new(),
            },
            CrontabFile {
                kind: Kind::User,
                path: PathBuf::from("./example.cron"),
                canonical_path,
                contents: String::new(),
            },
        ];

        let error = find_duplicate_files(&files).unwrap();

        let CrontabSourcesError::DuplicateFile { path, first_path } = &error else {
            panic!()
        };
        assert_eq!(path, &PathBuf::from("./example.cron"));
        assert_eq!(first_path, &PathBuf::from("example.cron"));
        assert_eq!(
            error.to_string(),
            "Crontab file './example.cron' refers to the same document as 'example.cron'"
        );
        assert!(error.source().is_none());
    }

    #[test]
    fn distinct_files_are_not_duplicates() {
        let files = [
            CrontabFile {
                kind: Kind::User,
                path: PathBuf::from("first.cron"),
                canonical_path: PathBuf::from("/tmp/first.cron"),
                contents: String::new(),
            },
            CrontabFile {
                kind: Kind::User,
                path: PathBuf::from("second.cron"),
                canonical_path: PathBuf::from("/tmp/second.cron"),
                contents: String::new(),
            },
        ];

        assert!(find_duplicate_files(&files).is_none());
    }

    #[test]
    fn file_read_error_retains_path_and_io_error() {
        let missing = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/tmp/sources_tests/does-not-exist.cron");

        let error = try_read_files(&[InputFile::from_crontab(missing.clone())]).unwrap_err();

        let CrontabSourcesError::FileRead { path, source } = &error else {
            panic!()
        };
        assert_eq!(path, &missing);
        assert_eq!(source.kind(), io::ErrorKind::NotFound);
        assert!(error.to_string().contains(&missing.display().to_string()));
        assert!(error.source().is_some());
    }

    #[test]
    fn file_reading_stops_at_the_first_error() {
        let first = PathBuf::from("first-missing.cron");
        let second = PathBuf::from("second-missing.cron");

        let error = try_read_files(&[
            InputFile::from_crontab(first.clone()),
            InputFile::from_crontab(second),
        ])
        .unwrap_err();

        let CrontabSourcesError::FileRead { path, .. } = error else {
            panic!()
        };
        assert_eq!(path, first);
    }

    #[test]
    fn file_reading_rejects_invalid_utf8() {
        let temporary_directory =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/tmp/sources_tests");
        std::fs::create_dir_all(&temporary_directory).unwrap();
        let invalid = temporary_directory.join("invalid-utf8.cron");
        std::fs::write(&invalid, [0xff]).unwrap();

        let error = try_read_files(&[InputFile::from_crontab(invalid.clone())]).unwrap_err();

        let CrontabSourcesError::FileRead { path, source } = error else {
            panic!()
        };
        assert_eq!(path, invalid);
        assert_eq!(source.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn system_directory_filters_and_sorts_crontab_files() {
        let directory = fresh_system_directory("system_directory_filters_and_sorts");
        std::fs::write(directory.join("z-last"), "@daily root echo last").unwrap();
        std::fs::write(directory.join("a_first"), "@daily root echo first").unwrap();
        std::fs::write(directory.join("ignored.cron"), "@daily root echo ignored").unwrap();
        std::fs::create_dir(directory.join("directory")).unwrap();
        let target = directory.join("target.backup");
        std::fs::write(&target, "@daily root echo linked").unwrap();
        std::os::unix::fs::symlink(&target, directory.join("linked-file")).unwrap();

        let files = system_crontab_files_from(directory.clone()).unwrap();

        assert_eq!(
            files.iter().map(InputFile::path).collect::<Vec<_>>(),
            [
                &directory.join("a_first"),
                &directory.join("linked-file"),
                &directory.join("z-last"),
            ]
        );
        assert!(files.iter().all(|file| file.kind == Kind::System));
    }

    #[test]
    fn missing_system_directory_retains_path_and_io_error() {
        let directory = fresh_system_directory("missing_system_directory");
        std::fs::remove_dir(&directory).unwrap();

        let error = system_crontab_files_from(directory.clone()).unwrap_err();

        let CrontabSourcesError::DirectoryRead { path, source } = &error else {
            panic!()
        };
        assert_eq!(path, &directory);
        assert_eq!(source.kind(), io::ErrorKind::NotFound);
        assert!(error.to_string().contains(&directory.display().to_string()));
        assert!(error.source().is_some());
    }

    #[test]
    fn discovered_system_files_use_system_parsing() {
        let directory = fresh_system_directory("discovered_system_files_use_system_parsing");
        std::fs::write(directory.join("example"), "@daily root echo system").unwrap();
        let files = system_crontab_files_from(directory).unwrap();

        let sources = CrontabSources::try_from(files.as_slice()).unwrap();

        assert_eq!(sources.jobs()[0].user.as_deref(), Some("root"));
        assert_eq!(sources.jobs()[0].command, "echo system");
    }

    #[test]
    fn discovered_and_explicit_canonical_duplicates_are_rejected() {
        let directory = fresh_system_directory("discovered_and_explicit_duplicates");
        let path = directory.join("example");
        std::fs::write(&path, "@daily root echo system").unwrap();
        let mut files = vec![InputFile::from_system(path.clone())];
        files.extend(system_crontab_files_from(directory).unwrap());

        let error = CrontabSources::try_from(files.as_slice()).unwrap_err();

        assert!(matches!(error, CrontabSourcesError::DuplicateFile { .. }));
    }

    #[test]
    fn live_crontab_metadata_is_not_normalized() {
        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 42,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@daily"),
            user: None,
            command: String::from(":"),
            description: None,
            section: Some(JobSection {
                uid: 7,
                title: String::from("Section"),
            }),
        })]);

        let sources = CrontabSources::from(crontab);
        let job = sources.jobs()[0];

        assert_eq!(job.uid, 42);
        assert_eq!(job.section.as_ref().unwrap().uid, 7);
    }

    #[test]
    fn documents_preserve_source_order_and_jobs() {
        let first = Crontab::new(Parser::parse("@daily echo first"));
        let second = Crontab::new(Parser::parse("@daily echo second"));
        let sources = CrontabSources::from(vec![first, second]);

        let documents = sources.documents();

        assert_eq!(documents.len(), 2);
        assert_eq!(documents[0].jobs()[0].command, "echo first");
        assert_eq!(documents[1].jobs()[0].command, "echo second");
    }

    #[test]
    fn prepending_a_source_reassigns_uids_without_changing_fingerprints() {
        let user = Crontab::new(Parser::parse("@daily echo user"));
        let file = Crontab::new(Parser::parse_with_document_id("@daily echo file", b"file"));
        let user_fingerprint = user.jobs()[0].fingerprint;
        let file_fingerprint = file.jobs()[0].fingerprint;
        let mut sources = CrontabSources::from(file);

        sources.prepend(user);

        assert_eq!(sources.documents().len(), 2);
        assert_eq!(sources.jobs()[0].command, "echo user");
        assert_eq!(sources.jobs()[0].uid, 1);
        assert_eq!(sources.jobs()[0].fingerprint, user_fingerprint);
        assert_eq!(sources.jobs()[1].command, "echo file");
        assert_eq!(sources.jobs()[1].uid, 2);
        assert_eq!(sources.jobs()[1].fingerprint, file_fingerprint);
    }

    #[test]
    fn file_crontabs_get_global_uids_without_changing_fingerprints() {
        let first = Crontab::new(Parser::parse_with_document_id(
            "@daily echo first",
            b"first",
        ));
        let second = Crontab::new(Parser::parse_with_document_id(
            "@daily echo second\n@daily echo third",
            b"second",
        ));
        let fingerprints = first
            .jobs()
            .into_iter()
            .chain(second.jobs())
            .map(|job| job.fingerprint)
            .collect::<Vec<_>>();

        let sources = CrontabSources::from(vec![first, second]);

        assert_eq!(
            sources.jobs().iter().map(|job| job.uid).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(
            sources
                .jobs()
                .iter()
                .map(|job| job.fingerprint)
                .collect::<Vec<_>>(),
            fingerprints
        );
    }

    #[test]
    fn reordering_file_crontabs_changes_global_uids_but_not_fingerprints() {
        let make_first = || {
            Crontab::new(Parser::parse_with_document_id(
                "@daily echo first",
                b"first",
            ))
        };
        let make_second = || {
            Crontab::new(Parser::parse_with_document_id(
                "@daily echo second",
                b"second",
            ))
        };

        let original = CrontabSources::from(vec![make_first(), make_second()]);
        let reordered = CrontabSources::from(vec![make_second(), make_first()]);
        let original_job = original
            .jobs()
            .into_iter()
            .find(|job| job.command == "echo first")
            .unwrap();
        let reordered_job = reordered
            .jobs()
            .into_iter()
            .find(|job| job.command == "echo first")
            .unwrap();

        assert_eq!(original_job.uid, 1);
        assert_eq!(reordered_job.uid, 2);
        assert_eq!(original_job.fingerprint, reordered_job.fingerprint);
    }

    #[test]
    fn file_crontab_sections_get_global_uids_including_ignored_jobs() {
        let first = Crontab::new(Parser::parse_with_document_id(
            "### Shared\n@daily echo first\n## %{ignore}\n@daily echo ignored",
            b"first",
        ));
        let second = Crontab::new(Parser::parse_with_document_id(
            "### Shared\n## %{ignore}\n@daily echo ignored\n@daily echo second",
            b"second",
        ));

        let sources = CrontabSources::from(vec![first, second]);
        let first_section = sources.sources[0].jobs()[0].section.as_ref().unwrap();
        let second_section = sources.sources[1].jobs()[0].section.as_ref().unwrap();
        let ignored_section = sources.sources[1]
            .tokens
            .iter()
            .find_map(|token| match token {
                Token::IgnoredJob(job) => job.section.as_ref(),
                _ => None,
            })
            .unwrap();

        assert_eq!(first_section.uid, 1);
        assert_eq!(second_section.uid, 2);
        assert_eq!(ignored_section.uid, 2);
        assert_ne!(first_section, second_section);
    }

    #[test]
    fn every_selector_returns_the_owning_crontab() {
        let first_tag = "first";
        let first_source = format!("## %{{{first_tag}}}\n@daily echo {first_tag}");
        let first = Crontab::new(Parser::parse_with_document_id(&first_source, b"first"));
        let second_tag = "second";
        let second_source = format!("## %{{{second_tag}}}\n@daily echo {second_tag}");
        let second = Crontab::new(Parser::parse_with_document_id(&second_source, b"second"));
        let second_fingerprint = second.jobs()[0].fingerprint;
        let mut sources = CrontabSources::from(vec![first, second]);

        for selection in [
            Job::Uid(2),
            Job::Fingerprint(second_fingerprint),
            Job::Tag(String::from("second")),
        ] {
            let (owner, job) = sources.select(&selection).unwrap();

            assert!(owner.has_job(&job));
            assert_eq!(job.command, "echo second");
        }
    }

    #[test]
    fn crontab_sources_report_whether_any_document_has_jobs() {
        let empty = CrontabSources::from(Vec::new());
        let jobless = CrontabSources::from(vec![Crontab::new(Parser::parse("FOO=bar\n# Comment"))]);
        let runnable = CrontabSources::from(vec![
            Crontab::new(Vec::new()),
            Crontab::new(Parser::parse("@daily :")),
        ]);

        assert!(!empty.has_runnable_jobs());
        assert!(!jobless.has_runnable_jobs());
        assert!(runnable.has_runnable_jobs());
    }

    #[test]
    fn multi_document_json_uses_global_uids_and_document_fingerprints() {
        let first = Crontab::new(Parser::parse_with_document_id(
            "@daily echo first",
            b"first",
        ));
        let second = Crontab::new(Parser::parse_with_document_id(
            "@daily echo second",
            b"second",
        ));
        let first_fingerprint = first.jobs()[0].fingerprint;
        let second_fingerprint = second.jobs()[0].fingerprint;
        let sources = CrontabSources::from(vec![first, second]);

        let json = sources.to_json();

        assert!(json.contains(&format!(r#""uid":1,"fingerprint":"{first_fingerprint:x}""#)));
        assert!(json.contains(&format!(
            r#""uid":2,"fingerprint":"{second_fingerprint:x}""#
        )));
    }

    #[test]
    fn file_crontab_variables_are_isolated_and_override_shared_environment() {
        let first = Crontab::new(Parser::parse_with_document_id(
            "VALUE=first\n@daily test \"$VALUE\" = first",
            b"first",
        ));
        let second = Crontab::new(Parser::parse_with_document_id(
            "@daily test \"$VALUE\" = shared",
            b"second",
        ));
        let mut sources = CrontabSources::from(vec![first, second]);
        let env = HashMap::from([
            (String::from("HOME"), String::from("/tmp")),
            (String::from("VALUE"), String::from("shared")),
        ]);

        let (first_owner, first_job) = sources.select(&Job::Uid(1)).unwrap();
        first_owner.set_env(env.clone());
        assert!(first_owner.run(&first_job).was_successful);

        let (second_owner, second_job) = sources.select(&Job::Uid(2)).unwrap();
        second_owner.set_env(env);
        assert!(second_owner.run(&second_job).was_successful);
    }
}
