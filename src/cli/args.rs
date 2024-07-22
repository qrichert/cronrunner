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

use super::ui;

#[derive(Debug, Eq, PartialEq)]
pub struct Config {
    pub help: bool,
    pub version: bool,
    pub detach: bool,
    pub job: Option<u32>,
}

impl Config {
    pub fn new() -> Self {
        Self {
            help: false,
            version: false,
            detach: false,
            job: None,
        }
    }

    pub fn make_from_args(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut config = Self::new();

        #[allow(clippy::never_loop)]
        for arg in args.skip(1) {
            if arg == "-h" || arg == "--help" {
                config.help = true;
                break;
            }

            if arg == "-v" || arg == "--version" {
                config.version = true;
                break;
            }

            if arg == "-d" || arg == "--detach" {
                config.detach = true;
                continue;
            }

            if let Ok(job) = arg.parse::<u32>() {
                #[cfg(not(tarpaulin_include))] // Wrongly marked uncovered.
                {
                    config.job = Some(job);
                    continue;
                }
            }

            return Err(arg);
        }

        Ok(config)
    }
}

// TODO: Split extras out to --help --verbose or something
pub fn help_message() -> String {
    format!(
        "\
{description}

Usage: {bin} [OPTIONS] [ID]

Options:
  -h, --help           Show this message and exit.
  -v, --version        Show the version and exit.
  -d, --detach         Run job in background.

Extras:
  Comments that start with two hashes (##) and immediately precede
  a job are used as description for that job.

      {comment}## Say hello.{reset}
      {schedule}@hourly{reset} {command}echo \"hello\"{reset}

  This job will be presented like this:

      {highlight}1.{reset} Say hello. {attenuate}@hourly echo \"hello\"{reset}

  Comments that start with three hashes (###) are used as section
  headers, up until a new section starts or up until the end.

      {comment}### Housekeeping{reset}

      {schedule}@daily{reset} {command}docker image prune --force{reset}

  This job will be presented like this:

      {title}Housekeeping{reset}

      {highlight}1.{reset} {attenuate}@daily{reset} docker image prune --force

  Descriptions and sections are independent from one another.
",
        description = env!("CARGO_PKG_DESCRIPTION"),
        bin = env!("CARGO_BIN_NAME"),
        comment = "\x1b[96m",
        schedule = "\x1b[38;5;224m",
        command = "\x1b[93m",
        title = ui::TITLE,
        highlight = ui::HIGHLIGHT,
        attenuate = ui::ATTENUATE,
        reset = ui::RESET
    )
}

pub fn version_message() -> String {
    format!("{} {}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"))
}

pub fn unexpected_argument_error_message(arg: &str) -> String {
    format!(
        "\
{error} unexpected argument '{arg}'.
Try '{bin} -h' for help.",
        error = ui::color_error("Error:"),
        bin = env!("CARGO_BIN_NAME"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_arguments_because_first_is_skipped() {
        let args = std::iter::once(String::from("/usr/local/bin/cronrunner"));

        let config = Config::make_from_args(args).unwrap();

        assert_eq!(config, Config::new());
    }

    #[test]
    fn no_arguments_not_even_executable_path() {
        let args = std::iter::empty();

        let config = Config::make_from_args(args).unwrap();

        assert_eq!(config, Config::new());
    }

    #[test]
    fn unexpected_argument() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--unknown"),
        ]
        .into_iter();

        let config = Config::make_from_args(args).err().unwrap();

        assert_eq!(config, "--unknown");
    }

    #[test]
    fn unexpected_argument_message_contains_argument_and_help() {
        let message = unexpected_argument_error_message("--unexpected");

        dbg!(&message);
        assert!(message.contains("--unexpected"));
        assert!(message.contains("-h"));
    }

    #[test]
    fn argument_help() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--help"),
        ]
        .into_iter();

        let config = Config::make_from_args(args).unwrap();

        assert!(config.help);
    }

    #[test]
    fn argument_help_shorthand() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("-h"),
        ]
        .into_iter();

        let config = Config::make_from_args(args).unwrap();

        assert!(config.help);
    }

    #[test]
    fn argument_help_stops_after_match() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--help"),
            String::from("--unknown"),
        ]
        .into_iter();

        let config = Config::make_from_args(args).unwrap();

        assert!(config.help);
    }

    #[test]
    fn help_message_contains_bin_name_and_options() {
        let message = help_message();

        dbg!(&message);
        assert!(message.contains(env!("CARGO_BIN_NAME")));
        assert!(message.contains("-h, --help"));
        assert!(message.contains("-v, --version"));
        assert!(message.contains("-d, --detach"));
    }

    #[test]
    fn argument_version() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--version"),
        ]
        .into_iter();

        let config = Config::make_from_args(args).unwrap();

        assert!(config.version);
    }

    #[test]
    fn argument_version_shorthand() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("-v"),
        ]
        .into_iter();

        let config = Config::make_from_args(args).unwrap();

        assert!(config.version);
    }

    #[test]
    fn argument_version_stops_after_match() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--version"),
            String::from("--unknown"),
        ]
        .into_iter();

        let config = Config::make_from_args(args).unwrap();

        assert!(config.version);
    }

    #[test]
    fn version_message_contains_binary_name_and_version() {
        let message = version_message();

        dbg!(&message);
        assert!(message.contains(env!("CARGO_BIN_NAME")));
        assert!(message.contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn argument_detach() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--detach"),
        ]
        .into_iter();

        let config = Config::make_from_args(args).unwrap();

        assert!(config.detach);
    }

    #[test]
    fn argument_detach_shorthand() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("-d"),
        ]
        .into_iter();

        let config = Config::make_from_args(args).unwrap();

        assert!(config.detach);
    }

    #[test]
    fn argument_detach_continues_after_match() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--detach"),
            String::from("42"),
        ]
        .into_iter();

        let config = Config::make_from_args(args).unwrap();

        assert!(config.detach);
        assert!(matches!(config.job, Some(42)));
    }

    #[test]
    fn argument_job() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("42"),
        ]
        .into_iter();

        let config = Config::make_from_args(args).unwrap();

        assert!(matches!(config.job, Some(42)));
    }

    #[test]
    fn argument_job_continues_after_match() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("42"),
            String::from("--version"),
        ]
        .into_iter();

        let config = Config::make_from_args(args).unwrap();

        assert!(matches!(config.job, Some(42)));
        assert!(config.version);
    }
}
