pub(crate) mod hash;

pub mod parser;
pub mod reader;
pub mod tokens;

use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::fmt::Write;
use std::process::{Command, Stdio};

use self::parser::Parser;
use self::reader::{ReadError, Reader};
use self::tokens::{CronJob, Token};

/// Default shell used if not overridden by a variable in the crontab.
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
        /// The exit code, or `None` if the process was killed early.
        exit_code: Option<i32>,
    },
    /// If the command failed to execute at all (e.g., executable not
    /// found).
    DidNotRun {
        /// Explanation of the error in plain English.
        reason: String,
    },
    /// If the command is run in detached mode and the child process got
    /// spawned successfully.
    IsRunning { pid: u32 },
}

/// Info about a run, provided by [`Crontab`] once it is finished.
#[derive(Debug, Eq, PartialEq)]
pub struct RunResult {
    /// Whether the command was successful or not. _Successful_ means
    /// the command ran _AND_ exited without errors (exit 0).
    ///
    /// <div class="warning">
    ///
    /// Commands ran in detached mode will set `was_successful` to
    /// `false`. This is not a special case according to the previous
    /// definition (the command did not yet exit), but it can be
    /// surprising. Instead, detached commands take advantage of
    /// `detail` to tell whether it was launched successfully, and
    /// provide a PID in that case.
    ///
    /// </div>
    pub was_successful: bool,
    /// Detail about the run. May contain exit code or reason of
    /// failure, see [`RunResultDetail`].
    pub detail: RunResultDetail,
}

/// Do things with jobs found in the crontab.
///
/// Chiefly, [`Crontab`] provides the [`run()`](Crontab::run()) method,
/// and takes a [`Vec<Token>`](Token) as input, usually from [`Parser`].
#[derive(Debug)]
pub struct Crontab {
    pub tokens: Vec<Token>,
    env: Option<HashMap<String, String>>,
}

impl Crontab {
    #[must_use]
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, env: None }
    }

    /// Whether there are jobs in the crontab at all.
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

    /// Whether a given job is in the crontab or not.
    #[must_use]
    pub fn has_job(&self, job: &CronJob) -> bool {
        self.jobs().iter().any(|x| *x == job)
    }

    /// Get a job object from its [`UID`](CronJob::uid).
    #[must_use]
    pub fn get_job_from_uid(&self, uid: usize) -> Option<&CronJob> {
        self.jobs().into_iter().find(|job| job.uid == uid)
    }

    /// Get a job object from its [`fingerprint`](CronJob::fingerprint).
    #[must_use]
    pub fn get_job_from_fingerprint(&self, fingerprint: u64) -> Option<&CronJob> {
        self.jobs()
            .into_iter()
            .find(|job| job.fingerprint == fingerprint)
    }

    /// Get a job object from its [`tag`](CronJob::tag).
    #[must_use]
    pub fn get_job_from_tag(&self, tag: &str) -> Option<&CronJob> {
        self.jobs()
            .into_iter()
            .find(|job| job.tag.as_ref().is_some_and(|job_tag| job_tag == tag))
    }

    /// Override `Crontab`'s default inherited environment.
    ///
    /// By default, jobs are run inheriting the env from the parent
    /// process. This method lets you set a custom environment instead.
    ///
    /// <div class="warning">
    ///
    /// Environments are not additive. The job's env is _replaced_ by
    /// `env`, and not merged with it. If you want to merge the envs,
    /// you will have to do that yourself beforehand.
    ///
    /// </div>
    ///
    /// Note that `set_env()` has no effect on variables declared inside
    /// the crontab or those set on a per-job basis. It only overrides
    /// the default parent-process-inherited environment.
    ///
    /// This requires the `Crontab` instance to be _mutable_.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use std::collections::HashMap;
    /// # use cronrunner::crontab::Crontab;
    /// # let mut crontab: Crontab = Crontab::new(Vec::new());
    /// // let mut crontab = crontab::make_instance()?;
    ///
    /// crontab.set_env(HashMap::from([
    ///     (String::from("FOO"), String::from("bar")),
    ///     (String::from("BAZ"), String::from("42")),
    /// ]));
    ///
    /// // let res = crontab.run(/* ... */);
    /// ```
    pub fn set_env(&mut self, env: HashMap<String, String>) {
        self.env = Some(env);
    }

    /// Run a job.
    ///
    /// By default, the job inherits the environment from the parent
    /// process. Use [`Crontab::set_env()`] to set a custom environment
    /// instead.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use cronrunner::crontab::{Crontab, RunResult};
    /// # use cronrunner::tokens::{CronJob, Token};
    /// #
    /// # let crontab: Crontab = Crontab::new(vec![Token::CronJob(CronJob {
    /// #     uid: 1,
    /// #     fingerprint: 13_376_942,
    /// #     tag: None,
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
    /// command ran _AND_ returned `0`, and will be set to `false` in
    /// any other case.
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
        let mut command = match self.prepare_command(job) {
            Ok(command) => command,
            Err(res) => return res,
        };

        let status = command.status();

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

    /// Run and detach job.
    ///
    /// Mostly the same as [`Crontab::run()`], but doesn't wait for the
    /// job to be finished (returns immediately).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use cronrunner::crontab::{Crontab, RunResult, RunResultDetail};
    /// # use cronrunner::tokens::{CronJob, Token};
    /// #
    /// # let crontab: Crontab = Crontab::new(vec![Token::CronJob(CronJob {
    /// #     uid: 1,
    /// #     fingerprint: 13_376_942,
    /// #     tag: None,
    /// #     schedule: String::new(),
    /// #     command: String::new(),
    /// #     description: None,
    /// #     section: None,
    /// # })]);
    /// #
    /// let job: &CronJob = crontab.get_job_from_fingerprint(13_376_942).expect("pretend it exists");
    ///
    /// let result: RunResult = crontab.run_detached(job);
    ///
    /// if let RunResultDetail::IsRunning { pid } = result.detail {
    ///     // ...
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// [`Crontab::run()`] will return a [`RunResult`] regardless of
    /// whether the run succeeded or not.
    ///
    /// [`RunResult::was_successful`] will always be set to `false`,
    /// because the job is only spawned, we don't wait for it to finish.
    ///
    /// [`RunResult::detail`] will be [`RunResultDetail::IsRunning`],
    /// which will contain the PID of the spawned process.
    #[must_use]
    pub fn run_detached(&self, job: &CronJob) -> RunResult {
        let mut command = match self.prepare_command(job) {
            Ok(command) => command,
            Err(res) => return res,
        };

        #[cfg(not(tarpaulin_include))] // Wrongly marked uncovered.
        let child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        match child {
            Ok(child) => RunResult {
                // We don't know yet, and `false` enables us to take
                // advantage of `detail` more easily, as calling code
                // will naturally fall back to it.
                was_successful: false,
                detail: RunResultDetail::IsRunning { pid: child.id() },
            },
            Err(_) => RunResult {
                was_successful: false,
                detail: RunResultDetail::DidNotRun {
                    reason: String::from("Failed to run command (does shell exist?)."),
                },
            },
        }
    }

    fn prepare_command(&self, job: &CronJob) -> Result<Command, RunResult> {
        let shell_command = match self.make_shell_command(job) {
            Ok(shell_command) => shell_command,
            Err(reason) => {
                return Err(RunResult {
                    was_successful: false,
                    detail: RunResultDetail::DidNotRun { reason },
                });
            }
        };

        #[cfg(not(tarpaulin_include))] // Wrongly marked uncovered.
        {
            let mut command = Command::new(shell_command.shell);

            if let Some(env) = self.env.as_ref() {
                command.env_clear().envs(env);
            }

            command
                .envs(&shell_command.env)
                .current_dir(shell_command.home)
                .arg("-c")
                .arg(shell_command.command);

            Ok(command)
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
            // Set explicitly in Crontab's env.
            shell
        } else {
            String::from(DEFAULT_SHELL)
        }
    }

    fn determine_home_to_use(env: &mut HashMap<String, String>) -> Result<String, String> {
        if let Some(home) = env.remove("HOME") {
            // Set explicitly in Crontab's env.
            Ok(home)
        } else {
            Ok(Self::get_home_directory()?)
        }
    }

    fn get_home_directory() -> Result<String, String> {
        // TODO: Use `std::env::home_dir()` once it gets un-deprecated.
        if let Ok(home_directory) = env::var("HOME") {
            Ok(home_directory)
        } else {
            Err(String::from(
                "Could not read Home directory from environment.",
            ))
        }
    }
}

impl Crontab {
    #[must_use]
    pub fn to_json(&self) -> String {
        let jobs = self.jobs();

        let mut json = String::with_capacity(jobs.len() * 250);
        let mut jobs = jobs.iter().peekable();

        _ = write!(json, "[");
        while let Some(job) = jobs.next() {
            _ = write!(json, "{{");
            _ = write!(json, r#""uid":{},"#, job.uid);
            _ = write!(json, r#""fingerprint":"{:x}","#, job.fingerprint);
            _ = write!(
                json,
                r#""tag":{},"#,
                job.tag.as_ref().map_or_else(
                    || Cow::Borrowed("null"),
                    |tag| { Cow::Owned(format!(r#""{}""#, tag.replace('"', r#"\""#))) }
                )
            );
            _ = write!(json, r#""schedule":"{}","#, job.schedule);
            _ = write!(
                json,
                r#""command":"{}","#,
                job.command.replace('"', r#"\""#)
            );
            _ = write!(
                json,
                r#""description":{},"#,
                job.description.as_ref().map_or_else(
                    || Cow::Borrowed("null"),
                    |description| {
                        Cow::Owned(format!(r#""{}""#, description.0.replace('"', r#"\""#)))
                    }
                )
            );
            _ = write!(
                json,
                r#""section":{}"#,
                job.section.as_ref().map_or_else(
                    || Cow::Borrowed("null"),
                    |section| Cow::Owned(format!(
                        r#"{{"uid":{},"title":"{}"}}"#,
                        section.uid,
                        section.title.replace('"', r#"\""#)
                    ))
                )
            );
            _ = write!(json, "}}");

            if jobs.peek().is_some() {
                _ = write!(json, ",");
            }
        }
        _ = write!(json, "]");

        json
    }
}

/// Create an instance of [`Crontab`].
///
/// This helper reads the current user's crontab and creates a
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
    use self::tokens::{Comment, CommentKind, JobDescription, Variable};
    use super::*;

    // Warning: These tests MUST be run sequentially. Running them in
    // parallel threads may cause conflicts with environment variables,
    // as a variable may be overridden before it is used.

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
                fingerprint: 13_376_942,
                tag: None,
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
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("30 20 * * *"),
                command: String::from("/usr/local/bin/brew update && /usr/local/bin/brew upgrade"),
                description: Some(JobDescription(String::from("Update brew."))),
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
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("* * * * *"),
                command: String::from("echo $FOO"),
                description: Some(JobDescription(String::from("Print variable."))),
                section: None,
            }),
            Token::Comment(Comment {
                value: String::from("# Do nothing (this is a regular comment)."),
                kind: CommentKind::Regular,
            }),
            Token::CronJob(CronJob {
                uid: 4,
                fingerprint: 13_376_942,
                tag: None,
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
                fingerprint: 13_376_942,
                tag: None,
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
                fingerprint: 13_376_942,
                tag: None,
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
            fingerprint: 13_376_942,
            tag: None,
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
        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 1,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@daily"),
            command: String::from("docker image prune --force"),
            description: None,
            section: None,
        })]);

        // Same job, same UID.
        assert!(crontab.has_job(&CronJob {
            uid: 1,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@daily"),
            command: String::from("docker image prune --force"),
            description: None,
            section: None,
        }),);
        // Same job, different UID.
        assert!(!crontab.has_job(&CronJob {
            uid: 0,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@daily"),
            command: String::from("docker image prune --force"),
            description: None,
            section: None,
        }),);
        // Different job, same UID.
        assert!(!crontab.has_job(&CronJob {
            uid: 1,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("<invalid>"),
            command: String::from("<invalid>"),
            description: None,
            section: None,
        }),);
    }

    #[test]
    fn get_job_from_uid() {
        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 1,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@reboot"),
            command: String::from("echo 'hello, world'"),
            description: None,
            section: None,
        })]);

        let job = crontab.get_job_from_uid(1).unwrap();

        assert_eq!(
            *job,
            CronJob {
                uid: 1,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@reboot"),
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            }
        );
    }

    #[test]
    fn get_job_from_uid_not_in_crontab() {
        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 1,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@daily"),
            command: String::from("echo 'hello, world'"),
            description: None,
            section: None,
        })]);

        let job = crontab.get_job_from_uid(42);

        assert!(job.is_none());
    }

    #[test]
    fn get_job_from_fingerprint() {
        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 1,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@reboot"),
            command: String::from("echo 'hello, world'"),
            description: None,
            section: None,
        })]);

        let job = crontab.get_job_from_fingerprint(13_376_942).unwrap();

        assert_eq!(
            *job,
            CronJob {
                uid: 1,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@reboot"),
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            }
        );
    }

    #[test]
    fn get_job_from_fingerprint_not_in_crontab() {
        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 1,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@daily"),
            command: String::from("echo 'hello, world'"),
            description: None,
            section: None,
        })]);

        let job = crontab.get_job_from_fingerprint(42);

        assert!(job.is_none());
    }

    #[test]
    fn get_job_from_tag() {
        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 1,
            fingerprint: 13_376_942,
            tag: Some(String::from("my-tag")),
            schedule: String::from("@reboot"),
            command: String::from("echo 'hello, world'"),
            description: None,
            section: None,
        })]);

        let job = crontab.get_job_from_tag("my-tag").unwrap();

        assert_eq!(
            *job,
            CronJob {
                uid: 1,
                fingerprint: 13_376_942,
                tag: Some(String::from("my-tag")),
                schedule: String::from("@reboot"),
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            }
        );
    }

    #[test]
    fn get_job_from_tag_not_in_crontab() {
        let crontab = Crontab::new(vec![
            Token::CronJob(CronJob {
                uid: 1,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@daily"),
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            }),
            Token::CronJob(CronJob {
                uid: 2,
                fingerprint: 369_108,
                tag: Some(String::from("MY-TAG")),
                schedule: String::from("@daily"),
                command: String::from("echo 'hello, world'"),
                description: None,
                section: None,
            }),
        ]);

        let job = crontab.get_job_from_tag("my-tag");

        assert!(job.is_none());
    }

    #[test]
    fn two_equal_jobs_are_treated_as_different_jobs() {
        let crontab = Crontab::new(vec![
            Token::CronJob(CronJob {
                uid: 1,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@daily"),
                command: String::from("df -h > ~/track_disk_usage.txt"),
                description: Some(JobDescription(String::from("Track disk usage."))),
                section: None,
            }),
            Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("bar"),
            }),
            Token::CronJob(CronJob {
                uid: 2,
                fingerprint: 108_216_215,
                tag: None,
                schedule: String::from("@daily"),
                command: String::from("df -h > ~/track_disk_usage.txt"),
                description: Some(JobDescription(String::from("Track disk usage."))),
                section: None,
            }),
        ]);

        let job = crontab.get_job_from_uid(2).unwrap();
        let command = crontab.make_shell_command(job).unwrap();

        // If 'FOO=bar' is not included, it means the first of the twin
        // jobs was used instead of the second that we selected.
        assert_eq!(
            command.env,
            HashMap::from([(String::from("FOO"), String::from("bar"))])
        );
        assert_eq!(command.command, "df -h > ~/track_disk_usage.txt");
    }

    #[test]
    fn set_env() {
        let mut crontab = Crontab::new(Vec::new());

        assert!(crontab.env.is_none());

        crontab.set_env(HashMap::from([(String::from("FOO"), String::from("bar"))]));

        assert!(
            crontab.env.is_some_and(
                |env| env == HashMap::from([(String::from("FOO"), String::from("bar"))])
            )
        );
    }

    #[test]
    fn set_env_replaces_previous_one() {
        let mut crontab = Crontab::new(Vec::new());

        let env1 = HashMap::from([(String::from("FOO"), String::from("bar"))]);
        let env2 = HashMap::from([(String::from("BAZ"), String::from("42"))]);

        crontab.set_env(env1);
        crontab.set_env(env2.clone());

        assert!(crontab.env.is_some_and(|env| env == env2));
    }

    #[test]
    fn working_directory_is_home_directory() {
        unsafe {
            env::set_var("HOME", "/home/<test>");
        }

        let home_directory = Crontab::get_home_directory().unwrap();

        assert_eq!(home_directory, "/home/<test>");
    }

    #[test]
    fn run_cron_without_variable() {
        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 1,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@reboot"),
            command: String::from("/usr/bin/bash ~/startup.sh"),
            description: Some(JobDescription(String::from("Description."))),
            section: None,
        })]);

        let job = crontab.get_job_from_uid(1).unwrap();
        let command = crontab.make_shell_command(job).unwrap();

        assert_eq!(command.command, "/usr/bin/bash ~/startup.sh");
    }

    #[test]
    fn run_cron_with_variable() {
        let crontab = Crontab::new(vec![
            Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("bar"),
            }),
            Token::CronJob(CronJob {
                uid: 1,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("* * * * *"),
                command: String::from("echo $FOO"),
                description: Some(JobDescription(String::from("Print variable."))),
                section: None,
            }),
        ]);

        let job = crontab.get_job_from_uid(1).unwrap();
        let command = crontab.make_shell_command(job).unwrap();

        assert_eq!(
            command.env,
            HashMap::from([(String::from("FOO"), String::from("bar"))])
        );
        assert_eq!(command.command, "echo $FOO");
    }

    #[test]
    fn run_cron_after_variable_but_not_right_after_it() {
        let crontab = Crontab::new(vec![
            Token::Variable(Variable {
                identifier: String::from("FOO"),
                value: String::from("bar"),
            }),
            Token::Comment(Comment {
                value: String::from("## Print variable."),
                kind: CommentKind::Description,
            }),
            Token::CronJob(CronJob {
                uid: 1,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("* * * * *"),
                command: String::from("echo $FOO"),
                description: Some(JobDescription(String::from("Print variable."))),
                section: None,
            }),
            Token::Comment(Comment {
                value: String::from("# Do nothing (this is a regular comment)."),
                kind: CommentKind::Regular,
            }),
            Token::CronJob(CronJob {
                uid: 2,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@reboot"),
                command: String::from(":"),
                description: None,
                section: None,
            }),
        ]);

        let job = crontab.get_job_from_uid(2).unwrap();
        let command = crontab.make_shell_command(job).unwrap();

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
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("30 9 * * * "),
                command: String::from("echo 'gm'"),
                description: None,
                section: None,
            }),
        ]);

        let job = crontab.get_job_from_uid(1).unwrap();
        let command = crontab.make_shell_command(job).unwrap();

        assert_eq!(
            command.env,
            HashMap::from([(String::from("FOO"), String::from("baz"))])
        );
        assert_eq!(command.command, "echo 'gm'");
    }

    #[test]
    fn run_cron_with_default_shell() {
        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 1,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@reboot"),
            command: String::from("cat a-file.txt"),
            description: None,
            section: None,
        })]);

        let job = crontab.get_job_from_uid(1).unwrap();
        let command = crontab.make_shell_command(job).unwrap();

        assert_eq!(command.shell, DEFAULT_SHELL);
        assert_eq!(command.command, "cat a-file.txt");
    }

    #[test]
    fn run_cron_with_different_shell() {
        let crontab = Crontab::new(vec![
            Token::Variable(Variable {
                identifier: String::from("SHELL"),
                value: String::from("/bin/bash"),
            }),
            Token::CronJob(CronJob {
                uid: 1,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@hourly"),
                command: String::from("echo 'I am echoed by bash!'"),
                description: None,
                section: None,
            }),
        ]);

        let job = crontab.get_job_from_uid(1).unwrap();
        let command = crontab.make_shell_command(job).unwrap();

        assert_eq!(command.env, HashMap::new());
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
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@hourly"),
                command: String::from("echo 'I am echoed by a custom shell!'"),
                description: None,
                section: None,
            }),
        ]);

        let job = crontab.get_job_from_uid(1).unwrap();
        let command = crontab.make_shell_command(job).unwrap();

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
                fingerprint: 13_376_942,
                tag: None,
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
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@hourly"),
                command: String::from("echo 'I am echoed by zsh!'"),
                description: None,
                section: None,
            }),
        ]);

        let job = crontab.get_job_from_uid(2).unwrap();
        let command = crontab.make_shell_command(job).unwrap();

        assert_eq!(command.shell, "/bin/zsh");
        assert_eq!(command.command, "echo 'I am echoed by zsh!'");
    }

    #[test]
    fn run_cron_with_default_home() {
        unsafe {
            env::set_var("HOME", "/home/<default>");
        }

        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 1,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@daily"),
            command: String::from("/usr/bin/bash ~/startup.sh"),
            description: None,
            section: None,
        })]);

        let job = crontab.get_job_from_uid(1).unwrap();
        let command = crontab.make_shell_command(job).unwrap();

        assert_eq!(command.home, "/home/<default>");
    }

    #[test]
    fn run_cron_with_different_home() {
        unsafe {
            env::set_var("HOME", "/home/<default>");
        }

        let crontab = Crontab::new(vec![
            Token::Variable(Variable {
                identifier: String::from("HOME"),
                value: String::from("/home/<custom>"),
            }),
            Token::CronJob(CronJob {
                uid: 1,
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@yearly"),
                command: String::from("./cleanup.sh"),
                description: None,
                section: None,
            }),
        ]);

        let job = crontab.get_job_from_uid(1).unwrap();
        let command = crontab.make_shell_command(job).unwrap();

        assert_eq!(command.env, HashMap::new());
        assert_eq!(command.home, "/home/<custom>");
        assert_eq!(command.command, "./cleanup.sh");
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
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@hourly"),
                command: String::from("echo 'I am echoed in a different Home!'"),
                description: None,
                section: None,
            }),
        ]);

        let job = crontab.get_job_from_uid(1).unwrap();
        let command = crontab.make_shell_command(job).unwrap();

        assert!(!command.env.contains_key("HOME"));
        assert_eq!(command.home, "/home/<custom>");
    }

    #[test]
    fn get_home_directory_error() {
        unsafe {
            env::remove_var("HOME");
        }

        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 1,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@reboot"),
            command: String::from("/usr/bin/bash ~/startup.sh"),
            description: None,
            section: None,
        })]);

        let job = crontab.get_job_from_uid(1).unwrap();
        let error = crontab.make_shell_command(job).unwrap_err();

        assert_eq!(error, "Could not read Home directory from environment.");

        // If we don't re-create it, other tests will fail.
        unsafe {
            env::set_var("HOME", "/home/<test>");
        }
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
                fingerprint: 13_376_942,
                tag: None,
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
                fingerprint: 13_376_942,
                tag: None,
                schedule: String::from("@hourly"),
                command: String::from("echo 'I run is user2's Home!'"),
                description: None,
                section: None,
            }),
        ]);

        let job = crontab.get_job_from_uid(2).unwrap();
        let command = crontab.make_shell_command(job).unwrap();

        assert_eq!(command.home, "/home/user2");
        assert_eq!(command.command, "echo 'I run is user2's Home!'");
    }

    #[test]
    fn run_cron_with_non_existing_job() {
        let crontab = Crontab::new(vec![Token::CronJob(CronJob {
            uid: 1,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@hourly"),
            command: String::from("echo 'I am echoed by bash!'"),
            description: None,
            section: None,
        })]);
        let job_not_in_crontab = CronJob {
            uid: 42,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@never"),
            command: String::from("sleep infinity"),
            description: None,
            section: None,
        };

        let error = crontab.make_shell_command(&job_not_in_crontab).unwrap_err();

        assert_eq!(error, "The given job is not in the crontab.");
    }

    #[test]
    fn to_json() {
        let crontab = Crontab::new(vec![
            Token::Variable(Variable {
                identifier: String::from("HOME"),
                value: String::from("/home/user1"),
            }),
            Token::CronJob(CronJob {
                uid: 1,
                fingerprint: 13_376_942,
                tag: Some(String::from("taggy \"tag\"")),
                schedule: String::from("@daily"),
                command: String::from("/usr/bin/bash ~/startup.sh"),
                description: None,
                section: None,
            }),
            Token::Variable(Variable {
                identifier: String::from("HOME"),
                value: String::from("/home/user2"),
            }),
            Token::CronJob(CronJob {
                uid: 2,
                fingerprint: 17_118_619_922_108_271_534,
                tag: None,
                schedule: String::from("* * * * *"),
                command: String::from("echo \"$FOO\""),
                description: Some(JobDescription(String::from("Print \"variable\"."))),
                section: Some(tokens::JobSection {
                    uid: 1,
                    title: String::from("Some \"testing\" going on here..."),
                }),
            }),
        ]);

        let json = crontab.to_json();

        println!("{}", &json);
        assert_eq!(
            json,
            r#"[{"uid":1,"fingerprint":"cc1dae","tag":"taggy \"tag\"","schedule":"@daily","command":"/usr/bin/bash ~/startup.sh","description":null,"section":null},{"uid":2,"fingerprint":"ed918e1eee304bae","tag":null,"schedule":"* * * * *","command":"echo \"$FOO\"","description":"Print \"variable\".","section":{"uid":1,"title":"Some \"testing\" going on here..."}}]"#
        );
    }
}
