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

use super::job::Job;
use super::ui;

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default, Eq, PartialEq)]
pub struct Config {
    pub help: bool,
    pub long_help: bool,
    pub version: bool,
    pub list_only: bool,
    pub as_json: bool,
    pub safe: bool,
    pub detach: bool,
    pub job: Option<Job>,
}

impl Config {
    pub fn build_from_args(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut config = Self::default();

        for arg in args.skip(1) {
            if arg == "-h" {
                config.help = true;
                break;
            }
            if arg == "--help" {
                config.long_help = true;
                break;
            }

            if arg == "-v" || arg == "--version" {
                config.version = true;
                break;
            }

            if arg == "-l" || arg == "--list-only" {
                config.list_only = true;
                continue;
            }

            if arg == "--as-json" {
                config.list_only = true;
                config.as_json = true;
                continue;
            }

            if arg == "-s" || arg == "--safe" {
                config.safe = true;
                continue;
            }

            if arg == "-d" || arg == "--detach" {
                config.detach = true;
                continue;
            }

            if config.safe {
                // Check for fingerprint.
                if let Ok(job) = u64::from_str_radix(&arg, 16) {
                    #[cfg(not(tarpaulin_include))] // Wrongly marked uncovered.
                    {
                        config.job = Some(Job::Fingerprint(job));
                        break;
                    }
                }
            } else if let Ok(job) = arg.parse::<usize>() {
                // Check for UID.
                #[cfg(not(tarpaulin_include))] // Wrongly marked uncovered.
                {
                    config.job = Some(Job::Uid(job));
                    break;
                }
            }

            return Err(arg);
        }

        Ok(config)
    }
}

pub fn help_message() -> String {
    format!(
        "\
{description}

Usage: {bin} [OPTIONS] [ID]

Options:
  -h, --help           Show this message and exit.
  -v, --version        Show the version and exit.
  -l, --list-only      List available jobs and exit.
      --as-json        Render `--list-only` as JSON.
  -s, --safe           Use job fingerprints.
  -d, --detach         Run job in the background.
",
        description = env!("CARGO_PKG_DESCRIPTION"),
        bin = env!("CARGO_BIN_NAME"),
    )
}

pub fn longer_help_notice() -> String {
    format!(
        "For full help, see `{bin} --help`.",
        bin = env!("CARGO_BIN_NAME")
    )
}

pub fn long_help_message() -> String {
    format!(
        "\
{help}
Examples:
  If you know the ID of a job, you can run it directly:

      {attenuate}# Run job number 1.{reset}
      {highlight}${reset} {bin} 1
      Running...

  If the job takes a long time to run, you can detach it:

      {attenuate}# Prints the PID and exits.{reset}
      {highlight}${reset} {bin} --detach 3
      1337
      {highlight}${reset} _

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

Safe mode:
  Job IDs are attributed in the order of appearance in the crontab. This
  can be dangerous if used in scripts, because if the crontab changes,
  the wrong job may get run.

  Instead, you can activate `--safe` mode, in which jobs are identified
  by a fingerprint. This is less user-friendly, but if the jobs get
  reordered, or if the command changes, that fingerprint will be
  invalidated and the run will fail.
",
        help = help_message(),
        bin = env!("CARGO_BIN_NAME"),
        comment = ui::Color::maybe_color("\x1b[96m"),
        schedule = ui::Color::maybe_color("\x1b[38;5;224m"),
        command = ui::Color::maybe_color("\x1b[93m"),
        title = ui::Color::maybe_color(ui::TITLE),
        highlight = ui::Color::maybe_color(ui::HIGHLIGHT),
        attenuate = ui::Color::maybe_color(ui::ATTENUATE),
        reset = ui::Color::maybe_color(ui::RESET),
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
        error = ui::Color::error("Error:"),
        bin = env!("CARGO_BIN_NAME"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::iter;

    #[test]
    fn default_config() {
        assert_eq!(
            Config::default(),
            Config {
                help: false,
                long_help: false,
                version: false,
                list_only: false,
                as_json: false,
                safe: false,
                detach: false,
                job: None,
            }
        );
    }

    #[test]
    fn no_arguments_because_first_is_skipped() {
        let args = iter::once(String::from("/usr/local/bin/cronrunner"));

        let config = Config::build_from_args(args).unwrap();

        assert_eq!(config, Config::default());
    }

    #[test]
    fn no_arguments_not_even_executable_path() {
        let args = iter::empty();

        let config = Config::build_from_args(args).unwrap();

        assert_eq!(config, Config::default());
    }

    #[test]
    fn unexpected_argument() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--unknown"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap_err();

        assert_eq!(config, "--unknown");
    }

    #[test]
    fn unexpected_argument_message_contains_argument_and_help() {
        let message = unexpected_argument_error_message("--unexpected");

        dbg!(&message);
        assert!(message.contains("Error:"));
        assert!(message.contains("--unexpected"));
        assert!(message.contains("-h"));
    }

    #[test]
    fn argument_help() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("-h"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.help);
        assert!(!config.long_help);
    }

    #[test]
    fn argument_help_stops_after_match() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("-h"),
            String::from("--unknown"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.help);
    }

    #[test]
    fn help_message_contains_bin_name_and_options() {
        let message = help_message();

        dbg!(&message);
        assert!(message.contains(env!("CARGO_BIN_NAME")));
        assert!(message.contains("-h, --help"));
        assert!(message.contains("-v, --version"));
        assert!(message.contains("-l, --list-only"));
        assert!(message.contains("--as-json"));
        assert!(message.contains("-s, --safe"));
        assert!(message.contains("-d, --detach"));
    }

    #[test]
    fn longer_help_notice_contains_bin_and_arg() {
        let message = longer_help_notice();

        dbg!(&message);
        assert!(message.contains(env!("CARGO_BIN_NAME")));
        assert!(message.contains("--help"));
    }

    #[test]
    fn argument_long_help() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--help"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.long_help);
        assert!(!config.help);
    }

    #[test]
    fn argument_long_help_stops_after_match() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--help"),
            String::from("--unknown"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.long_help);
    }

    #[test]
    fn long_help_message_contains_short_help_message() {
        let message = long_help_message();

        dbg!(&message);
        assert!(message.contains(env!("CARGO_BIN_NAME")));
        assert!(message.contains("-h, --help"));
        assert!(message.contains("-v, --version"));
    }

    #[test]
    fn argument_version() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--version"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.version);
    }

    #[test]
    fn argument_version_shorthand() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("-v"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

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

        let config = Config::build_from_args(args).unwrap();

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
    fn argument_list_only() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--list-only"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.list_only);
    }

    #[test]
    fn argument_list_only_shorthand() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("-l"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.list_only);
    }

    #[test]
    fn argument_list_only_continues_after_match() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--list-only"),
            String::from("--safe"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.list_only);
        assert!(config.safe);
    }

    #[test]
    fn argument_as_json() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--as-json"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.as_json);
    }

    #[test]
    fn argument_as_json_continues_after_match() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--as-json"),
            String::from("--safe"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.as_json);
        assert!(config.safe);
    }

    #[test]
    fn argument_as_json_implicitly_activates_list_only() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--list-only"),
            String::from("--as-json"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.list_only);
        assert!(config.as_json);
    }

    #[test]
    fn argument_safe() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--safe"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.safe);
    }

    #[test]
    fn argument_safe_shorthand() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("-s"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.safe);
    }

    #[test]
    fn argument_safe_continues_after_match() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--safe"),
            String::from("1337f"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.safe);
        assert!(matches!(config.job, Some(Job::Fingerprint(78_719))));
    }

    #[test]
    fn argument_detach() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--detach"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.detach);
    }

    #[test]
    fn argument_detach_shorthand() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("-d"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

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

        let config = Config::build_from_args(args).unwrap();

        assert!(config.detach);
        assert!(matches!(config.job, Some(Job::Uid(42))));
    }

    #[test]
    fn argument_job() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("42"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(matches!(config.job, Some(Job::Uid(42))));
    }

    #[test]
    fn argument_job_stops_after_match() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("42"),
            String::from("--version"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(matches!(config.job, Some(Job::Uid(42))));
        assert!(!config.version);
    }
}
