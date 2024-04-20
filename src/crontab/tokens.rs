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

use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CronJob {
    pub uid: u32,
    pub schedule: String,
    pub command: String,
    // TODO(refactor): Put these in a `metadata` struct.
    pub description: Option<String>,
    pub section: Option<String>,
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
            schedule: String::from("@hourly"),
            command: String::from("sleep 3599"),
            description: Some(String::from("Sleep (almost) forever.")),
            section: None,
        };

        let job_display = cronjob.to_string();

        assert_eq!(job_display, "Sleep (almost) forever.");
    }

    #[test]
    fn cronjob_display_without_description() {
        let cronjob = CronJob {
            uid: 1,
            schedule: String::from("@hourly"),
            command: String::from("sleep 3599"),
            description: None,
            section: None,
        };

        let job_display = cronjob.to_string();

        assert_eq!(job_display, "@hourly sleep 3599");
    }
}
