use std::error::Error;
use std::fmt;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use cronrunner::crontab::Crontab;
use cronrunner::parser::{Kind, Parser};
use cronrunner::reader::{ReadError, Reader};
use cronrunner::tokens::{CronJob, Token};

use super::job::Job;

/// Source for a crontab.
#[derive(Debug, Eq, PartialEq)]
pub enum Source {
    /// User crontab (`crontab -l`).
    UserCrontab(UserCrontab),
    /// User-provided user-crontab file (`-f`/`--file`).
    UserFile(UserFile),
    /// System crontab (`/etc/cron.d`).
    SystemCrontab(SystemCrontab),
    /// User-provider system-crontab file (`-F`/`--system-file`).
    SystemFile(SystemFile),
}

impl Source {
    /// The current user's live crontab (`crontab -l`), the default.
    pub fn from_user_crontab() -> Self {
        Self::UserCrontab(UserCrontab)
    }

    /// A user crontab read from a file (`-f`/`--file`).
    pub fn from_user_file(path: PathBuf) -> Self {
        Self::UserFile(UserFile(path))
    }

    /// The system crontabs (`/etc/crontab` and `/etc/cron.d/*`).
    pub fn from_system_crontab() -> Self {
        Self::SystemCrontab(SystemCrontab)
    }

    /// A system crontab read from a file (`-F`/`--system-file`).
    pub fn from_system_file(path: PathBuf) -> Self {
        Self::SystemFile(SystemFile(path))
    }

    /// Read the source into memory, ready to parse.
    ///
    /// A source resolves to one or more [`Read`]s: files and the live
    /// crontab resolve to one; `--system` fans out over its directory.
    fn read(&self) -> Result<Vec<Read>, CrontabSourcesError> {
        // Read every variant via `source.read()`, never by hand.
        match self {
            Self::UserCrontab(source) => Ok(vec![source.read()?]),
            Self::UserFile(source) => Ok(vec![source.read()?]),
            Self::SystemFile(source) => Ok(vec![source.read()?]),
            Self::SystemCrontab(source) => source.read(),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct UserCrontab;

impl UserCrontab {
    #[allow(clippy::unused_self)]
    fn read(&self) -> Result<Read, CrontabSourcesError> {
        let contents = Reader::read().map_err(CrontabSourcesError::LiveRead)?;
        Ok(Read::Live(contents))
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct UserFile(PathBuf);

impl UserFile {
    fn read(&self) -> Result<Read, CrontabSourcesError> {
        Ok(Read::File(CrontabFile::read(Kind::User, &self.0)?))
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct SystemCrontab;

impl SystemCrontab {
    /// Fan out over the system crontab locations, reading each as a
    /// system file: `--system` is just a multi-file special case that
    /// delegates to normal file reading.
    #[allow(clippy::unused_self)]
    fn read(&self) -> Result<Vec<Read>, CrontabSourcesError> {
        system_crontab_paths()?
            .iter()
            .map(|path| Ok(Read::File(CrontabFile::read(Kind::System, path)?)))
            .collect()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct SystemFile(PathBuf);

impl SystemFile {
    fn read(&self) -> Result<Read, CrontabSourcesError> {
        Ok(Read::File(CrontabFile::read(Kind::System, &self.0)?))
    }
}

/// A source read into memory, not yet parsed.
///
/// There are only two things to read: a file (which carries the identity
/// used for dedup and fingerprints) or the live user crontab
/// (`crontab -l`, which has no path).
enum Read {
    Live(String),
    File(CrontabFile),
}

impl Read {
    /// The underlying file, if this is a file read.
    fn as_file(&self) -> Option<&CrontabFile> {
        if let Self::File(file) = self {
            Some(file)
        } else {
            None
        }
    }

    /// Parse into a [`Crontab`]. The live crontab has no document id, so
    /// its fingerprints match a plain `crontab -l` read.
    fn into_crontab(self) -> Crontab {
        match self {
            Self::Live(contents) => Crontab::new(Parser::parse(&contents)),
            Self::File(file) => file.parse(),
        }
    }
}

/// A normalized crontab file source that knows its kind.
///
/// It's the counterpart of a file [`Source`] once read and validated.
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
    fn read(kind: Kind, path: &Path) -> Result<Self, CrontabSourcesError> {
        let canonical_path =
            std::fs::canonicalize(path).map_err(|source| CrontabSourcesError::FileRead {
                path: path.to_path_buf(),
                source,
            })?;
        let contents = std::fs::read_to_string(&canonical_path).map_err(|source| {
            CrontabSourcesError::FileRead {
                path: path.to_path_buf(),
                source,
            }
        })?;

        Ok(Self {
            kind,
            path: path.to_path_buf(),
            canonical_path,
            contents,
        })
    }

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

/// Main system crontab. Same format as `/etc/cron.d/*` (has a `user`).
const SYSTEM_CRONTAB: &str = "/etc/crontab";
/// Directory of drop-in system crontabs.
const SYSTEM_CRONTAB_DIR: &str = "/etc/cron.d";

/// Resolve the system crontab locations to concrete file paths.
///
/// `/etc/crontab` first, then `/etc/cron.d/*` sorted for determinism. A
/// missing `/etc/cron.d` is an empty set, not an error (e.g., macOS).
fn system_crontab_paths() -> Result<Vec<PathBuf>, CrontabSourcesError> {
    let mut paths = Vec::new();

    let main = PathBuf::from(SYSTEM_CRONTAB);
    if is_safe_system_crontab(&main) {
        paths.push(main);
    }

    match std::fs::read_dir(SYSTEM_CRONTAB_DIR) {
        Ok(entries) => {
            let entries = entries.map(|entry| entry.map(|entry| entry.path()));
            paths.extend(system_crontab_dir_paths(entries)?);
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(CrontabSourcesError::FileRead {
                path: PathBuf::from(SYSTEM_CRONTAB_DIR),
                source,
            });
        }
    }

    Ok(paths)
}

/// Collect safe `/etc/cron.d` paths without hiding iteration errors.
fn system_crontab_dir_paths(
    entries: impl Iterator<Item = io::Result<PathBuf>>,
) -> Result<Vec<PathBuf>, CrontabSourcesError> {
    let mut paths = entries.collect::<io::Result<Vec<_>>>().map_err(|source| {
        CrontabSourcesError::FileRead {
            path: PathBuf::from(SYSTEM_CRONTAB_DIR),
            source,
        }
    })?;
    paths.retain(|path| is_valid_cron_d_name(path) && is_safe_system_crontab(path));
    paths.sort();
    Ok(paths)
}

/// Whether Cron considers an automatically discovered system crontab safe.
///
/// Cron excludes files that are not root-owned, are group- or other-writable,
/// or are symlinks not owned by root or not targeting a root-owned file. Mirror
/// those exclusions so `--system` cannot execute configuration Cron ignores.
/// These path checks are not atomic with the later read. That race is accepted
/// here because this mirrors Cron's discovery policy; closing it would require
/// opening, validating, and reading the same file descriptor.
fn is_safe_system_crontab(path: &Path) -> bool {
    let Ok(path_metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    let Ok(target_metadata) = std::fs::metadata(path) else {
        return false;
    };

    has_safe_system_crontab_metadata(
        path_metadata.uid(),
        target_metadata.uid(),
        target_metadata.mode(),
        target_metadata.is_file(),
    )
}

fn has_safe_system_crontab_metadata(
    path_owner: u32,
    target_owner: u32,
    target_mode: u32,
    target_is_file: bool,
) -> bool {
    path_owner == 0 && target_owner == 0 && target_mode & 0o022 == 0 && target_is_file
}

/// Whether cron would pick up this `/etc/cron.d` entry.
///
/// `run-parts` ignores names outside `[A-Za-z0-9_-]`, which skips
/// dotfiles, `.dpkg-dist`, editor backups, and the like.
fn is_valid_cron_d_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            !name.is_empty()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        })
}

#[derive(Debug)]
pub enum CrontabSourcesError {
    LiveRead(ReadError),
    FileRead { path: PathBuf, source: io::Error },
    DuplicateFile { path: PathBuf, first_path: PathBuf },
    DuplicateSource { name: &'static str },
}

impl fmt::Display for CrontabSourcesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LiveRead(source) => write!(f, "{source}"),
            Self::FileRead { path, source } => {
                write!(f, "Cannot read crontab file '{}': {source}", path.display())
            }
            Self::DuplicateFile { path, first_path } => write!(
                f,
                "Crontab file '{}' refers to the same document as '{}'",
                path.display(),
                first_path.display()
            ),
            Self::DuplicateSource { name } => {
                write!(f, "'{name}' is given more than once")
            }
        }
    }
}

impl Error for CrontabSourcesError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::LiveRead(source) => Some(source),
            Self::FileRead { source, .. } => Some(source),
            Self::DuplicateFile { .. } | Self::DuplicateSource { .. } => None,
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
/// [`CrontabSources`] is constructed from a set of [`Source`]s. And
/// uses its own file representation under the hood ([`CrontabFile`]).
#[derive(Debug)]
pub struct CrontabSources {
    sources: Vec<Crontab>,
}

impl CrontabSources {
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

impl TryFrom<&[Source]> for CrontabSources {
    type Error = CrontabSourcesError;

    /// Create an instance from a list of [`Source`]s.
    fn try_from(sources: &[Source]) -> Result<Self, Self::Error> {
        if let Some(error) = find_duplicate_source(sources) {
            return Err(error);
        }

        let reads: Vec<Read> = sources
            .iter()
            .map(Source::read)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();

        if let Some(error) = find_duplicate_files(&reads) {
            return Err(error);
        }

        let crontabs: Vec<Crontab> = reads.into_iter().map(Read::into_crontab).collect();
        Ok(crontabs.into())
    }
}

/// Reject a non-file source (`--user`/`--system`) given more than once.
fn find_duplicate_source(sources: &[Source]) -> Option<CrontabSourcesError> {
    for (index, source) in sources.iter().enumerate() {
        let name = match source {
            Source::UserCrontab(_) => "--user",
            Source::SystemCrontab(_) => "--system",
            Source::UserFile(_) | Source::SystemFile(_) => continue,
        };
        if sources[..index].contains(source) {
            return Some(CrontabSourcesError::DuplicateSource { name });
        }
    }
    None
}

fn find_duplicate_files(reads: &[Read]) -> Option<CrontabSourcesError> {
    let files: Vec<&CrontabFile> = reads.iter().filter_map(Read::as_file).collect();
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
    use std::os::unix::fs::PermissionsExt;

    use cronrunner::parser::{Kind, Parser};
    use cronrunner::tokens::{CronJob, JobSection};

    use super::*;

    #[test]
    fn automatic_system_sources_follow_cron_metadata_exclusions() {
        // Separate path and target owners model Cron's symlink rule.
        assert!(has_safe_system_crontab_metadata(0, 0, 0o600, true));
        assert!(!has_safe_system_crontab_metadata(1_000, 0, 0o600, true));
        assert!(!has_safe_system_crontab_metadata(0, 1_000, 0o600, true));
        assert!(!has_safe_system_crontab_metadata(0, 0, 0o620, true));
        assert!(!has_safe_system_crontab_metadata(0, 0, 0o602, true));
        assert!(!has_safe_system_crontab_metadata(0, 0, 0o600, false));
    }

    #[test]
    fn system_crontab_directory_entry_errors_are_propagated() {
        let entries = [Err(io::Error::other("directory entry failed"))].into_iter();

        let error = system_crontab_dir_paths(entries).unwrap_err();

        let CrontabSourcesError::FileRead { path, source } = error else {
            panic!()
        };
        assert_eq!(path, PathBuf::from(SYSTEM_CRONTAB_DIR));
        assert_eq!(source.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn explicit_system_files_bypass_cron_discovery_exclusions() {
        let temporary_directory =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/tmp/sources_tests");
        std::fs::create_dir_all(&temporary_directory).unwrap();
        let writable = temporary_directory.join("explicit-writable-system.cron");
        std::fs::write(&writable, "@daily root :\n").unwrap();
        let mut permissions = std::fs::metadata(&writable).unwrap().permissions();
        permissions.set_mode(0o666);
        std::fs::set_permissions(&writable, permissions).unwrap();

        assert!(!is_safe_system_crontab(&writable));
        assert!(SystemFile(writable).read().is_ok());
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

        let files: Vec<CrontabFile> = [relative, absolute.clone(), symlink]
            .iter()
            .map(|path| CrontabFile::read(Kind::User, path).unwrap())
            .collect();

        assert_eq!(files[0].canonical_path, absolute);
        assert_eq!(files[1].canonical_path, files[0].canonical_path);
        assert_eq!(files[2].canonical_path, files[0].canonical_path);
    }

    #[test]
    fn duplicate_file_error_retains_both_supplied_paths() {
        let canonical_path = PathBuf::from("/tmp/example.cron");
        let reads = [
            Read::File(CrontabFile {
                kind: Kind::User,
                path: PathBuf::from("example.cron"),
                canonical_path: canonical_path.clone(),
                contents: String::new(),
            }),
            Read::File(CrontabFile {
                kind: Kind::User,
                path: PathBuf::from("./example.cron"),
                canonical_path,
                contents: String::new(),
            }),
        ];

        let error = find_duplicate_files(&reads).unwrap();

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
        let reads = [
            Read::File(CrontabFile {
                kind: Kind::User,
                path: PathBuf::from("first.cron"),
                canonical_path: PathBuf::from("/tmp/first.cron"),
                contents: String::new(),
            }),
            Read::File(CrontabFile {
                kind: Kind::User,
                path: PathBuf::from("second.cron"),
                canonical_path: PathBuf::from("/tmp/second.cron"),
                contents: String::new(),
            }),
        ];

        assert!(find_duplicate_files(&reads).is_none());
    }

    #[test]
    fn file_read_error_retains_path_and_io_error() {
        let missing = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/tmp/sources_tests/does-not-exist.cron");

        let error = CrontabFile::read(Kind::User, &missing).unwrap_err();

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

        let error = CrontabSources::try_from(
            [
                Source::from_user_file(first.clone()),
                Source::from_user_file(second),
            ]
            .as_slice(),
        )
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

        let error = CrontabFile::read(Kind::User, &invalid).unwrap_err();

        let CrontabSourcesError::FileRead { path, source } = error else {
            panic!()
        };
        assert_eq!(path, invalid);
        assert_eq!(source.kind(), io::ErrorKind::InvalidData);
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
