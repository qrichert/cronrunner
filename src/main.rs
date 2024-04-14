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

mod crontab;
mod ui;

use crate::crontab::{CronJob, ReadError, ReadErrorDetail, RunResult, RunResultDetail};
use crate::ui::{color_attenuate, color_error, color_highlight};
use std::io::Write;

use std::process::ExitCode;

fn main() -> ExitCode {
    let crontab = match crontab::make_instance() {
        Ok(crontab) => crontab,
        Err(error) => return exit_from_crontab_read_error(error),
    };
    if !crontab.has_runnable_jobs() {
        return exit_from_no_runnable_jobs();
    }

    print_job_selection_menu(&crontab.jobs());

    let job_selected = match get_user_selection() {
        Err(()) => {
            // Bad input.
            return exit_from_invalid_job_selection();
        }
        Ok(None) => {
            // No input.
            return ExitCode::SUCCESS;
        }
        Ok(Some(job_selected)) => job_selected,
    };
    let Some(job) = crontab.get_job_from_uid(job_selected) else {
        return exit_from_invalid_job_selection();
    };

    println!("{} {}", color_highlight("$"), &job.command);

    let res = crontab.run(job);
    exit_from_run_result(res)
}

fn exit_from_crontab_read_error(error: ReadError) -> ExitCode {
    eprintln!("{}", color_error(&error.reason));

    if let ReadErrorDetail::NonZeroExit { exit_code, stderr } = error.detail {
        if let Some(stderr) = stderr {
            eprintln!("{stderr}");
        }
        if let Some(exit_code) = exit_code {
            let exit_code = convert_i32_exit_code_to_u8_exit_code(exit_code);
            return ExitCode::from(exit_code);
        }
    }

    ExitCode::FAILURE
}

fn exit_from_no_runnable_jobs() -> ExitCode {
    println!("No jobs to run.");
    ExitCode::SUCCESS
}

fn print_job_selection_menu(jobs: &Vec<&CronJob>) {
    // Print jobs available, numbered.
    for &job in jobs {
        let job_number = color_highlight(&format!("{}.", job.uid));

        let job_has_description = job.description.is_empty();

        let description = if job_has_description {
            String::new()
        } else {
            format!("{} ", job.description)
        };

        let schedule = color_attenuate(&job.schedule);

        let command = if job_has_description {
            String::from(&job.command)
        } else {
            color_attenuate(&job.command)
        };

        println!("{job_number} {description}{schedule} {command}");
    }
}

fn exit_from_invalid_job_selection() -> ExitCode {
    eprintln!("{}", color_error("Invalid job selection."));
    ExitCode::FAILURE
}

fn get_user_selection() -> Result<Option<u32>, ()> {
    print!(">>> Select a job to run: ");
    // Flush manually in case `stdout` is line-buffered (common case),
    // else the previous print won't be displayed immediately (no `\n`).
    std::io::stdout().flush().unwrap();

    let mut job_selected = String::new();
    std::io::stdin()
        .read_line(&mut job_selected)
        .expect("cannot read user input");
    job_selected = String::from(job_selected.trim());

    if job_selected.is_empty() {
        return Ok(None);
    }

    if let Ok(job_selected) = job_selected.parse::<u32>() {
        Ok(Some(job_selected))
    } else {
        Err(())
    }
}

fn exit_from_run_result(result: RunResult) -> ExitCode {
    if result.was_successful {
        return ExitCode::SUCCESS;
    }

    let detail = result.detail;

    if let RunResultDetail::DidNotRun { reason } = detail {
        eprintln!("{}", color_error(&reason));
        return ExitCode::FAILURE;
    }

    if let RunResultDetail::DidRun {
        exit_code: Some(exit_code),
    } = detail
    {
        let exit_code = convert_i32_exit_code_to_u8_exit_code(exit_code);
        return ExitCode::from(exit_code);
    }

    ExitCode::FAILURE
}

fn convert_i32_exit_code_to_u8_exit_code(code: i32) -> u8 {
    // error_code in [0 ; 255]
    if code >= i32::from(u8::MIN) && code <= i32::from(u8::MAX) {
        return u8::try_from(code).expect("bounds have been checked already");
    }
    1u8 // Default to generic exit 1.
}
