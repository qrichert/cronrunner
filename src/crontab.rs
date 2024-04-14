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

pub mod parser;
pub mod reader;
pub mod tokens;

use std::env;
use std::process::Command;

pub use self::parser::Parser;
pub use self::reader::{ReadError, ReadErrorDetail, Reader};
pub use self::tokens::{CronJob, Token};

/// Default shell used if not overridden by a variable in the `crontab`.
const DEFAULT_SHELL: &str = "/bin/sh";

struct ShellCommand {
    home_directory: String,
    shell: String,
    command: String,
}

/// Low level detail about the run result.
///
/// This is only meant to be used attached to a [`RunResult`], provided
/// by [`Crontab`].
#[derive(Debug)]
pub enum RunResultDetail {
    /// If the command could be run.
    DidRun {
        /// The exit code or `None` if the process was killed early.
        exit_code: Option<i32>,
    },
    /// If the command failed to execute at all (e.g., executable not
    /// found).
    DidNotRun {
        /// Explanation of the error in plain English.
        reason: String,
    },
}

/// Info about a run, provided by [`Crontab`] once it is finished.
#[derive(Debug)]
pub struct RunResult {
    /// Whether the command was successful or not. _Successful_ means
    /// the command ran _AND_ exited without errors (exit 0).
    pub was_successful: bool,
    /// Detail about the run. May contain exit code or reason of
    /// failure, see [`RunResultDetail`].
    pub detail: RunResultDetail,
}

/// Do things with jobs found in the `crontab`.
///
/// Chiefly, [`Crontab`] provides the [`run()`](Crontab::run()) method,
/// and takes a [`Vec<Token>`](Token) as input, usually from [`Parser`].
#[derive(Debug)]
pub struct Crontab {
    pub tokens: Vec<Token>,
}

impl Crontab {
    #[must_use]
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens }
    }

    /// Whether there are jobs in the `crontab` at all.
    ///
    /// Crontab could be empty or only contain variables, comments or
    /// unrecognized tokens.
    #[must_use]
    pub fn has_runnable_jobs(&self) -> bool {
        self.tokens
            .iter()
            .any(|token| matches!(token, Token::CronJob(_)))
    }

    /// All the jobs, and only the jobs.
    #[must_use]
    pub fn jobs(&self) -> Vec<&CronJob> {
        self.tokens
            .iter()
            .filter_map(|token| {
                if let Token::CronJob(job) = token {
                    Some(job)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Whether a given job is in the `crontab` or not.
    #[must_use]
    pub fn has_job(&self, job: &CronJob) -> bool {
        self.jobs().iter().any(|x| *x == job)
    }

    /// Get a job object from its [`UID`](CronJob::uid).
    #[must_use]
    pub fn get_job_from_uid(&self, job_uid: u32) -> Option<&CronJob> {
        if let Some(job) = self.jobs().iter().find(|job| job.uid == job_uid) {
            Some(*job)
        } else {
            None
        }
    }

    /// Run a job.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use cronrunner::crontab::{CronJob, Crontab, RunResult, Token};
    /// #
    /// # let crontab: Crontab = Crontab::new(vec![Token::CronJob(CronJob {
    /// #     uid: 1,
    /// #     schedule: String::new(),
    /// #     command: String::new(),
    /// #     description: String::new(),
    /// # })]);
    /// #
    /// let job: &CronJob = crontab.get_job_from_uid(1).expect("pretend it exists");
    ///
    /// let result: RunResult = crontab.run(job);
    ///
    /// if result.was_successful {
    ///     // ...
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// [`Crontab::run()`] will return a [`RunResult`] regardless of
    /// whether the run succeeded or not.
    ///
    /// [`RunResult::was_successful`] will be set to `true` if the
    /// command ran _AND_ returns `0`, and will be set to `false` in any
    /// other case.
    ///
    /// Unless a run failed (as in the command was not run at all), the
    /// exit code will be provided in [`RunResult`] (but can be `None`
    /// if the process got killed).
    ///
    /// A run can fail if:
    ///
    /// - An invalid job UID was provided.
    /// - The Home directory cannot be read from the environment.
    /// - The shell executable cannot be found.
    #[must_use]
    pub fn run(&self, job: &CronJob) -> RunResult {
        let command = match self.make_shell_command(job) {
            Ok(command) => command,
            Err(reason) => {
                return RunResult {
                    was_successful: false,
                    detail: RunResultDetail::DidNotRun { reason },
                }
            }
        };

        let status = Command::new(command.shell)
            .current_dir(command.home_directory)
            .arg("-c")
            .arg(command.command)
            .status();

        match status {
            Ok(status) => RunResult {
                was_successful: status.success(),
                detail: RunResultDetail::DidRun {
                    exit_code: status.code(),
                },
            },
            Err(_) => RunResult {
                was_successful: false,
                detail: RunResultDetail::DidNotRun {
                    reason: String::from("Failed to run command (does shell exist?)."),
                },
            },
        }
    }

    fn make_shell_command(&self, job: &CronJob) -> Result<ShellCommand, String> {
        let home_directory = Self::get_home_directory()?;
        let (shell, command) = self.convert_job_to_command(job)?;

        Ok(ShellCommand {
            home_directory,
            shell,
            command,
        })
    }

    fn get_home_directory() -> Result<String, String> {
        if let Ok(home_directory) = env::var("HOME") {
            Ok(home_directory)
        } else {
            Err(String::from(
                "Could not read Home directory from environment.",
            ))
        }
    }

    fn convert_job_to_command(&self, job: &CronJob) -> Result<(String, String), String> {
        self.ensure_job_exists(job)?;
        let vars_and_job = self.extract_variables_and_target_job(job);

        let shell = Self::determine_shell_to_use(&vars_and_job);
        let command = Self::variables_and_job_to_shell_command(&vars_and_job);

        Ok((shell, command))
    }

    fn ensure_job_exists(&self, job: &CronJob) -> Result<(), String> {
        if !self.has_job(job) {
            return Err(String::from("The given job is not in the crontab."));
        }
        Ok(())
    }

    fn extract_variables_and_target_job(&self, target_job: &CronJob) -> Vec<&Token> {
        let mut out: Vec<&Token> = Vec::new();
        for token in &self.tokens {
            if let Token::Variable(_) = token {
                out.push(token);
            } else if let Token::CronJob(job) = token {
                if job == target_job {
                    out.push(token);
                    break; // Variables coming after the job are not used.
                }
            }
        }
        out
    }

    fn determine_shell_to_use(tokens: &Vec<&Token>) -> String {
        let mut shell = String::from(DEFAULT_SHELL);
        for token in tokens {
            if let Token::Variable(variable) = token {
                if variable.identifier == "SHELL" {
                    shell = String::from(&variable.value);
                }
            }
        }
        shell
    }

    fn variables_and_job_to_shell_command(tokens: &Vec<&Token>) -> String {
        let mut command: Vec<String> = Vec::new();
        for token in tokens {
            if let Token::Variable(variable) = token {
                command.push(String::from(&variable.statement()));
            } else if let Token::CronJob(job) = token {
                command.push(String::from(&job.command));
            }
        }
        command.join(";")
    }
}

/// Create an instance of [`Crontab`].
///
/// This helper reads the current user's `crontab` and creates a
/// [`Crontab`] instance out of it.
///
/// # Examples
///
/// ```rust
/// use cronrunner::crontab;
///
/// let crontab = match crontab::make_instance() {
///     Ok(crontab) => crontab,
///     Err(_) => return (),
/// };
/// ```
///
/// # Errors
///
/// Will forward [`ReadError`] from [`Reader`] if any.
pub fn make_instance() -> Result<Crontab, ReadError> {
    let crontab: String = Reader::read()?;
    let tokens: Vec<Token> = Parser::parse(&crontab);

    Ok(Crontab::new(tokens))
}

#[cfg(test)]
mod tests {
    use super::tokens::{Comment, Variable};
    use super::*;

    fn tokens() -> Vec<Token> {
        vec![
            Token::Comment(Comment {
                value: String::from("# CronRunner Demo"),
            }),
            Token::Comment(Comment {
                value: String::from("# ---------------"),
            }),
            Token::CronJob(CronJob {
                uid: 1,
                schedule: String::from("@reboot"),
                command: String::from("/usr/bin/bash ~/startup.sh"),
                description: String::new(),
            }),
            Token::Comment(Comment {
                value: String::from(
                    "# Double-hash comments (##) immediately preceding a job are used as",
                ),
            }),
            Token::Comment(Comment {
                value: String::from("# description. See below:"),
            }),
            Token::Comment(Comment {
                value: String::from("## Update brew."),
            }),
            Token::CronJob(CronJob {
                uid: 2,
                schedule: String::from("30 20 * * *"),
                command: String::from("/usr/local/bin/brew update && /usr/local/bin/brew upgrade"),
                description: String::from("Update brew."),
            }),
            Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("bar"),
            }),
            Token::Comment(Comment {
                value: String::from("## Print variable."),
            }),
            Token::CronJob(CronJob {
                uid: 3,
                schedule: String::from("* * * * *"),
                command: String::from("echo $FOO"),
                description: String::from("Print variable."),
            }),
            Token::Comment(Comment {
                value: String::from("# Do nothing (this is a regular comment)."),
            }),
            Token::CronJob(CronJob {
                uid: 4,
                schedule: String::from("@reboot"),
                command: String::from(":"),
                description: String::new(),
            }),
            Token::Variable(Variable {
                identifier: String::from("SHELL"),
                value: String::from("/bin/bash"),
            }),
            Token::CronJob(CronJob {
                uid: 5,
                schedule: String::from("@hourly"),
                command: String::from("echo 'I am echoed by bash!'"),
                description: String::new(),
            }),
        ]
    }

    #[test]
    fn has_runnable_jobs() {
        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 1,
            schedule: String::from("@hourly"),
            command: String::from("echo 'hello, world'"),
            description: String::new(),
        })]);

        assert!(crontab.has_runnable_jobs());
    }

    #[test]
    fn has_no_runnable_jobs() {
        let crontab = Crontab::new(vec![
            Token::Comment(Comment {
                value: String::from("# This is a comment"),
            }),
            Token::Variable(Variable {
                identifier: String::from("SHELL"),
                value: String::from("/bin/bash"),
            }),
        ]);

        assert!(!crontab.has_runnable_jobs());
    }

    #[test]
    fn has_no_runnable_jobs_because_crontab_is_empty() {
        let crontab = Crontab::new(vec![]);

        assert!(!crontab.has_runnable_jobs());
    }

    #[test]
    fn list_of_jobs() {
        let crontab = Crontab::new(tokens());

        let tokens = tokens();
        let jobs: Vec<&CronJob> = tokens
            .iter()
            .filter_map(|token| {
                if let Token::CronJob(job) = token {
                    Some(job)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(crontab.jobs(), jobs);
    }

    #[test]
    fn has_job() {
        let crontab = Crontab::new(tokens());

        // Valid job, same UID.
        assert!(crontab.has_job(&CronJob {
            uid: 1,
            schedule: String::from("@reboot"),
            command: String::from("/usr/bin/bash ~/startup.sh"),
            description: String::new(),
        }),);
        // Valid job, invalid UID.
        assert!(!crontab.has_job(&CronJob {
            uid: 0,
            schedule: String::from("@reboot"),
            command: String::from("/usr/bin/bash ~/startup.sh"),
            description: String::new(),
        }),);
        // Valid job, different job's UID.
        assert!(!crontab.has_job(&CronJob {
            uid: 0,
            schedule: String::from("@reboot"),
            command: String::from("/usr/bin/bash ~/startup.sh"),
            description: String::new(),
        }),);
        // Invalid job, same UID.
        assert!(!crontab.has_job(&CronJob {
            uid: 1,
            schedule: String::from("<invalid>"),
            command: String::from("<invalid>"),
            description: String::new(),
        }),);
    }

    #[test]
    fn get_job() {
        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 1,
            schedule: String::from("@reboot"),
            command: String::from("echo 'hello, world'"),
            description: String::new(),
        })]);

        let job = crontab.get_job_from_uid(1).expect("job exists");

        assert_eq!(
            *job,
            CronJob {
                uid: 1,
                schedule: String::from("@reboot"),
                command: String::from("echo 'hello, world'"),
                description: String::new(),
            }
        );
    }

    #[test]
    fn get_job_not_in_crontab() {
        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 1,
            schedule: String::from("@daily"),
            command: String::from("echo 'hello, world'"),
            description: String::new(),
        })]);

        let job = crontab.get_job_from_uid(42);

        assert!(job.is_none());
    }

    #[test]
    fn working_directory_is_home_directory() {
        env::set_var("HOME", "/home/<test>");

        let home_directory =
            Crontab::get_home_directory().expect("the environment variable is set");

        assert_eq!(home_directory, "/home/<test>");
    }

    #[test]
    fn run_cron_without_variable() {
        let crontab = Crontab::new(tokens());

        let job = crontab.get_job_from_uid(1).expect("job exists in fixture");
        let command = crontab
            .make_shell_command(job)
            .expect("job exists in fixture");

        assert_eq!(command.command, "/usr/bin/bash ~/startup.sh");
    }

    #[test]
    fn run_cron_with_variable() {
        let crontab = Crontab::new(tokens());

        let job = crontab.get_job_from_uid(3).expect("job exists in fixture");
        let command = crontab
            .make_shell_command(job)
            .expect("job exists in fixture");

        assert_eq!(command.command, "FOO=bar;echo $FOO");
    }

    #[test]
    fn run_cron_after_variable_but_not_right_after_it() {
        let crontab = Crontab::new(tokens());

        let job = crontab.get_job_from_uid(4).expect("job exists in fixture");
        let command = crontab
            .make_shell_command(job)
            .expect("job exists in fixture");

        assert_eq!(command.command, "FOO=bar;:");
    }

    #[test]
    fn run_cron_with_default_shell() {
        let crontab = Crontab::new(tokens());

        let job = crontab.get_job_from_uid(1).expect("job exists in fixture");
        let command = crontab
            .make_shell_command(job)
            .expect("job exists in fixture");

        assert_eq!(command.shell, DEFAULT_SHELL);
        assert_eq!(command.command, "/usr/bin/bash ~/startup.sh");
    }

    #[test]
    fn run_cron_with_different_shell() {
        let crontab = Crontab::new(tokens());

        let job = crontab.get_job_from_uid(5).expect("job exists in fixture");
        let command = crontab
            .make_shell_command(job)
            .expect("job exists in fixture");

        assert_eq!(command.shell, "/bin/bash");
        assert_eq!(
            command.command,
            "FOO=bar;SHELL=/bin/bash;echo 'I am echoed by bash!'"
        );
    }

    #[test]
    fn double_shell_change() {
        let crontab = Crontab::new(vec![
            Token::Variable(Variable {
                identifier: String::from("SHELL"),
                value: String::from("/bin/bash"),
            }),
            Token::CronJob(CronJob {
                uid: 1,
                schedule: String::from("@hourly"),
                command: String::from("echo 'I am echoed by bash!'"),
                description: String::new(),
            }),
            Token::Variable(Variable {
                identifier: String::from("SHELL"),
                value: String::from("/bin/zsh"),
            }),
            Token::CronJob(CronJob {
                uid: 2,
                schedule: String::from("@hourly"),
                command: String::from("echo 'I am echoed by zsh!'"),
                description: String::new(),
            }),
        ]);

        let job = crontab.get_job_from_uid(2).expect("job exists in fixture");
        let command = crontab
            .make_shell_command(job)
            .expect("job exists in fixture");

        assert_eq!(command.shell, "/bin/zsh");
        assert_eq!(
            command.command,
            "SHELL=/bin/bash;SHELL=/bin/zsh;echo 'I am echoed by zsh!'"
        );
    }

    #[test]
    fn two_equal_jobs_are_treated_as_different_jobs() {
        let crontab = Crontab::new(vec![
            Token::CronJob(CronJob {
                uid: 1,
                schedule: String::from("@daily"),
                command: String::from("df -h > ~/track_disk_usage.txt"),
                description: String::from("Track disk usage."),
            }),
            Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("bar"),
            }),
            Token::CronJob(CronJob {
                uid: 2,
                schedule: String::from("@daily"),
                command: String::from("df -h > ~/track_disk_usage.txt"),
                description: String::from("Track disk usage."),
            }),
        ]);

        let job = crontab.get_job_from_uid(2).expect("job exists in fixture");
        let command = crontab
            .make_shell_command(job)
            .expect("job exists in fixture");

        // If 'FOO=bar' is not included, it means the first of the twin
        // jobs was used instead of the second that we selected.
        assert_eq!(command.command, "FOO=bar;df -h > ~/track_disk_usage.txt");
    }
}
