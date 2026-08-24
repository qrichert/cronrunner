mod cli;

use std::collections::HashMap;
use std::env;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use lessify::Pager;

use cronrunner::crontab::{Crontab, RunResult, RunResultDetail};
use cronrunner::reader::{ReadError, ReadErrorDetail};
use cronrunner::tokens::{CronJob, JobDescription, JobSection};

use crate::cli::exit_status::ExitStatus;
use crate::cli::sources::{CrontabSources, CrontabSourcesError, Source};
use crate::cli::{args, job::Job, ui};

#[cfg(not(tarpaulin_include))]
fn main() -> ExitStatus {
    let config = match args::Config::build_from_args(env::args()) {
        Ok(config) => config,
        Err(arg) => return exit_from_arguments_error(&arg),
    };

    if config.help {
        println!("{}\n{}", args::help_message(), args::longer_help_notice());
        return ExitStatus::Success;
    } else if config.long_help {
        Pager::page_or_print(&args::long_help_message());
        return ExitStatus::Success;
    } else if config.version {
        println!("{}", args::version_message());
        return ExitStatus::Success;
    }

    // Failing to parse the env file is considered an argument error,
    // thus it must come before other program logic.
    let env = match try_parse_env_file_if_given(config.env_file.as_ref()) {
        Ok(env) => env,
        Err(error) => {
            return exit_from_env_file_parse_error(
                &config.env_file.expect("can't fail without a file"),
                &error,
            );
        }
    };

    // No source flag means read the current user's live crontab.
    let sources = if config.crontab_sources.is_empty() {
        vec![Source::from_user_crontab()]
    } else {
        config.crontab_sources
    };
    let mut sources = match CrontabSources::try_from(sources.as_slice()) {
        Ok(sources) => sources,
        Err(error) => return exit_from_crontab_sources_error(&error),
    };

    if !sources.has_runnable_jobs() {
        return exit_from_no_runnable_jobs();
    }

    if config.list_only {
        if config.as_json {
            println!("{}", sources.to_json());
        } else {
            print_job_selection_menu(sources.documents(), config.fingerprint);
        }
        return ExitStatus::Success;
    }

    let job_selected = if let Some(job) = config.job {
        job
    } else if let Some(job) = read_job_selection_from_stdin(config.fingerprint) {
        job
    } else {
        print_job_selection_menu(sources.documents(), config.fingerprint);

        match get_user_selection(config.fingerprint) {
            Err(()) => return exit_from_invalid_job_selection(),
            Ok(None) => return ExitStatus::Success,
            Ok(Some(job)) => job,
        }
    };

    if job_selected == Job::Uid(42) && sources.jobs().len() < 42 {
        println!("What was the question again?");
        return ExitStatus::Success;
    }

    let Some((crontab, job)) = sources.select(&job_selected) else {
        return exit_from_invalid_job_selection();
    };
    if let Some(env) = env {
        crontab.set_env(env);
    }

    println!("{} {}", ui::Color::highlight("$"), job.command);

    let res = if config.detach {
        crontab.run_detached(&job)
    } else {
        crontab.run(&job)
    };
    exit_from_run_result(res)
}

fn exit_from_arguments_error(arg: &str) -> ExitStatus {
    eprintln!("{}", args::bad_arguments_error_message(arg));
    ExitStatus::ArgsError
}

fn exit_from_crontab_sources_error(error: &CrontabSourcesError) -> ExitStatus {
    match error {
        CrontabSourcesError::LiveRead(error) => exit_from_crontab_read_error(error),
        CrontabSourcesError::FileRead { .. } => {
            eprintln!("{label}: {error}.", label = ui::Color::error("error"));
            ExitStatus::Failure
        }
        CrontabSourcesError::DuplicateFile { .. } | CrontabSourcesError::DuplicateSource { .. } => {
            exit_from_arguments_error(&error.to_string())
        }
    }
}

fn try_parse_env_file_if_given(
    env_file: Option<&PathBuf>,
) -> Result<Option<HashMap<String, String>>, String> {
    let Some(env_file) = env_file else {
        return Ok(None); // Not given.
    };

    if !env_file.is_file() {
        return Err(format!("'{}' does not exist.", env_file.display()));
    }
    let Ok(env) = std::fs::read_to_string(env_file) else {
        #[cfg(not(tarpaulin_include))] // Hard to make reading fail.
        return Err(format!("'{}' could not be read.", env_file.display()));
    };

    let env: HashMap<String, String> = env
        .lines()
        .filter_map(|line| {
            let (variable, value) = line.trim().split_once('=')?;
            // Skip special variables.
            if ["SHLVL", "_"].contains(&variable) {
                return None;
            }
            Some((variable.to_string(), value.to_string()))
        })
        .collect();

    Ok(Some(env))
}

fn exit_from_env_file_parse_error(env_file: &Path, reason: &str) -> ExitStatus {
    eprintln!(
        "\
{error}: Error parsing environment file.
{reason}

Hint:
  You can export Cron's environment by temporarily adding this job
  to the crontab, and letting Cron run it:

      {min}*{reset} {h}*{reset} {d}*{reset} {mon}*{reset} {dow}*{reset} {command}env > {env_file}{reset}
",
        env_file=env_file.display(),
        error = ui::Color::error("error"),
        min = ui::Color::maybe_color("\x1b[95m"),
        h = ui::Color::maybe_color("\x1b[38;5;81m"),
        d = ui::Color::maybe_color("\x1b[38;5;121m"),
        mon = ui::Color::maybe_color("\x1b[95m"),
        dow = ui::Color::maybe_color("\x1b[96m"),
        command = ui::Color::maybe_color("\x1b[93m"),
        reset = ui::Color::maybe_color(ui::RESET),
    );
    ExitStatus::Failure
}

fn exit_from_crontab_read_error(error: &ReadError) -> ExitStatus {
    eprintln!(
        "{error}: {}",
        error.reason,
        error = ui::Color::error("error")
    );

    if let ReadErrorDetail::NonZeroExit { exit_code, stderr } = &error.detail {
        if let Some(stderr) = stderr {
            eprintln!("{}", strip_terminating_newline(stderr));
        }
        if let Some(exit_code) = exit_code {
            return (*exit_code).into();
        }
    }

    ExitStatus::Failure
}

fn strip_terminating_newline(text: &str) -> &str {
    text.strip_suffix('\n').unwrap_or(text)
}

fn exit_from_no_runnable_jobs() -> ExitStatus {
    println!("No jobs to run.");
    ExitStatus::Success
}

#[cfg(not(tarpaulin_include))]
fn read_job_selection_from_stdin(use_fingerprint: bool) -> Option<Job> {
    // If the descriptor/handle refers to a terminal/tty, there is
    // nothing in stdin to be consumed yet.
    if io::stdin().is_terminal() {
        return None;
    }

    let mut job_selected = String::new();
    if io::stdin().read_line(&mut job_selected).is_err() {
        return None;
    }

    match parse_user_job_selection(&job_selected, use_fingerprint) {
        Ok(Some(job_selected)) => Some(job_selected),
        _ => None,
    }
}

#[cfg(not(tarpaulin_include))]
fn print_job_selection_menu(documents: &[Crontab], use_fingerprint: bool) {
    let entries = format_jobs_as_menu_entries(documents, use_fingerprint);
    println!("{}", entries.join("\n"));
}

fn format_jobs_as_menu_entries(documents: &[Crontab], use_fingerprint: bool) -> Vec<String> {
    let jobs_by_document: Vec<Vec<&CronJob>> = documents
        .iter()
        .map(Crontab::jobs)
        .filter(|jobs| !jobs.is_empty())
        .collect();
    let jobs: Vec<&CronJob> = jobs_by_document.iter().flatten().copied().collect();
    let has_sections = jobs.iter().any(|job| job.section.is_some());
    let mut menu = Vec::with_capacity(jobs.len());

    let mut last_section = None;
    // Only used to right-align UIDs; fingerprints are fixed-width.
    let max_uid_width = determine_max_uid_width(&jobs);

    let mut documents = jobs_by_document.iter().peekable();
    while let Some(document) = documents.next() {
        for &job in document {
            if let Some(new_section) = update_section_if_needed(job, &mut last_section) {
                menu.push(format_job_section(new_section));
            }

            let number = if use_fingerprint {
                format_job_fingerprint(job.fingerprint)
            } else {
                format_job_uid(job.uid, max_uid_width)
            };
            let description = format_job_description(job.description.as_ref());
            let schedule = format_job_schedule(&job.schedule);
            let user = format_job_user(job.user.as_deref());
            let command = format_job_command(&job.command, !description.is_empty());

            menu.push(format!("{number} {description}{schedule}{user}{command}"));
        }

        if let Some(next_document) = documents.peek() {
            close_section_if_needed(&mut menu, next_document, &mut last_section);
        }
    }

    add_spacing_to_menu_if_it_has_sections(&mut menu, has_sections);

    menu
}

fn close_section_if_needed(
    menu: &mut Vec<String>,
    next_document: &[&CronJob],
    last_section: &mut Option<JobSection>,
) {
    // - Unsectioned -> Unsectioned: No blank line.
    // - Sectioned   -> Unsectioned: Close section with blank line.
    // - Unsectioned -> Sectioned: No blank line, section header provides spacing.
    // - Sectioned   -> Sectioned: No blank line, section header provides spacing.
    let previous_document_ended_in_section = last_section.take().is_some();
    let next_document_starts_without_section = next_document
        .first()
        .is_some_and(|job| job.section.is_none());

    if previous_document_ended_in_section && next_document_starts_without_section {
        menu.push(String::new());
    }
}

fn determine_max_uid_width(jobs: &[&CronJob]) -> usize {
    let max_uid = jobs.iter().map(|job| job.uid).max().unwrap_or(0);
    max_uid.to_string().len()
}

fn update_section_if_needed<'a>(
    job: &CronJob,
    last_section: &'a mut Option<JobSection>,
) -> Option<&'a JobSection> {
    if job.section.is_some() && job.section != *last_section {
        last_section.clone_from(&job.section);
        return last_section.as_ref();
    }
    None
}

fn format_job_section(section: &JobSection) -> String {
    format!("\n{}\n", ui::Color::title(&section.to_string()))
}

fn format_job_fingerprint(fingerprint: u64) -> String {
    // Fixed 16-wide (full `u64`) so every fingerprint is the same
    // length and stays stable regardless of what else is loaded.
    // Leading zeros don't affect selection: input is parsed as hex,
    // so `0094...` and `94...` are the same job.
    ui::Color::highlight(&format!("{fingerprint:016x}")).into_owned()
}

fn format_job_uid(uid: usize, max_uid_width: usize) -> String {
    ui::Color::highlight(&format!("{uid:>max_uid_width$}.")).into_owned()
}

fn format_job_description(description: Option<&JobDescription>) -> String {
    if let Some(description) = description {
        format!("{description} ")
    } else {
        String::new()
    }
}

fn format_job_schedule(schedule: &str) -> String {
    ui::Color::attenuate(schedule).into_owned()
}

fn format_job_user(user: Option<&str>) -> String {
    if let Some(user) = user {
        format!(
            " {user}{is_root} ",
            user = ui::Color::accentuate(user),
            is_root = if user == "root" { "*" } else { "" }
        )
    } else {
        String::from(" ")
    }
}

fn format_job_command(command: &str, has_description: bool) -> String {
    if has_description {
        ui::Color::attenuate(command).into_owned()
    } else {
        String::from(command)
    }
}

fn add_spacing_to_menu_if_it_has_sections(menu: &mut Vec<String>, has_sections: bool) {
    // It looks weird having spacing around section titles,
    // but not after the last job line.
    if has_sections {
        menu.push(String::new());
    }
}

#[cfg(not(tarpaulin_include))]
fn get_user_selection(use_fingerprint: bool) -> Result<Option<Job>, ()> {
    print!(">>> Select a job to run: ");
    // Flush manually in case `stdout` is line-buffered (common case),
    // else the previous print won't be displayed immediately (no `\n`).
    _ = io::stdout().flush();

    let mut job_selected = String::new();
    io::stdin().read_line(&mut job_selected).map_err(|_| ())?;

    parse_user_job_selection(&job_selected, use_fingerprint)
}

fn parse_user_job_selection(job_selected: &str, use_fingerprint: bool) -> Result<Option<Job>, ()> {
    let job_selected = String::from(job_selected.trim());

    if job_selected.is_empty() {
        return Ok(None);
    }

    if use_fingerprint {
        if let Ok(job_selected) = u64::from_str_radix(&job_selected, 16) {
            return Ok(Some(Job::Fingerprint(job_selected)));
        }
    } else if let Ok(job_selected) = job_selected.parse::<usize>() {
        return Ok(Some(Job::Uid(job_selected)));
    }

    Err(())
}

fn exit_from_invalid_job_selection() -> ExitStatus {
    eprintln!(
        "{error}: Invalid job selection.",
        error = ui::Color::error("error")
    );
    ExitStatus::Failure
}

fn exit_from_run_result(result: RunResult) -> ExitStatus {
    if result.was_successful {
        return ExitStatus::Success;
    }

    match result.detail {
        RunResultDetail::DidNotRun { reason } => {
            eprintln!("{error}: {reason}", error = ui::Color::error("error"));
            ExitStatus::Failure
        }
        RunResultDetail::DidRun { exit_code: None } => ExitStatus::Failure,
        RunResultDetail::DidRun {
            exit_code: Some(exit_code),
        } => exit_code.into(),
        RunResultDetail::IsRunning { pid } => {
            println!("{pid}");
            ExitStatus::Success
        }
    }
}

#[cfg(test)]
mod tests {
    use cronrunner::parser::Parser;
    use cronrunner::tokens::Token;

    use super::*;

    const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/");

    fn format_single_document_jobs_as_menu_entries(
        jobs: &[&CronJob],
        use_fingerprint: bool,
    ) -> Vec<String> {
        let tokens = jobs
            .iter()
            .map(|job| Token::CronJob((*job).clone()))
            .collect();
        format_jobs_as_menu_entries(&[Crontab::new(tokens)], use_fingerprint)
    }

    fn menu_test_job(uid: usize, section: Option<JobSection>) -> CronJob {
        CronJob {
            uid,
            fingerprint: uid as u64,
            tag: None,
            schedule: String::from("@hourly"),
            user: None,
            command: format!("echo {uid}"),
            description: None,
            section,
        }
    }

    fn job_document(job: CronJob) -> Crontab {
        Crontab::new(vec![Token::CronJob(job)])
    }

    #[test]
    fn exit_from_arguments_error_regular() {
        let arg = "--unknown";

        let exit_code = exit_from_arguments_error(arg);

        assert_eq!(exit_code, ExitStatus::ArgsError);
    }

    #[test]
    fn exit_from_env_file_parse_error_regular() {
        let file = PathBuf::from("/dev/null");
        let reason = "'/dev/null' does not exist";

        let exit_code = exit_from_env_file_parse_error(&file, reason);

        assert_eq!(exit_code, ExitStatus::Failure);
    }

    #[test]
    fn exit_from_crontab_read_error_with_non_zero_with_exit_code() {
        let error = ReadError {
            reason: "Could not run command.",
            detail: ReadErrorDetail::NonZeroExit {
                stderr: Some(String::from("Bad arguments.")),
                exit_code: Some(2),
            },
        };

        let exit_code = exit_from_crontab_read_error(&error);

        assert_eq!(exit_code, ExitStatus::ArgsError);
    }

    #[test]
    fn exit_from_crontab_read_error_without_exit_code() {
        let error = ReadError {
            reason: "Could not run command.",
            detail: ReadErrorDetail::NonZeroExit {
                stderr: None,
                exit_code: None,
            },
        };

        let exit_code = exit_from_crontab_read_error(&error);

        assert_eq!(exit_code, ExitStatus::Failure);
    }

    #[test]
    fn exit_from_crontab_read_error_could_not_run_command() {
        let error = ReadError {
            reason: "Could not run command.",
            detail: ReadErrorDetail::CouldNotRunCommand,
        };

        let exit_code = exit_from_crontab_read_error(&error);

        assert_eq!(exit_code, ExitStatus::Failure);
    }

    #[test]
    fn try_parse_env_file_if_given_regular() {
        let file = PathBuf::from(FIXTURES_DIR).join("cron.env");

        let env = try_parse_env_file_if_given(Some(&file)).unwrap().unwrap();

        assert_eq!(
            env,
            HashMap::from([
                (String::from("FOO"), String::from("bar")),
                (String::from("BAZ"), String::from("42")),
            ])
        );
    }

    #[test]
    fn try_parse_env_file_if_given_empty_file() {
        let file = PathBuf::from(FIXTURES_DIR).join("cron.env.empty");

        let env = try_parse_env_file_if_given(Some(&file)).unwrap().unwrap();

        assert_eq!(env, HashMap::new());
    }

    #[test]
    fn try_parse_env_file_if_given_removes_special_variables() {
        let file = PathBuf::from(FIXTURES_DIR).join("cron.env");

        let env = try_parse_env_file_if_given(Some(&file)).unwrap().unwrap();

        assert!(!env.contains_key("SHLVL"));
        assert!(!env.contains_key("_"));
    }

    #[test]
    fn try_parse_env_file_if_given_not_given() {
        let file = None;

        let res = try_parse_env_file_if_given(file);

        assert!(matches!(res, Ok(None)));
    }

    #[test]
    fn try_parse_env_file_if_given_file_does_not_exist() {
        let file = PathBuf::from(FIXTURES_DIR).join("does-not-exist");

        let err = try_parse_env_file_if_given(Some(&file)).unwrap_err();

        assert_eq!(err, format!("'{}' does not exist.", file.display()));
    }

    #[test]
    fn strip_terminating_newline_with_newline() {
        let stripped_text = strip_terminating_newline("foo\nbar\n\n");

        assert_eq!(stripped_text, "foo\nbar\n");
    }

    #[test]
    fn strip_terminating_newline_without_newline() {
        let stripped_text = strip_terminating_newline("foo\nbar");

        assert_eq!(stripped_text, "foo\nbar");
    }

    #[test]
    fn strip_terminating_newline_empty_string() {
        let stripped_text = strip_terminating_newline("");

        assert_eq!(stripped_text, "");
    }

    #[test]
    fn exit_from_no_runnable_jobs_is_success() {
        let exit_code = exit_from_no_runnable_jobs();

        assert_eq!(exit_code, ExitStatus::Success);
    }

    #[test]
    fn format_menu_entries() {
        let tokens = [
            CronJob {
                uid: 1,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@hourly"),
                user: None,
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
            CronJob {
                uid: 2,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@monthly"),
                user: None,
                command: String::from("echo 'buongiorno'"),
                description: Some(JobDescription(String::from("This job has a description"))),
                section: None,
            },
        ];

        let entries =
            format_single_document_jobs_as_menu_entries(&tokens.iter().collect::<Vec<_>>(), false);

        assert_eq!(
            entries,
            vec![
                String::from(
                    "\u{1b}[0;92m1.\u{1b}[0m \u{1b}[0;90m@hourly\u{1b}[0m echo 'hello, world'"
                ),
                String::from(
                    "\u{1b}[0;92m2.\u{1b}[0m This job has a description \u{1b}[0;90m@monthly\u{1b}[0m \u{1b}[0;90mecho 'buongiorno'\u{1b}[0m"
                ),
            ]
        );
    }

    #[test]
    fn format_menu_entries_with_fingerprint() {
        let tokens = [
            CronJob {
                uid: 1,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@hourly"),
                user: None,
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
            CronJob {
                uid: 2,
                fingerprint: 1_234_567,
                tag: None,
                schedule: String::from("@monthly"),
                user: None,
                command: String::from("echo 'buongiorno'"),
                description: Some(JobDescription(String::from("This job has a description"))),
                section: None,
            },
        ];

        let entries =
            format_single_document_jobs_as_menu_entries(&tokens.iter().collect::<Vec<_>>(), true);

        assert_eq!(
            entries,
            vec![
                String::from(
                    "\u{1b}[0;92m0000000000cc1dae\u{1b}[0m \u{1b}[0;90m@hourly\u{1b}[0m echo 'hello, world'"
                ),
                String::from(
                    "\u{1b}[0;92m000000000012d687\u{1b}[0m This job has a description \u{1b}[0;90m@monthly\u{1b}[0m \u{1b}[0;90mecho 'buongiorno'\u{1b}[0m"
                ),
            ]
        );
    }

    #[test]
    fn format_menu_sections() {
        let tokens = [
            CronJob {
                uid: 1,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@hourly"),
                user: None,
                command: String::from("echo 'foo'"),
                description: None,
                section: None,
            },
            CronJob {
                uid: 2,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@monthly"),
                user: None,
                command: String::from("echo 'bar'"),
                description: None,
                section: Some(JobSection {
                    uid: 1,
                    title: String::from("These jobs have a section"),
                }),
            },
            CronJob {
                uid: 3,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@monthly"),
                user: None,
                command: String::from("echo 'baz'"),
                description: None,
                section: Some(JobSection {
                    uid: 2,
                    title: String::from("These jobs have a section"),
                }),
            },
        ];

        let entries =
            format_single_document_jobs_as_menu_entries(&tokens.iter().collect::<Vec<_>>(), false);

        assert_eq!(
            entries,
            vec![
                String::from("\u{1b}[0;92m1.\u{1b}[0m \u{1b}[0;90m@hourly\u{1b}[0m echo 'foo'"),
                String::from("\n\u{1b}[1;4mThese jobs have a section\u{1b}[0m\n"),
                String::from("\u{1b}[0;92m2.\u{1b}[0m \u{1b}[0;90m@monthly\u{1b}[0m echo 'bar'"),
                String::from("\n\u{1b}[1;4mThese jobs have a section\u{1b}[0m\n"),
                String::from("\u{1b}[0;92m3.\u{1b}[0m \u{1b}[0;90m@monthly\u{1b}[0m echo 'baz'"),
                String::new(),
            ]
        );
    }

    #[test]
    fn format_menu_does_not_separate_unsectioned_documents() {
        let documents = [
            job_document(menu_test_job(1, None)),
            job_document(menu_test_job(2, None)),
        ];

        let entries = format_jobs_as_menu_entries(&documents, false);

        assert_eq!(entries.len(), 2);
        assert!(entries[0].contains("echo 1"));
        assert!(entries[1].contains("echo 2"));
    }

    #[test]
    fn format_menu_closes_section_before_unsectioned_document() {
        let documents = [
            job_document(menu_test_job(
                1,
                Some(JobSection {
                    uid: 1,
                    title: String::from("First"),
                }),
            )),
            job_document(menu_test_job(2, None)),
        ];

        let entries = format_jobs_as_menu_entries(&documents, false);

        assert_eq!(entries.len(), 5);
        assert!(entries[0].contains("First"));
        assert!(entries[1].contains("echo 1"));
        assert_eq!(entries[2], String::new());
        assert!(entries[3].contains("echo 2"));
        assert_eq!(entries[4], String::new());
    }

    #[test]
    fn format_menu_uses_section_spacing_after_unsectioned_document() {
        let documents = [
            job_document(menu_test_job(1, None)),
            job_document(menu_test_job(
                2,
                Some(JobSection {
                    uid: 1,
                    title: String::from("Second"),
                }),
            )),
        ];

        let entries = format_jobs_as_menu_entries(&documents, false);

        assert_eq!(entries.len(), 4);
        assert!(entries[0].contains("echo 1"));
        assert!(entries[1].contains("Second"));
        assert!(entries[2].contains("echo 2"));
        assert_eq!(entries[3], String::new());
    }

    #[test]
    fn format_menu_does_not_duplicate_spacing_between_sectioned_documents() {
        let documents = [
            job_document(menu_test_job(
                1,
                Some(JobSection {
                    uid: 1,
                    title: String::from("First"),
                }),
            )),
            job_document(menu_test_job(
                2,
                Some(JobSection {
                    uid: 2,
                    title: String::from("Second"),
                }),
            )),
        ];

        let entries = format_jobs_as_menu_entries(&documents, false);

        assert_eq!(entries.len(), 5);
        assert!(entries[0].contains("First"));
        assert!(entries[1].contains("echo 1"));
        assert!(entries[2].contains("Second"));
        assert!(entries[3].contains("echo 2"));
        assert_eq!(entries[4], String::new());
    }

    #[test]
    fn format_menu_ignores_empty_documents_between_unsectioned_documents() {
        let documents = [
            job_document(menu_test_job(1, None)),
            Crontab::new(Vec::new()),
            Crontab::new(Parser::parse("FOO=bar")),
            job_document(menu_test_job(2, None)),
        ];

        let entries = format_jobs_as_menu_entries(&documents, false);

        assert_eq!(entries.len(), 2);
        assert!(entries[0].contains("echo 1"));
        assert!(entries[1].contains("echo 2"));
    }

    #[test]
    fn job_uid_alignment() {
        let tokens = [
            CronJob {
                uid: 1,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@hourly"),
                user: None,
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
            CronJob {
                uid: 108,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@hourly"),
                user: None,
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
            CronJob {
                uid: 12,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@hourly"),
                user: None,
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
        ];

        let entries =
            format_single_document_jobs_as_menu_entries(&tokens.iter().collect::<Vec<_>>(), false);

        assert!(entries[0].starts_with("\u{1b}[0;92m  1.\u{1b}[0m"));
        assert!(entries[1].starts_with("\u{1b}[0;92m108.\u{1b}[0m"));
        assert!(entries[2].starts_with("\u{1b}[0;92m 12.\u{1b}[0m"));
    }

    #[test]
    fn job_fingerprint_is_fixed_width_zero_padded() {
        let tokens = [
            CronJob {
                uid: 1,
                fingerprint: 1,
                tag: None,
                schedule: String::from("@hourly"),
                user: None,
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
            CronJob {
                uid: 1337,
                fingerprint: 1337,
                tag: None,
                schedule: String::from("@hourly"),
                user: None,
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
            CronJob {
                uid: 12,
                fingerprint: 12,
                tag: None,
                schedule: String::from("@hourly"),
                user: None,
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
        ];

        let entries =
            format_single_document_jobs_as_menu_entries(&tokens.iter().collect::<Vec<_>>(), true);

        assert!(entries[0].starts_with("\u{1b}[0;92m0000000000000001\u{1b}[0m"));
        assert!(entries[1].starts_with("\u{1b}[0;92m0000000000000539\u{1b}[0m"));
        assert!(entries[2].starts_with("\u{1b}[0;92m000000000000000c\u{1b}[0m"));
    }

    #[test]
    fn fingerprint_token_does_not_depend_on_other_jobs() {
        fn fingerprint_token(entry: &str) -> &str {
            entry
                .strip_prefix("\u{1b}[0;92m")
                .unwrap()
                .split_once("\u{1b}[0m")
                .unwrap()
                .0
                .trim_start()
        }

        let short_job = CronJob {
            uid: 1,
            fingerprint: 0x94_b1_9a_b6_8c_84_11,
            tag: None,
            schedule: String::from("@hourly"),
            user: None,
            command: String::from("echo 'short'"),
            description: None,
            section: None,
        };
        let wide_job = CronJob {
            uid: 2,
            fingerprint: u64::MAX,
            tag: None,
            schedule: String::from("@hourly"),
            user: None,
            command: String::from("echo 'wide'"),
            description: None,
            section: None,
        };

        let alone = format_single_document_jobs_as_menu_entries(&[&short_job], true);
        let with_wide_job =
            format_single_document_jobs_as_menu_entries(&[&short_job, &wide_job], true);

        assert_eq!(fingerprint_token(&alone[0]), "0094b19ab68c8411");
        assert_eq!(
            fingerprint_token(&alone[0]),
            fingerprint_token(&with_wide_job[0])
        );
    }

    #[test]
    fn format_menu_entries_uid_is_correct() {
        let tokens = [CronJob {
            uid: 42,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@hourly"),
            user: None,
            command: String::from("echo '¡hola!'"),
            description: None,
            section: None,
        }];

        let entries =
            format_single_document_jobs_as_menu_entries(&tokens.iter().collect::<Vec<_>>(), false);

        assert_eq!(
            entries,
            vec![String::from(
                "\u{1b}[0;92m42.\u{1b}[0m \u{1b}[0;90m@hourly\u{1b}[0m echo '¡hola!'"
            )]
        );
    }

    #[test]
    fn format_menu_entries_fingerprint_is_correct() {
        let tokens = [CronJob {
            uid: 42,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@hourly"),
            user: None,
            command: String::from("echo '¡hola!'"),
            description: None,
            section: None,
        }];

        let entries =
            format_single_document_jobs_as_menu_entries(&tokens.iter().collect::<Vec<_>>(), true);

        assert_eq!(
            entries,
            vec![String::from(
                "\u{1b}[0;92m0000000000cc1dae\u{1b}[0m \u{1b}[0;90m@hourly\u{1b}[0m echo '¡hola!'"
            )]
        );
    }

    #[test]
    fn parse_user_job_selection_fingerprint_redirection() {
        let selection = parse_user_job_selection("1", true).unwrap().unwrap();

        assert!(matches!(selection, Job::Fingerprint(_)));
    }

    #[test]
    fn parse_user_job_selection_uid_redirection() {
        let selection = parse_user_job_selection("1", false).unwrap().unwrap();

        assert!(matches!(selection, Job::Uid(_)));
    }

    #[test]
    fn parse_user_job_selection_fingerprint_success() {
        let selection = parse_user_job_selection("1", true).unwrap().unwrap();

        assert_eq!(selection, Job::Fingerprint(1));
    }

    #[test]
    fn parse_user_job_selection_fingerprint_success_with_whitespace() {
        let selection = parse_user_job_selection(&String::from("   1337   \n"), true)
            .unwrap()
            .unwrap();

        assert_eq!(selection, Job::Fingerprint(4919));
    }

    #[test]
    fn parse_user_job_selection_fingerprint_success_but_empty() {
        let selection = parse_user_job_selection("    \n", true).unwrap();

        assert!(selection.is_none());
    }

    #[test]
    fn parse_user_job_selection_fingerprint_error() {
        let selection = parse_user_job_selection("-1", true);

        assert_eq!(selection, Err(()));
    }

    #[test]
    fn parse_user_job_selection_uid_success() {
        let selection = parse_user_job_selection("1", false).unwrap().unwrap();

        assert_eq!(selection, Job::Uid(1));
    }

    #[test]
    fn parse_user_job_selection_uid_success_with_whitespace() {
        let selection = parse_user_job_selection(&String::from("   1337   \n"), false)
            .unwrap()
            .unwrap();

        assert_eq!(selection, Job::Uid(1337));
    }

    #[test]
    fn parse_user_job_selection_uid_success_but_empty() {
        let selection = parse_user_job_selection("    \n", false).unwrap();

        assert!(selection.is_none());
    }

    #[test]
    fn parse_user_job_selection_uid_error() {
        let selection = parse_user_job_selection("-1", false);

        assert_eq!(selection, Err(()));
    }

    #[test]
    fn exit_from_invalid_job_selection_is_error() {
        let exit_code = exit_from_invalid_job_selection();

        assert_eq!(exit_code, ExitStatus::Failure);
    }

    #[test]
    fn exit_from_run_result_success() {
        let result = RunResult {
            was_successful: true,
            detail: RunResultDetail::DidRun { exit_code: Some(0) },
        };

        let exit_code = exit_from_run_result(result);

        assert_eq!(exit_code, ExitStatus::Success);
    }

    #[test]
    fn exit_from_run_result_error_did_not_run() {
        let result = RunResult {
            was_successful: false,
            detail: RunResultDetail::DidNotRun {
                reason: String::from("Error running job."),
            },
        };

        let exit_code = exit_from_run_result(result);

        assert_eq!(exit_code, ExitStatus::Failure);
    }

    #[test]
    fn exit_from_run_result_error_did_run_without_exit_code() {
        let result = RunResult {
            was_successful: false,
            detail: RunResultDetail::DidRun { exit_code: None },
        };

        let exit_code = exit_from_run_result(result);

        assert_eq!(exit_code, ExitStatus::Failure);
    }

    #[test]
    fn exit_from_run_result_error_did_run_with_exit_code() {
        let result = RunResult {
            was_successful: false,
            detail: RunResultDetail::DidRun {
                exit_code: Some(42),
            },
        };

        let exit_code = exit_from_run_result(result);

        assert_eq!(exit_code, ExitStatus::Code(42));
    }

    #[test]
    fn exit_from_run_result_child_process_is_running() {
        let result = RunResult {
            was_successful: false,
            detail: RunResultDetail::IsRunning { pid: 1337 },
        };

        let exit_code = exit_from_run_result(result);

        assert_eq!(exit_code, ExitStatus::Success);
    }

    #[test]
    fn exit_from_crontab_sources_file_read_error_is_failure() {
        let error = CrontabSourcesError::FileRead {
            path: PathBuf::from("missing.cron"),
            source: io::Error::new(io::ErrorKind::NotFound, "missing"),
        };

        assert_eq!(exit_from_crontab_sources_error(&error), ExitStatus::Failure);
    }

    #[test]
    fn exit_from_duplicate_crontab_source_error_is_arguments_error() {
        let error = CrontabSourcesError::DuplicateFile {
            path: PathBuf::from("./example.cron"),
            first_path: PathBuf::from("example.cron"),
        };

        assert_eq!(
            exit_from_crontab_sources_error(&error),
            ExitStatus::ArgsError
        );
    }

    #[test]
    fn format_job_user_none_is_a_single_space() {
        assert_eq!(format_job_user(None), " ");
    }

    #[test]
    fn format_job_user_is_accentuated() {
        assert_eq!(
            format_job_user(Some("www-data")),
            " \u{1b}[0;94mwww-data\u{1b}[0m "
        );
    }

    #[test]
    fn format_job_user_root_is_starred() {
        // `root` is the escalation-danger case (we run it as the
        // current user, not root), so it gets an extra marker.
        assert_eq!(
            format_job_user(Some("root")),
            " \u{1b}[0;94mroot\u{1b}[0m* "
        );
    }

    #[test]
    fn format_menu_entry_renders_system_user() {
        let job = CronJob {
            uid: 1,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@daily"),
            user: Some(String::from("root")),
            command: String::from("echo 'hi'"),
            description: None,
            section: None,
        };

        let entries = format_single_document_jobs_as_menu_entries(&[&job], false);

        assert_eq!(
            entries,
            vec![String::from(
                "\u{1b}[0;92m1.\u{1b}[0m \u{1b}[0;90m@daily\u{1b}[0m \u{1b}[0;94mroot\u{1b}[0m* echo 'hi'"
            )]
        );
    }
}
