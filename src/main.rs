// cronrunner — Run cron jobs manually.
// Copyright (C) 2024  Quentin Richert
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

mod cli;

use std::collections::HashMap;
use std::env;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use cronrunner::crontab::{self, RunResult, RunResultDetail};
use cronrunner::reader::{ReadError, ReadErrorDetail};
use cronrunner::tokens::{CronJob, JobDescription, JobSection};

use crate::cli::exit_status::ExitStatus;
use crate::cli::output::Pager;
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

    let mut crontab = match crontab::make_instance() {
        Ok(crontab) => crontab,
        Err(error) => return exit_from_crontab_read_error(&error),
    };
    if let Some(env) = env {
        crontab.set_env(env);
    }

    if !crontab.has_runnable_jobs() {
        return exit_from_no_runnable_jobs();
    }

    if config.list_only {
        if config.as_json {
            println!("{}", crontab.to_json());
        } else {
            print_job_selection_menu(&crontab.jobs(), config.safe);
        }
        return ExitStatus::Success;
    }

    let job_selected = if let Some(job) = config.job {
        job
    } else if let Some(job) = read_job_selection_from_stdin(config.safe) {
        job
    } else {
        print_job_selection_menu(&crontab.jobs(), config.safe);

        match get_user_selection(config.safe) {
            Err(()) => return exit_from_invalid_job_selection(),
            Ok(None) => return ExitStatus::Success,
            Ok(Some(job)) => job,
        }
    };

    if job_selected == Job::Uid(42) && crontab.jobs().len() < 42 {
        println!("What was the question again?");
        return ExitStatus::Success;
    }

    let Some(job) = (match job_selected {
        Job::Uid(job) => crontab.get_job_from_uid(job),
        Job::Fingerprint(job) => crontab.get_job_from_fingerprint(job),
        Job::Tag(tag) => crontab.get_job_from_tag(&tag),
    }) else {
        return exit_from_invalid_job_selection();
    };

    println!("{} {}", ui::Color::highlight("$"), &job.command);

    let res = if config.detach {
        crontab.run_detached(job)
    } else {
        crontab.run(job)
    };
    exit_from_run_result(res)
}

fn exit_from_arguments_error(arg: &str) -> ExitStatus {
    eprintln!("{}", args::bad_arguments_error_message(arg));
    ExitStatus::ArgsError
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
        return Err(format!("'{}' could not be read.", &env_file.display()));
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
fn print_job_selection_menu(jobs: &Vec<&CronJob>, use_fingerprint: bool) {
    let entries = format_jobs_as_menu_entries(jobs, use_fingerprint);
    println!("{}", entries.join("\n"));
}

fn format_jobs_as_menu_entries(jobs: &Vec<&CronJob>, use_fingerprint: bool) -> Vec<String> {
    let mut menu = Vec::with_capacity(jobs.len());

    let mut last_section = None;
    let max_id_width = determine_max_id_width(jobs, use_fingerprint);

    for &job in jobs {
        if let Some(new_section) = update_section_if_needed(job, &mut last_section) {
            menu.push(format_job_section(new_section));
        }

        let number = if use_fingerprint {
            format_job_fingerprint(job.fingerprint, max_id_width)
        } else {
            format_job_uid(job.uid, max_id_width)
        };
        let description = format_job_description(job.description.as_ref());
        let schedule = format_job_schedule(&job.schedule);
        let command = format_job_command(&job.command, !description.is_empty());

        menu.push(format!("{number} {description}{schedule} {command}"));
    }

    add_spacing_to_menu_if_it_has_sections(&mut menu, last_section.is_some());

    menu
}

fn determine_max_id_width(jobs: &[&CronJob], use_fingerprint: bool) -> usize {
    if use_fingerprint {
        jobs.iter()
            .map(|job| format!("{:x}", job.fingerprint).len())
            .max()
            .unwrap_or(0)
    } else {
        let max_uid = jobs.iter().map(|job| job.uid).max().unwrap_or(0);
        max_uid.to_string().len()
    }
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

fn format_job_fingerprint(fingerprint: u64, max_uid_width: usize) -> String {
    ui::Color::highlight(&format!("{fingerprint:0>max_uid_width$x}")).into_owned()
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
    io::stdin()
        .read_line(&mut job_selected)
        .expect("cannot read user input");

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
    use super::*;

    const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/");

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
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
            CronJob {
                uid: 2,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@monthly"),
                command: String::from("echo 'buongiorno'"),
                description: Some(JobDescription(String::from("This job has a description"))),
                section: None,
            },
        ];

        let entries = format_jobs_as_menu_entries(&tokens.iter().collect(), false);

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
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
            CronJob {
                uid: 2,
                fingerprint: 1_234_567,
                tag: None,
                schedule: String::from("@monthly"),
                command: String::from("echo 'buongiorno'"),
                description: Some(JobDescription(String::from("This job has a description"))),
                section: None,
            },
        ];

        let entries = format_jobs_as_menu_entries(&tokens.iter().collect(), true);

        assert_eq!(
            entries,
            vec![
                String::from(
                    "\u{1b}[0;92mcc1dae\u{1b}[0m \u{1b}[0;90m@hourly\u{1b}[0m echo 'hello, world'"
                ),
                String::from(
                    "\u{1b}[0;92m12d687\u{1b}[0m This job has a description \u{1b}[0;90m@monthly\u{1b}[0m \u{1b}[0;90mecho 'buongiorno'\u{1b}[0m"
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
                command: String::from("echo 'foo'"),
                description: None,
                section: None,
            },
            CronJob {
                uid: 2,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@monthly"),
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
                command: String::from("echo 'baz'"),
                description: None,
                section: Some(JobSection {
                    uid: 2,
                    title: String::from("These jobs have a section"),
                }),
            },
        ];

        let entries = format_jobs_as_menu_entries(&tokens.iter().collect(), false);

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
    fn job_uid_alignment() {
        let tokens = [
            CronJob {
                uid: 1,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@hourly"),
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
            CronJob {
                uid: 108,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@hourly"),
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
            CronJob {
                uid: 12,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@hourly"),
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
        ];

        let entries = format_jobs_as_menu_entries(&tokens.iter().collect(), false);

        assert!(entries[0].starts_with("\u{1b}[0;92m  1.\u{1b}[0m"));
        assert!(entries[1].starts_with("\u{1b}[0;92m108.\u{1b}[0m"));
        assert!(entries[2].starts_with("\u{1b}[0;92m 12.\u{1b}[0m"));
    }

    #[test]
    fn job_uid_alignment_with_fingerprint() {
        let tokens = [
            CronJob {
                uid: 1,
                fingerprint: 1,
                tag: None,
                schedule: String::from("@hourly"),
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
            CronJob {
                uid: 1337,
                fingerprint: 1337,
                tag: None,
                schedule: String::from("@hourly"),
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
            CronJob {
                uid: 12,
                fingerprint: 12,
                tag: None,
                schedule: String::from("@hourly"),
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
        ];

        let entries = format_jobs_as_menu_entries(&tokens.iter().collect(), true);

        assert!(entries[0].starts_with("\u{1b}[0;92m001\u{1b}[0m"));
        assert!(entries[1].starts_with("\u{1b}[0;92m539\u{1b}[0m"));
        assert!(entries[2].starts_with("\u{1b}[0;92m00c\u{1b}[0m"));
    }

    #[test]
    fn format_menu_entries_uid_is_correct() {
        let tokens = [CronJob {
            uid: 42,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@hourly"),
            command: String::from("echo '¡hola!'"),
            description: None,
            section: None,
        }];

        let entries = format_jobs_as_menu_entries(&tokens.iter().collect(), false);

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
            command: String::from("echo '¡hola!'"),
            description: None,
            section: None,
        }];

        let entries = format_jobs_as_menu_entries(&tokens.iter().collect(), true);

        assert_eq!(
            entries,
            vec![String::from(
                "\u{1b}[0;92mcc1dae\u{1b}[0m \u{1b}[0;90m@hourly\u{1b}[0m echo '¡hola!'"
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
}
