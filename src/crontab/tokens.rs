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

#[derive(Clone, Debug, PartialEq)]
pub struct CronJob {
    pub uid: u32,
    pub schedule: String,
    pub command: String,
    pub description: String,
}

impl fmt::Display for CronJob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.description.is_empty() {
            write!(f, "{} {}", self.schedule, self.command)
        } else {
            write!(f, "{}", self.description)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Variable {
    pub identifier: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Comment {
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Unknown {
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
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
            description: String::from("Sleep (almost) forever."),
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
            description: String::new(),
        };

        let job_display = cronjob.to_string();

        assert_eq!(job_display, "@hourly sleep 3599");
    }
}
