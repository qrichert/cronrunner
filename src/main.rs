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

mod args;
mod ui;

use cronrunner::crontab;
use cronrunner::crontab::{
    CronJob, JobDescription, JobSection, ReadError, ReadErrorDetail, RunResult, RunResultDetail,
};

use std::env;
use std::io::{IsTerminal, Write};
use std::process::ExitCode;

#[cfg(not(tarpaulin_include))]
fn main() -> ExitCode {
    let config = match args::Config::make_from_args(env::args()) {
        Ok(config) => config,
        Err(arg) => return exit_from_argument_error(&arg).into(),
    };

    if config.help {
        println!("{}", args::help_message());
        return ExitCode::SUCCESS;
    } else if config.version {
        println!("{}", args::version_message());
        return ExitCode::SUCCESS;
    }

    let crontab = match crontab::make_instance() {
        Ok(crontab) => crontab,
        Err(error) => return exit_from_crontab_read_error(&error).into(),
    };
    if !crontab.has_runnable_jobs() {
        return exit_from_no_runnable_jobs().into();
    }

    let job_selected = if let Some(job) = read_job_selection_from_stdin() {
        job
    } else {
        print_job_selection_menu(&crontab.jobs());

        match get_user_selection() {
            Err(()) => return exit_from_invalid_job_selection().into(),
            Ok(None) => return ExitCode::SUCCESS,
            Ok(Some(job)) => job,
        }
    };

    if job_selected == 42 && crontab.jobs().len() < 42 {
        println!("What was the question again?");
        return ExitCode::SUCCESS;
    }

    let Some(job) = crontab.get_job_from_uid(job_selected) else {
        return exit_from_invalid_job_selection().into();
    };

    println!("{} {}", ui::color_highlight("$"), &job.command);

    let res = crontab.run(job);
    exit_from_run_result(res).into()
}

fn exit_from_argument_error(arg: &str) -> u8 {
    eprintln!("{}", args::unexpected_argument_error_message(arg));
    2u8
}

fn exit_from_crontab_read_error(error: &ReadError) -> u8 {
    eprintln!("{}", ui::color_error(error.reason));

    if let ReadErrorDetail::NonZeroExit { exit_code, stderr } = &error.detail {
        if let Some(stderr) = stderr {
            eprintln!("{}", strip_terminating_newline(stderr));
        }
        if let Some(exit_code) = exit_code {
            return convert_i32_exit_code_to_u8_exit_code(*exit_code);
        }
    }

    1u8
}

fn strip_terminating_newline(text: &str) -> &str {
    text.strip_suffix('\n').unwrap_or(text)
}

fn exit_from_no_runnable_jobs() -> u8 {
    println!("No jobs to run.");
    0u8
}

#[cfg(not(tarpaulin_include))]
fn read_job_selection_from_stdin() -> Option<u32> {
    // If the descriptor/handle refers to a terminal/tty, there is
    // nothing in stdin to be consumed yet.
    if std::io::stdin().is_terminal() {
        return None;
    }
    let mut job_selected = String::new();
    if std::io::stdin().read_line(&mut job_selected).is_err() {
        return None;
    }
    match parse_user_job_selection(&job_selected) {
        Ok(Some(job_selected)) => Some(job_selected),
        _ => None,
    }
}

#[cfg(not(tarpaulin_include))]
fn print_job_selection_menu(jobs: &Vec<&CronJob>) {
    let entries = format_jobs_as_menu_entries(jobs);
    println!("{}", entries.join("\n"));
}

fn format_jobs_as_menu_entries(jobs: &Vec<&CronJob>) -> Vec<String> {
    let mut menu = Vec::with_capacity(jobs.len());

    let mut last_section = None;
    let max_uid_width = determine_max_uid_width(jobs);

    for &job in jobs {
        if let Some(new_section) = update_section_if_needed(job, &mut last_section) {
            menu.push(format_job_section(new_section));
        }

        let number = format_job_uid(job.uid, max_uid_width);
        let description = format_job_description(&job.description);
        let schedule = format_job_schedule(&job.schedule);
        let command = format_job_command(&job.command, !description.is_empty());

        menu.push(format!("{number} {description}{schedule} {command}"));
    }

    add_spacing_to_menu_if_it_has_sections(&mut menu, last_section.is_some());

    menu
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
    format!("\n{}\n", ui::color_title(&section.to_string()))
}

fn format_job_uid(uid: u32, max_uid_width: usize) -> String {
    let padding = determine_uid_padding(uid, max_uid_width);
    ui::color_highlight(&format!("{padding}{uid}."))
}

fn determine_uid_padding(job_uid: u32, width: usize) -> String {
    let job_uid = job_uid.to_string();
    let padding_length = width.saturating_sub(job_uid.len());
    " ".repeat(padding_length)
}

fn format_job_description(description: &Option<JobDescription>) -> String {
    if let Some(description) = description {
        format!("{description} ")
    } else {
        String::new()
    }
}

fn format_job_schedule(schedule: &str) -> String {
    ui::color_attenuate(schedule)
}

fn format_job_command(command: &str, has_description: bool) -> String {
    if has_description {
        ui::color_attenuate(command)
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
fn get_user_selection() -> Result<Option<u32>, ()> {
    print!(">>> Select a job to run: ");
    // Flush manually in case `stdout` is line-buffered (common case),
    // else the previous print won't be displayed immediately (no `\n`).
    std::io::stdout().flush().unwrap_or_default();

    let mut job_selected = String::new();
    std::io::stdin()
        .read_line(&mut job_selected)
        .expect("cannot read user input");

    parse_user_job_selection(&job_selected)
}

fn parse_user_job_selection(job_selected: &str) -> Result<Option<u32>, ()> {
    let job_selected = String::from(job_selected.trim());

    if job_selected.is_empty() {
        return Ok(None);
    }

    if let Ok(job_selected) = job_selected.parse::<u32>() {
        Ok(Some(job_selected))
    } else {
        Err(())
    }
}

fn exit_from_invalid_job_selection() -> u8 {
    eprintln!("{}", ui::color_error("Invalid job selection."));
    1u8
}

fn exit_from_run_result(result: RunResult) -> u8 {
    if result.was_successful {
        return 0u8;
    }

    let detail = result.detail;

    if let RunResultDetail::DidNotRun { reason } = detail {
        eprintln!("{}", ui::color_error(&reason));
        return 1u8;
    }

    if let RunResultDetail::DidRun {
        exit_code: Some(exit_code),
    } = detail
    {
        let exit_code = convert_i32_exit_code_to_u8_exit_code(exit_code);
        return exit_code;
    }

    1u8
}

fn convert_i32_exit_code_to_u8_exit_code(code: i32) -> u8 {
    // error_code in [0 ; 255]
    if code >= i32::from(u8::MIN) && code <= i32::from(u8::MAX) {
        return u8::try_from(code).expect("bounds have been checked already");
    }
    1u8 // Default to generic exit 1.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_from_argument_error_regular() {
        let arg = "--unknown";

        let exit_code = exit_from_argument_error(arg);

        assert_eq!(exit_code, 2u8);
    }

    #[test]
    fn exit_from_crontab_read_error_with_non_zero_with_exit_code() {
        let error = ReadError {
            reason: "Could not run command.",
            detail: ReadErrorDetail::NonZeroExit {
                stderr: Some(String::from("Bad arguments.")),
                exit_code: Some(2i32),
            },
        };

        let exit_code = exit_from_crontab_read_error(&error);

        assert_eq!(exit_code, 2u8);
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

        assert_eq!(exit_code, 1u8);
    }

    #[test]
    fn exit_from_crontab_read_error_could_not_run_command() {
        let error = ReadError {
            reason: "Could not run command.",
            detail: ReadErrorDetail::CouldNotRunCommand,
        };

        let exit_code = exit_from_crontab_read_error(&error);

        assert_eq!(exit_code, 1u8);
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

        assert_eq!(exit_code, 0u8);
    }

    #[test]
    fn format_menu_entries() {
        let tokens = [
            CronJob {
                uid: 1,
                schedule: String::from("@hourly"),
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
            CronJob {
                uid: 2,
                schedule: String::from("@monthly"),
                command: String::from("echo 'buongiorno'"),
                description: Some(JobDescription(String::from("This job has a description"))),
                section: None,
            },
        ];

        let entries = format_jobs_as_menu_entries(&tokens.iter().collect());

        assert_eq!(
            entries,
            vec![
                String::from("\u{1b}[0;92m1.\u{1b}[0m \u{1b}[0;90m@hourly\u{1b}[0m echo 'hello, world'"),
                String::from("\u{1b}[0;92m2.\u{1b}[0m This job has a description \u{1b}[0;90m@monthly\u{1b}[0m \u{1b}[0;90mecho 'buongiorno'\u{1b}[0m"),
            ]
        );
    }

    #[test]
    fn format_menu_sections() {
        let tokens = [
            CronJob {
                uid: 1,
                schedule: String::from("@hourly"),
                command: String::from("echo 'foo'"),
                description: None,
                section: None,
            },
            CronJob {
                uid: 2,
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
                schedule: String::from("@monthly"),
                command: String::from("echo 'baz'"),
                description: None,
                section: Some(JobSection {
                    uid: 2,
                    title: String::from("These jobs have a section"),
                }),
            },
        ];

        let entries = format_jobs_as_menu_entries(&tokens.iter().collect());

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
                schedule: String::from("@hourly"),
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
            CronJob {
                uid: 108,
                schedule: String::from("@hourly"),
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
            CronJob {
                uid: 12,
                schedule: String::from("@hourly"),
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            },
        ];

        let entries = format_jobs_as_menu_entries(&tokens.iter().collect());

        assert!(entries[0].starts_with("\u{1b}[0;92m  1.\u{1b}[0m"));
        assert!(entries[1].starts_with("\u{1b}[0;92m108.\u{1b}[0m"));
        assert!(entries[2].starts_with("\u{1b}[0;92m 12.\u{1b}[0m"));
    }

    #[test]
    fn format_menu_entries_uid_is_correct() {
        let tokens = [CronJob {
            uid: 42,
            schedule: String::from("@hourly"),
            command: String::from("echo '¡hola!'"),
            description: None,
            section: None,
        }];

        let entries = format_jobs_as_menu_entries(&tokens.iter().collect());

        assert_eq!(
            entries,
            vec![String::from(
                "\u{1b}[0;92m42.\u{1b}[0m \u{1b}[0;90m@hourly\u{1b}[0m echo '¡hola!'"
            )]
        );
    }

    #[test]
    fn parse_user_job_selection_success() {
        let selection = parse_user_job_selection("1")
            .expect("valid input")
            .expect("non empty input");

        assert_eq!(selection, 1u32);
    }

    #[test]
    fn parse_user_job_selection_success_with_whitespace() {
        let selection = parse_user_job_selection(&String::from("   1337   \n"))
            .expect("valid input")
            .expect("non empty input");

        assert_eq!(selection, 1337u32);
    }

    #[test]
    fn parse_user_job_selection_success_but_empty() {
        let selection = parse_user_job_selection("    \n").expect("valid input");

        assert!(selection.is_none());
    }

    #[test]
    fn parse_user_job_selection_error() {
        let selection = parse_user_job_selection("-1");

        assert_eq!(selection, Err(()));
    }

    #[test]
    fn exit_from_invalid_job_selection_is_error() {
        let exit_code = exit_from_invalid_job_selection();

        assert_eq!(exit_code, 1u8);
    }

    #[test]
    fn exit_from_run_result_success() {
        let result = RunResult {
            was_successful: true,
            detail: RunResultDetail::DidRun {
                exit_code: Some(0i32),
            },
        };

        let exit_code = exit_from_run_result(result);

        assert_eq!(exit_code, 0u8);
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

        assert_eq!(exit_code, 1u8);
    }

    #[test]
    fn exit_from_run_result_error_but_did_run_with_exit_code() {
        let result = RunResult {
            was_successful: false,
            detail: RunResultDetail::DidRun {
                exit_code: Some(42i32),
            },
        };

        let exit_code = exit_from_run_result(result);

        assert_eq!(exit_code, 42u8);
    }

    #[test]
    fn exit_from_run_result_error_but_did_run_without_exit_code() {
        let result = RunResult {
            was_successful: false,
            detail: RunResultDetail::DidRun { exit_code: None },
        };

        let exit_code = exit_from_run_result(result);

        assert_eq!(exit_code, 1u8);
    }

    #[test]
    fn convert_i32_to_u8_exit_code() {
        // Test boundaries and middle value.
        assert_eq!(convert_i32_exit_code_to_u8_exit_code(0i32), 0u8);
        assert_eq!(convert_i32_exit_code_to_u8_exit_code(1i32), 1u8);
        assert_eq!(convert_i32_exit_code_to_u8_exit_code(255i32), 255u8);
    }

    #[test]
    fn convert_i32_to_u8_exit_code_out_of_lower_bound() {
        assert_eq!(convert_i32_exit_code_to_u8_exit_code(-1i32), 1u8);
    }

    #[test]
    fn convert_i32_to_u8_exit_code_out_of_upper_bound() {
        assert_eq!(convert_i32_exit_code_to_u8_exit_code(256i32), 1u8);
    }
}
