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

use std::collections::HashMap;
use std::env;
use std::process::Command;

pub use self::parser::Parser;
pub use self::reader::{ReadError, ReadErrorDetail, Reader};
pub use self::tokens::{CronJob, Token};

/// Default shell used if not overridden by a variable in the `crontab`.
const DEFAULT_SHELL: &str = "/bin/sh";

#[derive(Debug)]
struct ShellCommand {
    env: HashMap<String, String>,
    shell: String,
    home: String,
    command: String,
}

/// Low level detail about the run result.
///
/// This is only meant to be used attached to a [`RunResult`], provided
/// by [`Crontab`].
#[derive(Debug, Eq, PartialEq)]
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
#[derive(Debug, Eq, PartialEq)]
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
        self.jobs().into_iter().find(|job| job.uid == job_uid)
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
    /// #     description: None,
    /// #     section: None,
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
            // .env_clear() // TODO: Cleaner env?
            .envs(&command.env)
            .current_dir(command.home)
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
        self.ensure_job_exists(job)?;

        let mut env = self.extract_variables(job);
        let shell = Self::determine_shell_to_use(&mut env);
        let home = Self::determine_home_to_use(&mut env)?;
        let command = job.command.clone();

        Ok(ShellCommand {
            env,
            shell,
            home,
            command,
        })
    }

    fn ensure_job_exists(&self, job: &CronJob) -> Result<(), String> {
        if !self.has_job(job) {
            return Err(String::from("The given job is not in the crontab."));
        }
        Ok(())
    }

    fn extract_variables(&self, target_job: &CronJob) -> HashMap<String, String> {
        let mut variables: HashMap<String, String> = HashMap::new();
        for token in &self.tokens {
            if let Token::Variable(variable) = token {
                variables.insert(variable.identifier.clone(), variable.value.clone());
            } else if let Token::CronJob(job) = token {
                if job == target_job {
                    break; // Variables coming after the job are not used.
                }
            }
        }
        variables
    }

    fn determine_shell_to_use(env: &mut HashMap<String, String>) -> String {
        if let Some(shell) = env.remove("SHELL") {
            shell
        } else {
            String::from(DEFAULT_SHELL)
        }
    }

    fn determine_home_to_use(env: &mut HashMap<String, String>) -> Result<String, String> {
        if let Some(home) = env.remove("HOME") {
            Ok(home)
        } else {
            Ok(Self::get_home_directory()?)
        }
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
    use super::tokens::{Comment, CommentKind, Variable};
    use super::*;

    // Warning: These tests MUST be run sequentially. Running them in
    // parallel threads may cause conflicts with environment variables,
    // as a variable may be overridden before it is used.

    // TODO(refactor): Tests should be independent, and not use a common
    //  fixture that will break unrelated tests when updated.
    fn tokens() -> Vec<Token> {
        vec![
            Token::Comment(Comment {
                value: String::from("# CronRunner Demo"),
                kind: CommentKind::Regular,
            }),
            Token::Comment(Comment {
                value: String::from("# ---------------"),
                kind: CommentKind::Regular,
            }),
            Token::CronJob(CronJob {
                uid: 1,
                schedule: String::from("@reboot"),
                command: String::from("/usr/bin/bash ~/startup.sh"),
                description: None,
                section: None,
            }),
            Token::Comment(Comment {
                value: String::from(
                    "# Double-hash comments (##) immediately preceding a job are used as",
                ),
                kind: CommentKind::Regular,
            }),
            Token::Comment(Comment {
                value: String::from("# description. See below:"),
                kind: CommentKind::Regular,
            }),
            Token::Comment(Comment {
                value: String::from("## Update brew."),
                kind: CommentKind::Description,
            }),
            Token::CronJob(CronJob {
                uid: 2,
                schedule: String::from("30 20 * * *"),
                command: String::from("/usr/local/bin/brew update && /usr/local/bin/brew upgrade"),
                description: Some(String::from("Update brew.")),
                section: None,
            }),
            Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("bar"),
            }),
            Token::Comment(Comment {
                value: String::from("## Print variable."),
                kind: CommentKind::Description,
            }),
            Token::CronJob(CronJob {
                uid: 3,
                schedule: String::from("* * * * *"),
                command: String::from("echo $FOO"),
                description: Some(String::from("Print variable.")),
                section: None,
            }),
            Token::Comment(Comment {
                value: String::from("# Do nothing (this is a regular comment)."),
                kind: CommentKind::Regular,
            }),
            Token::CronJob(CronJob {
                uid: 4,
                schedule: String::from("@reboot"),
                command: String::from(":"),
                description: None,
                section: None,
            }),
            Token::Variable(Variable {
                identifier: String::from("SHELL"),
                value: String::from("/bin/bash"),
            }),
            Token::CronJob(CronJob {
                uid: 5,
                schedule: String::from("@hourly"),
                command: String::from("echo 'I am echoed by bash!'"),
                description: None,
                section: None,
            }),
            Token::Variable(Variable {
                identifier: String::from("HOME"),
                value: String::from("/home/<custom>"),
            }),
            Token::CronJob(CronJob {
                uid: 6,
                schedule: String::from("@yerly"),
                command: String::from("./cleanup.sh"),
                description: None,
                section: None,
            }),
        ]
    }

    #[test]
    fn has_runnable_jobs() {
        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 1,
            schedule: String::from("@hourly"),
            command: String::from("echo 'hello, world'"),
            description: None,
            section: None,
        })]);

        assert!(crontab.has_runnable_jobs());
    }

    #[test]
    fn has_no_runnable_jobs() {
        let crontab = Crontab::new(vec![
            Token::Comment(Comment {
                value: String::from("# This is a comment"),
                kind: CommentKind::Regular,
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
            description: None,
            section: None,
        }),);
        // Valid job, invalid UID.
        assert!(!crontab.has_job(&CronJob {
            uid: 0,
            schedule: String::from("@reboot"),
            command: String::from("/usr/bin/bash ~/startup.sh"),
            description: None,
            section: None,
        }),);
        // Valid job, different job's UID.
        assert!(!crontab.has_job(&CronJob {
            uid: 0,
            schedule: String::from("@reboot"),
            command: String::from("/usr/bin/bash ~/startup.sh"),
            description: None,
            section: None,
        }),);
        // Invalid job, same UID.
        assert!(!crontab.has_job(&CronJob {
            uid: 1,
            schedule: String::from("<invalid>"),
            command: String::from("<invalid>"),
            description: None,
            section: None,
        }),);
    }

    #[test]
    fn get_job() {
        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 1,
            schedule: String::from("@reboot"),
            command: String::from("echo 'hello, world'"),
            description: None,
            section: None,
        })]);

        let job = crontab.get_job_from_uid(1).expect("job exists");

        assert_eq!(
            *job,
            CronJob {
                uid: 1,
                schedule: String::from("@reboot"),
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            }
        );
    }

    #[test]
    fn get_job_not_in_crontab() {
        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 1,
            schedule: String::from("@daily"),
            command: String::from("echo 'hello, world'"),
            description: None,
            section: None,
        })]);

        let job = crontab.get_job_from_uid(42);

        assert!(job.is_none());
    }

    #[test]
    fn two_equal_jobs_are_treated_as_different_jobs() {
        let crontab = Crontab::new(vec![
            Token::CronJob(CronJob {
                uid: 1,
                schedule: String::from("@daily"),
                command: String::from("df -h > ~/track_disk_usage.txt"),
                description: Some(String::from("Track disk usage.")),
                section: None,
            }),
            Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("bar"),
            }),
            Token::CronJob(CronJob {
                uid: 2,
                schedule: String::from("@daily"),
                command: String::from("df -h > ~/track_disk_usage.txt"),
                description: Some(String::from("Track disk usage.")),
                section: None,
            }),
        ]);

        let job = crontab.get_job_from_uid(2).expect("job exists in fixture");
        let command = crontab
            .make_shell_command(job)
            .expect("job exists in fixture");

        // If 'FOO=bar' is not included, it means the first of the twin
        // jobs was used instead of the second that we selected.
        assert_eq!(
            command.env,
            HashMap::from([(String::from("FOO"), String::from("bar"))])
        );
        assert_eq!(command.command, "df -h > ~/track_disk_usage.txt");
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

        assert_eq!(
            command.env,
            HashMap::from([(String::from("FOO"), String::from("bar"))])
        );
        assert_eq!(command.command, "echo $FOO");
    }

    #[test]
    fn run_cron_after_variable_but_not_right_after_it() {
        let crontab = Crontab::new(tokens());

        let job = crontab.get_job_from_uid(4).expect("job exists in fixture");
        let command = crontab
            .make_shell_command(job)
            .expect("job exists in fixture");

        assert_eq!(
            command.env,
            HashMap::from([(String::from("FOO"), String::from("bar"))])
        );
        assert_eq!(command.command, ":");
    }

    #[test]
    fn double_variable_change() {
        let crontab = Crontab::new(vec![
            Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("bar"),
            }),
            Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("baz"),
            }),
            Token::CronJob(CronJob {
                uid: 1,
                schedule: String::from("30 9 * * * "),
                command: String::from("echo 'gm'"),
                description: None,
                section: None,
            }),
        ]);

        let job = crontab.get_job_from_uid(1).expect("job exists in fixture");
        let command = crontab
            .make_shell_command(job)
            .expect("job exists in fixture");

        assert_eq!(
            command.env,
            HashMap::from([(String::from("FOO"), String::from("baz"))])
        );
        assert_eq!(command.command, "echo 'gm'");
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

        assert_eq!(
            command.env,
            HashMap::from([(String::from("FOO"), String::from("bar"))])
        );
        assert_eq!(command.shell, "/bin/bash");
        assert_eq!(command.command, "echo 'I am echoed by bash!'");
    }

    #[test]
    fn shell_variable_is_removed_from_env() {
        let crontab = Crontab::new(vec![
            Token::Variable(Variable {
                identifier: String::from("SHELL"),
                value: String::from("/bin/<custom>"),
            }),
            Token::CronJob(CronJob {
                uid: 1,
                schedule: String::from("@hourly"),
                command: String::from("echo 'I am echoed by a custom shell!'"),
                description: None,
                section: None,
            }),
        ]);

        let job = crontab.get_job_from_uid(1).expect("job exists in fixture");
        let command = crontab
            .make_shell_command(job)
            .expect("job exists in fixture");

        assert!(!command.env.contains_key("SHELL"));
        assert_eq!(command.shell, "/bin/<custom>");
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
                description: None,
                section: None,
            }),
            Token::Variable(Variable {
                identifier: String::from("SHELL"),
                value: String::from("/bin/zsh"),
            }),
            Token::CronJob(CronJob {
                uid: 2,
                schedule: String::from("@hourly"),
                command: String::from("echo 'I am echoed by zsh!'"),
                description: None,
                section: None,
            }),
        ]);

        let job = crontab.get_job_from_uid(2).expect("job exists in fixture");
        let command = crontab
            .make_shell_command(job)
            .expect("job exists in fixture");

        assert_eq!(command.shell, "/bin/zsh");
        assert_eq!(command.command, "echo 'I am echoed by zsh!'");
    }

    #[test]
    fn run_cron_with_default_home() {
        env::set_var("HOME", "/home/<default>");

        let crontab = Crontab::new(tokens());

        let job = crontab.get_job_from_uid(1).expect("job exists in fixture");
        let command = crontab
            .make_shell_command(job)
            .expect("job exists in fixture");

        assert_eq!(command.home, "/home/<default>");
    }

    #[test]
    fn run_cron_with_different_home() {
        env::set_var("HOME", "/home/<default>");

        let crontab = Crontab::new(tokens());

        let job = crontab.get_job_from_uid(6).expect("job exists in fixture");
        let command = crontab
            .make_shell_command(job)
            .expect("job exists in fixture");

        assert_eq!(
            command.env,
            HashMap::from([(String::from("FOO"), String::from("bar"))])
        );
        assert_eq!(command.home, "/home/<custom>");
        assert_eq!(command.command, "./cleanup.sh");
    }

    #[test]
    fn get_home_directory_error() {
        env::remove_var("HOME");

        let crontab = Crontab::new(tokens());

        let job = crontab.get_job_from_uid(1).expect("job exists in fixture");
        let error = crontab
            .make_shell_command(job)
            .expect_err("should be an error");

        assert_eq!(error, "Could not read Home directory from environment.");

        // If we don't re-create it, other tests will fail.
        env::set_var("HOME", "/home/<test>");
    }

    #[test]
    fn home_variable_is_removed_from_env() {
        let crontab = Crontab::new(vec![
            Token::Variable(Variable {
                identifier: String::from("HOME"),
                value: String::from("/home/<custom>"),
            }),
            Token::CronJob(CronJob {
                uid: 1,
                schedule: String::from("@hourly"),
                command: String::from("echo 'I am echoed in a different Home!'"),
                description: None,
                section: None,
            }),
        ]);

        let job = crontab.get_job_from_uid(1).expect("job exists in fixture");
        let command = crontab
            .make_shell_command(job)
            .expect("job exists in fixture");

        assert!(!command.env.contains_key("HOME"));
        assert_eq!(command.home, "/home/<custom>");
    }

    #[test]
    fn double_home_change() {
        let crontab = Crontab::new(vec![
            Token::Variable(Variable {
                identifier: String::from("HOME"),
                value: String::from("/home/user1"),
            }),
            Token::CronJob(CronJob {
                uid: 1,
                schedule: String::from("@hourly"),
                command: String::from("echo 'I run is user1's Home!'"),
                description: None,
                section: None,
            }),
            Token::Variable(Variable {
                identifier: String::from("HOME"),
                value: String::from("/home/user2"),
            }),
            Token::CronJob(CronJob {
                uid: 2,
                schedule: String::from("@hourly"),
                command: String::from("echo 'I run is user2's Home!'"),
                description: None,
                section: None,
            }),
        ]);

        let job = crontab.get_job_from_uid(2).expect("job exists in fixture");
        let command = crontab
            .make_shell_command(job)
            .expect("job exists in fixture");

        assert_eq!(command.home, "/home/user2");
        assert_eq!(command.command, "echo 'I run is user2's Home!'");
    }

    #[test]
    fn run_cron_with_non_existing_job() {
        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 1,
            schedule: String::from("@hourly"),
            command: String::from("echo 'I am echoed by bash!'"),
            description: None,
            section: None,
        })]);
        let job_not_in_crontab = CronJob {
            uid: 42,
            schedule: String::from("@never"),
            command: String::from("sleep infinity"),
            description: None,
            section: None,
        };

        let error = crontab
            .make_shell_command(&job_not_in_crontab)
            .expect_err("the job is not in the crontab");

        assert_eq!(error, "The given job is not in the crontab.");
    }
}
