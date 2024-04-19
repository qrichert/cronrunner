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

use crate::ui::color_error;

#[must_use]
pub fn handle_cli_arguments(args: &[String]) -> Option<u8> {
    // Skip 1st argument because it's the path to the executable.
    let mut args = args.iter().skip(1);

    let arg = args.next()?;

    if arg == "-h" || arg == "--help" {
        println!("{}", help_message());
        return Some(0u8);
    }

    if arg == "-v" || arg == "--version" {
        println!("{}", version_message());
        return Some(0u8);
    }

    eprintln!("{}", unexpected_argument_message(arg));
    Some(2u8)
}

fn help_message() -> String {
    [
        format!("{}\n", env!("CARGO_PKG_DESCRIPTION")),
        format!("Usage: {} [OPTIONS]\n", env!("CARGO_BIN_NAME")),
        String::from(
            "
Options:
  -h, --help           Show this message and exit.
  -v, --version        Show the version and exit.

Extras:
  Comments that start with two hashes (##) and immediately precede
  a job are used as description for that job.

      ## Say hello.
      @hourly echo \"hello\"

  This job will be presented like this:

      1. Say hello. @hourly echo \"hello\"
      "
            .trim(),
        ),
    ]
    .join("\n")
}

fn version_message() -> String {
    format!("{} {}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"))
}

fn unexpected_argument_message(arg: &str) -> String {
    [
        format!("{} unexpected argument '{arg}'.", color_error("Error:")),
        format!("Try '{} -h' for help.", env!("CARGO_BIN_NAME")),
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_arguments_because_first_is_skipped() {
        let args: Vec<String> = vec![String::from("/usr/local/bin/cronrunner")];

        let res = handle_cli_arguments(&args);

        assert!(res.is_none());
    }

    #[test]
    fn no_arguments_not_even_executable_path() {
        let args: Vec<String> = Vec::new();

        let res = handle_cli_arguments(&args);

        assert!(res.is_none());
    }

    #[test]
    fn unexpected_argument() {
        let args: Vec<String> = vec![
            String::from("/usr/local/bin/cronrunner"),
            String::from("--unknown"),
        ];

        let res = handle_cli_arguments(&args);

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
        let args: Vec<String> = vec![
            String::from("/usr/local/bin/cronrunner"),
            String::from("--help"),
            String::from("--unknown"),
        ];

        let res = handle_cli_arguments(&args);

        assert_eq!(res, Some(0u8));
    }

    #[test]
    fn argument_help() {
        let args: Vec<String> = vec![
            String::from("/usr/local/bin/cronrunner"),
            String::from("--help"),
        ];

        let res = handle_cli_arguments(&args);

        assert_eq!(res, Some(0u8));
    }

    #[test]
    fn argument_help_shorthand() {
        let args: Vec<String> = vec![
            String::from("/usr/local/bin/cronrunner"),
            String::from("-h"),
        ];

        let res = handle_cli_arguments(&args);

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
        let args: Vec<String> = vec![
            String::from("/usr/local/bin/cronrunner"),
            String::from("--version"),
        ];

        let res = handle_cli_arguments(&args);

        assert_eq!(res, Some(0u8));
    }

    #[test]
    fn argument_version_shorthand() {
        let args: Vec<String> = vec![
            String::from("/usr/local/bin/cronrunner"),
            String::from("-v"),
        ];

        let res = handle_cli_arguments(&args);

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
