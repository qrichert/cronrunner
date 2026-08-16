use std::path::PathBuf;

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
    pub fingerprint: bool,
    pub tag: bool,
    pub detach: bool,
    pub env_file: Option<PathBuf>,
    pub crontab_files: Vec<PathBuf>,
    pub job: Option<Job>,
}

impl Config {
    pub fn build_from_args(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut config = Self::default();

        Self::pre_populate_from_env(&mut config);

        let mut iter = args.skip(1);
        while let Some(arg) = iter.next() {
            if arg == "-h" {
                config.help = true;
                break;
            }
            if arg == "--help" {
                config.long_help = true;
                break;
            }

            if arg == "-V" || arg == "--version" {
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

            if arg == "--fingerprint" {
                config.fingerprint = true;
                continue;
            }

            // TODO: Remove the deprecated `--safe` and `-s` aliases in a future major release.
            if arg == "-s" || arg == "--safe" {
                eprintln!(
                    "{warning}warning{reset}: '{arg}' is deprecated; use '--fingerprint' instead.",
                    warning = ui::Color::maybe_color("\x1b[0;93m"),
                    reset = ui::Color::maybe_color(ui::RESET),
                );

                config.fingerprint = true;
                continue;
            }

            if arg == "-t" || arg == "--tag" {
                config.tag = true;
                continue;
            }

            if arg == "-d" || arg == "--detach" {
                config.detach = true;
                continue;
            }

            if arg == "-e" || arg == "--env" {
                let Some(file) = iter.next().map(PathBuf::from) else {
                    return Err(format!("Expected file path after '{arg}'"));
                };
                #[cfg(not(tarpaulin_include))] // Wrongly marked uncovered.
                {
                    config.env_file = Some(file);
                    continue;
                }
            }

            if arg == "-f" || arg == "--file" {
                let Some(file) = iter.next().map(PathBuf::from) else {
                    return Err(format!("Expected file path after '{arg}'"));
                };
                config.crontab_files.push(file);
                continue;
            }

            if config.tag {
                config.job = Some(Job::Tag(arg));
                break;
            } else if config.fingerprint {
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

            return Err(format!("Unexpected argument '{arg}'"));
        }

        if config.tag && config.job.is_none() {
            return Err(String::from("Option '--tag' requires a tag"));
        }

        Ok(config)
    }

    /// Pre-populate `Config` with values from the environment.
    ///
    /// Some CLI arguments have environment counterparts, whose purpose
    /// is to make long term configuration simpler.
    ///
    /// For example, it's much nicer to export `CRONRUNNER_ENV=<file>`
    /// once in the `~/.bashrc` than to add `--env <file>` to every
    /// command.
    fn pre_populate_from_env(config: &mut Self) {
        // TODO: Remove the deprecated `CRONRUNNER_SAFE` alias in a future major release.
        if std::env::var_os("CRONRUNNER_FINGERPRINT").is_some()
            || std::env::var_os("CRONRUNNER_SAFE").is_some()
        {
            config.fingerprint = true;
        }
        if let Some(env_file) = std::env::var_os("CRONRUNNER_ENV").filter(|f| !f.is_empty()) {
            config.env_file = Some(PathBuf::from(env_file));
        }
    }
}

pub fn help_message() -> String {
    format!(
        "\
{description}

Usage: {bin} [OPTIONS] [ID]

Options:
  -l, --list-only      List available jobs and exit.
      --as-json        Render `--list-only` as JSON.
      --fingerprint    Use job fingerprints.
  -t, --tag <TAG>      Run specific tag.
  -d, --detach         Run job in the background.
  -e, --env <FILE>     Override job environment.
  -f, --file <FILE>    Read jobs from a file (repeatable).

  -h, --help           Show this message and exit.
  -V, --version        Show the version and exit.
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
  Comments that start with two hashes (`##`) and immediately precede
  a job are used as the description for that job.

      {comment}## Say hello.{reset}
      {schedule}@hourly{reset} {command}echo \"hello\"{reset}

  This job will be presented like this:

      {highlight}1.{reset} Say hello. {attenuate}@hourly echo \"hello\"{reset}

  Comments that start with three hashes (`###`) are used as section
  headers, up until a new section starts or up until the end.

      {comment}### Housekeeping{reset}

      {schedule}@daily{reset} {command}docker image prune --force{reset}

  This job will be presented like this:

      {title}Housekeeping{reset}

      {highlight}1.{reset} {attenuate}@daily{reset} docker image prune --force

  Descriptions and sections are independent from one another.

Fingerprints and tags:
  Job IDs are attributed in the order of appearance in the crontab. This
  can be dangerous if used in scripts, because if the crontab changes,
  the wrong job may get run.

  Instead, you can activate `--fingerprint` mode, in which jobs are
  identified by a fingerprint. This is less user-friendly, but if the
  jobs get reordered, or if the command changes, that fingerprint will
  be invalidated and the run will fail.

  Or, you could tag a specific job and run it with `--tag`. Tags are
  stable even if the underlying job changes. This is great for scripts,
  but it does not guarantee that the command remains the same.

  To define a tag, add a description comment starting with `%{{...}}`:

      {comment}## %{{my-tag}} Scriptable job.{reset}
      {schedule}@reboot{reset} {command}/usr/bin/bash ~/startup.sh{reset}

  Then you can run it like this:

      {highlight}${reset} {bin} --tag my-tag
      Running...

Ignore jobs:
  To ignore jobs, tag them with the special `%{{ignore}}` tag:

      {comment}## %{{ignore}} Ignored job.{reset}
      {schedule}@daily{reset} {command}/should/not/be/run/manually{reset}

Environment:
  Cron runs jobs in a very minimalistic environment, which you may want
  to replicate. The content of this environment is platform-specific and
  can vary a lot. The best way to capture it accurately is to export it
  directly from Cron. To do this, let Cron run this job once:

      {comment}## %{{ignore}}{reset}
      {min}*{reset} {h}*{reset} {d}*{reset} {mon}*{reset} {dow}*{reset} {command}env > ~/.cron.env{reset}

  Then, you can tell {package} to use this file as the environment for
  the child process:

      {highlight}${reset} {bin} --env ~/.cron.env 3
      Running...

Crontab source:
  By default, jobs are read from the current user's crontab through
  `crontab -l`. To read it from an arbitrary file, pass `--file`:

      {highlight}${reset} {bin} --file ./crontab.export --list-only
      {highlight}${reset} {bin} -f personal.cron -f project.cron

  If you pass multiple file sources, they are read and run in isolation.
  Variables from one crontab don't leak into the other, and job
  fingerprints remain stable even if you reorder the sources.

Configuration:
  Some arguments have corresponding environment variables, allowing you
  to set values permanently in a shell startup file (e.g., `~/.bashrc`).

      --fingerprint  CRONRUNNER_FINGERPRINT=1
      --env <FILE>   CRONRUNNER_ENV=<FILE>

Tips:
  If you have jobs you only want to execute manually, you can schedule
  them to run on February 31st:

      {min}0{reset} {h}0{reset} {d}31{reset} {mon}2{reset} {dow}*{reset} {command}echo \"I never run on my own!\"{reset}
",
        help = help_message(),
        bin = env!("CARGO_BIN_NAME"),
        package = env!("CARGO_PKG_NAME"),
        comment = ui::Color::maybe_color("\x1b[96m"),
        schedule = ui::Color::maybe_color("\x1b[38;5;224m"),
        min = ui::Color::maybe_color("\x1b[95m"),
        h = ui::Color::maybe_color("\x1b[38;5;81m"),
        d = ui::Color::maybe_color("\x1b[38;5;121m"),
        mon = ui::Color::maybe_color("\x1b[95m"),
        dow = ui::Color::maybe_color("\x1b[96m"),
        command = ui::Color::maybe_color("\x1b[93m"),
        title = ui::Color::maybe_color(ui::TITLE),
        highlight = ui::Color::maybe_color(ui::HIGHLIGHT),
        attenuate = ui::Color::maybe_color(ui::ATTENUATE),
        reset = ui::Color::maybe_color(ui::RESET),
    )
}

pub fn version_message() -> String {
    format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
}

pub fn bad_arguments_error_message(reason: &str) -> String {
    format!(
        "\
{error}: {reason}.
Try '{bin} -h' for help.",
        error = ui::Color::error("error"),
        bin = env!("CARGO_BIN_NAME"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, iter};

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
                fingerprint: false,
                tag: false,
                detach: false,
                env_file: None,
                crontab_files: Vec::new(),
                job: None,
            }
        );
    }

    #[test]
    fn no_arguments_because_first_is_skipped() {
        let args = iter::once(String::from("/usr/local/bin/crn"));

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
            String::from("/usr/local/bin/crn"),
            String::from("--unknown"),
        ]
        .into_iter();

        let err = Config::build_from_args(args).unwrap_err();

        assert_eq!(err, "Unexpected argument '--unknown'");
    }

    #[test]
    fn unexpected_argument_message_contains_argument_and_help() {
        let message = bad_arguments_error_message("<reason>");

        dbg!(&message);
        assert!(message.contains("error"));
        assert!(message.contains("<reason>"));
        assert!(message.contains("-h"));
    }

    #[test]
    fn argument_help() {
        let args = [String::from("/usr/local/bin/crn"), String::from("-h")].into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.help);
        assert!(!config.long_help);
    }

    #[test]
    fn argument_help_stops_after_match() {
        let args = [
            String::from("/usr/local/bin/crn"),
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
        assert!(message.contains("-V, --version"));
        assert!(message.contains("-l, --list-only"));
        assert!(message.contains("--as-json"));
        assert!(message.contains("--fingerprint"));
        assert!(message.contains("-t, --tag"));
        assert!(message.contains("-d, --detach"));
        assert!(message.contains("-e, --env <FILE>"));
        assert!(message.contains("-f, --file <FILE>"));
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
        let args = [String::from("/usr/local/bin/crn"), String::from("--help")].into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.long_help);
        assert!(!config.help);
    }

    #[test]
    fn argument_long_help_stops_after_match() {
        let args = [
            String::from("/usr/local/bin/crn"),
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
        assert!(message.contains("-V, --version"));
    }

    #[test]
    fn argument_version() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("--version"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.version);
    }

    #[test]
    fn argument_version_shorthand() {
        let args = [String::from("/usr/local/bin/crn"), String::from("-V")].into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.version);
    }

    #[test]
    fn argument_version_lowercase_shorthand_is_rejected() {
        let args = [String::from("/usr/local/bin/crn"), String::from("-v")].into_iter();

        let err = Config::build_from_args(args).unwrap_err();

        assert_eq!(err, "Unexpected argument '-v'");
    }

    #[test]
    fn argument_version_stops_after_match() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("--version"),
            String::from("--unknown"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.version);
    }

    #[test]
    fn version_message_contains_package_name_and_version() {
        let message = version_message();

        dbg!(&message);
        assert!(message.contains(env!("CARGO_PKG_NAME")));
        assert!(message.contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn argument_list_only() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("--list-only"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.list_only);
    }

    #[test]
    fn argument_list_only_shorthand() {
        let args = [String::from("/usr/local/bin/crn"), String::from("-l")].into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.list_only);
    }

    #[test]
    fn argument_list_only_continues_after_match() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("--list-only"),
            String::from("--fingerprint"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.list_only);
        assert!(config.fingerprint);
    }

    #[test]
    fn argument_as_json() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("--as-json"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.as_json);
    }

    #[test]
    fn argument_as_json_continues_after_match() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("--as-json"),
            String::from("--fingerprint"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.as_json);
        assert!(config.fingerprint);
    }

    #[test]
    fn argument_as_json_implicitly_activates_list_only() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("--list-only"),
            String::from("--as-json"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.list_only);
        assert!(config.as_json);
    }

    #[test]
    fn argument_fingerprint() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("--fingerprint"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.fingerprint);
    }

    #[test]
    fn argument_safe_compatibility_alias() {
        let args = [String::from("/usr/local/bin/crn"), String::from("--safe")].into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.fingerprint);
    }

    #[test]
    fn argument_safe_shorthand_compatibility_alias() {
        let args = [String::from("/usr/local/bin/crn"), String::from("-s")].into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.fingerprint);
    }

    #[test]
    fn argument_fingerprint_continues_after_match() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("--fingerprint"),
            String::from("1337f"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.fingerprint);
        assert!(matches!(config.job, Some(Job::Fingerprint(78_719))));
    }

    #[test]
    fn argument_fingerprint_from_env() {
        unsafe {
            env::set_var("CRONRUNNER_FINGERPRINT", "");
        }

        let args = iter::once(String::from("/usr/local/bin/crn"));

        let config = Config::build_from_args(args).unwrap();

        // If we don't remove it, it will make tests expecting it to be
        // `false` fail (remove _before_ `assert` returns early).
        unsafe {
            env::remove_var("CRONRUNNER_FINGERPRINT");
        }

        assert!(config.fingerprint);
    }

    #[test]
    fn argument_safe_compatibility_alias_from_env() {
        unsafe {
            env::set_var("CRONRUNNER_SAFE", "");
        }

        let args = iter::once(String::from("/usr/local/bin/crn"));

        let config = Config::build_from_args(args).unwrap();

        // If we don't remove it, it will make tests expecting it to be
        // `false` fail (remove _before_ `assert` returns early).
        unsafe {
            env::remove_var("CRONRUNNER_SAFE");
        }

        assert!(config.fingerprint);
    }

    #[test]
    fn argument_tag() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("--tag"),
            String::from("my-tag"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.tag);
        assert_eq!(config.job.unwrap(), Job::Tag(String::from("my-tag")));
    }

    #[test]
    fn argument_tag_shorthand() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("-t"),
            String::from("your-tag"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.tag);
        assert_eq!(config.job.unwrap(), Job::Tag(String::from("your-tag")));
    }

    #[test]
    fn argument_tag_continues_after_match() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("--tag"),
            String::from("--detach"),
            String::from("taginou"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.tag);
        assert!(config.detach);
        assert_eq!(config.job.unwrap(), Job::Tag(String::from("taginou")));
    }

    #[test]
    fn argument_tag_not_followed_by_tag() {
        let args = [String::from("/usr/local/bin/crn"), String::from("--tag")].into_iter();

        let err = Config::build_from_args(args).unwrap_err();

        assert_eq!(err, "Option '--tag' requires a tag");
    }

    #[test]
    fn argument_detach() {
        let args = [String::from("/usr/local/bin/crn"), String::from("--detach")].into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.detach);
    }

    #[test]
    fn argument_detach_shorthand() {
        let args = [String::from("/usr/local/bin/crn"), String::from("-d")].into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.detach);
    }

    #[test]
    fn argument_detach_continues_after_match() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("--detach"),
            String::from("42"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.detach);
        assert!(matches!(config.job, Some(Job::Uid(42))));
    }

    #[test]
    fn argument_env() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("--env"),
            String::from("~/.cron.env"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(
            config
                .env_file
                .is_some_and(|contents| *contents == *"~/.cron.env")
        );
    }

    #[test]
    fn argument_env_shorthand() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("-e"),
            String::from("~/.cron.env"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(
            config
                .env_file
                .is_some_and(|contents| *contents == *"~/.cron.env")
        );
    }

    #[test]
    fn argument_env_continues_after_match() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("--env"),
            String::from("~/.cron.env"),
            String::from("42"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(config.env_file.is_some());
        assert!(matches!(config.job, Some(Job::Uid(42))));
    }

    #[test]
    fn argument_env_requires_file() {
        let args = [String::from("/usr/local/bin/crn"), String::from("--env")].into_iter();

        let err = Config::build_from_args(args).unwrap_err();

        assert_eq!(err, "Expected file path after '--env'");
    }

    #[test]
    fn argument_file() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("--file"),
            String::from("./personal.cron"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert_eq!(config.crontab_files, [PathBuf::from("./personal.cron")]);
    }

    #[test]
    fn argument_file_shorthand() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("-f"),
            String::from("./personal.cron"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert_eq!(config.crontab_files, [PathBuf::from("./personal.cron")]);
    }

    #[test]
    fn argument_file_is_repeatable_and_continues_after_match() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("--file"),
            String::from("personal.cron"),
            String::from("-f"),
            String::from("project.cron"),
            String::from("--list-only"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert_eq!(
            config.crontab_files,
            [
                PathBuf::from("personal.cron"),
                PathBuf::from("project.cron")
            ]
        );
        assert!(config.list_only);
    }

    #[test]
    fn argument_file_requires_file() {
        let args = [String::from("/usr/local/bin/crn"), String::from("--file")].into_iter();

        let err = Config::build_from_args(args).unwrap_err();

        assert_eq!(err, "Expected file path after '--file'");
    }

    #[test]
    fn argument_env_file_from_env() {
        unsafe {
            env::set_var("CRONRUNNER_ENV", "~/.cron.env");
        }

        let args = iter::once(String::from("/usr/local/bin/crn"));

        let config = Config::build_from_args(args).unwrap();

        // If we don't remove it, it will make tests expecting it to be
        // `false` fail (remove _before_ `assert` returns early).
        unsafe {
            env::remove_var("CRONRUNNER_ENV");
        }

        assert!(
            config
                .env_file
                .is_some_and(|contents| *contents == *"~/.cron.env")
        );
    }

    #[test]
    fn argument_env_file_from_env_empty_file_name_is_ignored() {
        unsafe {
            env::set_var("CRONRUNNER_ENV", "");
        }

        let args = iter::once(String::from("/usr/local/bin/crn"));

        let config = Config::build_from_args(args).unwrap();

        // If we don't remove it, it will make tests expecting it to be
        // `false` fail (remove _before_ `assert` returns early).
        unsafe {
            env::remove_var("CRONRUNNER_ENV");
        }

        assert!(config.env_file.is_none());
    }

    #[test]
    fn argument_job() {
        let args = [String::from("/usr/local/bin/crn"), String::from("42")].into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(matches!(config.job, Some(Job::Uid(42))));
    }

    #[test]
    fn argument_job_stops_after_match() {
        let args = [
            String::from("/usr/local/bin/crn"),
            String::from("42"),
            String::from("--version"),
        ]
        .into_iter();

        let config = Config::build_from_args(args).unwrap();

        assert!(matches!(config.job, Some(Job::Uid(42))));
        assert!(!config.version);
    }
}
