use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CronJob {
    /// Unique ID (cronrunner-specific). This matches the job's order of
    /// appearance in the crontab (`1`, `2`, `3`, etc.). Contrary to
    /// `fingerprint` it is not guaranteed to be stable across runs. If
    /// the crontab changes between two runs, the same `uid` may target
    /// a different job(!). This is more user-friendly, however. (See
    /// help text for `--safe` and `--tag` mode).
    pub uid: usize,
    /// Fingerprint (cronrunner-specific). This uniquely identifies a
    /// job, but contrary to `uid`, it is stable across runs. If the
    /// job changes between two runs (position or command changes), the
    /// fingerprint will be invalidated. This is safer, but less
    /// user-friendly (See help text for `--safe` and `--tag` mode).
    pub fingerprint: u64,
    /// Tag (cronrunner-specific). This is a manual job identifier.
    /// Contrary to `fingerprint`, a tag is stable even if the job
    /// changes. This is great for scripts, but it does not guarantee
    /// that the command remains the same. (See help text for `--safe`
    /// and `--tag` mode). This is set by starting the job's description
    /// by `%{...}`, where `...` can be anything but a closing bracket.
    pub tag: Option<String>,
    /// The schedule of the job, as defined in the crontab. This value
    /// isn't used by [`Crontab`](super::Crontab).
    pub schedule: String,
    /// The command of the job, as defined in the crontab. This is what
    /// gets run in [`Crontab::run()`](super::Crontab::run).
    pub command: String,
    /// An optional (cronrunner-specific) description of the job. This
    /// is set by preceding the job with a double-hash (`##`) comment in
    /// the crontab.
    pub description: Option<JobDescription>,
    /// An optional (cronrunner-specific) parent section for the job.
    /// Sections are defined by triple-hash (`###`) comments in the
    /// crontab.
    pub section: Option<JobSection>,
}

impl fmt::Display for CronJob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(description) = &self.description {
            write!(f, "{description}")
        } else {
            write!(f, "{} {}", self.schedule, self.command)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Variable {
    pub identifier: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommentKind {
    Regular,
    Description,
    Section,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Comment {
    pub value: String,
    pub kind: CommentKind,
}

// The reason job descriptions have their own struct is that job
// sections have their own struct, and it feels weird to have one as a
// struct but not the other.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobDescription(pub String);

impl fmt::Display for JobDescription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Job sections have their own struct because we need some way (`uid`)
// to differentiate them even if their content is the same.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobSection {
    pub uid: u32,
    pub title: String,
}

impl fmt::Display for JobSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.title)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unknown {
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Token {
    CronJob(CronJob),
    Variable(Variable),
    Comment(Comment),
    Unknown(Unknown),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cronjob_display_with_description() {
        let cronjob = CronJob {
            uid: 1,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@hourly"),
            command: String::from("sleep 3599"),
            description: Some(JobDescription(String::from("Sleep (almost) forever."))),
            section: None,
        };

        let job_display = cronjob.to_string();

        assert_eq!(job_display, "Sleep (almost) forever.");
    }

    #[test]
    fn cronjob_display_without_description() {
        let cronjob = CronJob {
            uid: 1,
            fingerprint: 13_376_942,
            tag: None,
            schedule: String::from("@hourly"),
            command: String::from("sleep 3599"),
            description: None,
            section: None,
        };

        let job_display = cronjob.to_string();

        assert_eq!(job_display, "@hourly sleep 3599");
    }

    #[test]
    fn job_description_display() {
        let description = JobDescription(String::from("hello, world"));

        assert_eq!(description.to_string(), "hello, world");
    }

    #[test]
    fn job_description_section() {
        let section = JobSection {
            uid: 36,
            title: String::from("foo bar baz"),
        };

        assert_eq!(section.to_string(), "foo bar baz");
    }
}
