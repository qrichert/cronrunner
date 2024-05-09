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

use crate::ui;

#[must_use]
pub fn handle_cli_arguments(mut args: impl Iterator<Item = String>) -> Option<u8> {
    args.next(); // Skip path to executable.

    let arg = args.next()?; // or `None`.

    if arg == "-h" || arg == "--help" {
        println!("{}", help_message());
        return Some(0u8);
    }

    if arg == "-v" || arg == "--version" {
        println!("{}", version_message());
        return Some(0u8);
    }

    eprintln!("{}", unexpected_argument_message(&arg));
    Some(2u8)
}

fn help_message() -> String {
    [
        format!("{}\n", env!("CARGO_PKG_DESCRIPTION")),
        format!("Usage: {} [OPTIONS]\n", env!("CARGO_BIN_NAME")),
        format!(
            "
Options:
  -h, --help           Show this message and exit.
  -v, --version        Show the version and exit.

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
            comment = "\x1b[96m",
            schedule = "\x1b[38;5;224m",
            command = "\x1b[93m",
            title = ui::TITLE,
            highlight = ui::HIGHLIGHT,
            attenuate = ui::ATTENUATE,
            reset = ui::RESET
        )
        .trim()
        .to_string(),
    ]
    .join("\n")
}

fn version_message() -> String {
    format!("{} {}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"))
}

fn unexpected_argument_message(arg: &str) -> String {
    [
        format!("{} unexpected argument '{arg}'.", ui::color_error("Error:")),
        format!("Try '{} -h' for help.", env!("CARGO_BIN_NAME")),
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_arguments_because_first_is_skipped() {
        let args = std::iter::once(String::from("/usr/local/bin/cronrunner"));

        let res = handle_cli_arguments(args);

        assert!(res.is_none());
    }

    #[test]
    fn no_arguments_not_even_executable_path() {
        let args = std::iter::empty();

        let res = handle_cli_arguments(args);

        assert!(res.is_none());
    }

    #[test]
    fn unexpected_argument() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--unknown"),
        ]
        .into_iter();

        let res = handle_cli_arguments(args);

        assert_eq!(res, Some(2u8));
    }

    #[test]
    fn unexpected_argument_message_contains_argument_and_help() {
        let message = unexpected_argument_message("--unexpected");

        dbg!(&message);
        assert!(message.contains("--unexpected"));
        assert!(message.contains("-h"));
    }

    #[test]
    fn stops_after_first_argument_match() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--help"),
            String::from("--unknown"),
        ]
        .into_iter();

        let res = handle_cli_arguments(args);

        assert_eq!(res, Some(0u8));
    }

    #[test]
    fn argument_help() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("--help"),
        ]
        .into_iter();

        let res = handle_cli_arguments(args);

        assert_eq!(res, Some(0u8));
    }

    #[test]
    fn argument_help_shorthand() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("-h"),
        ]
        .into_iter();

        let res = handle_cli_arguments(args);

        assert_eq!(res, Some(0u8));
    }

    #[test]
    fn argument_help_message_contains_options() {
        let message = help_message();

        dbg!(&message);
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

        let res = handle_cli_arguments(args);

        assert_eq!(res, Some(0u8));
    }

    #[test]
    fn argument_version_shorthand() {
        let args = [
            String::from("/usr/local/bin/cronrunner"),
            String::from("-v"),
        ]
        .into_iter();

        let res = handle_cli_arguments(args);

        assert_eq!(res, Some(0u8));
    }

    #[test]
    fn argument_version_message_contains_binary_name_and_version() {
        let message = version_message();

        dbg!(&message);
        assert!(message.contains(env!("CARGO_BIN_NAME")));
        assert!(message.contains(env!("CARGO_PKG_VERSION")));
    }
}
