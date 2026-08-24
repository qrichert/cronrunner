use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/");
const TEMP_DIR: &str = env!("CARGO_TARGET_TMPDIR");

fn crn() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_crn"));
    command
        .env("NO_COLOR", "1")
        .env_remove("CRONRUNNER_ENV")
        .env_remove("CRONRUNNER_FINGERPRINT")
        .env_remove("CRONRUNNER_SAFE")
        .env_remove("FIRST_ONLY")
        .stdin(Stdio::null());
    command
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn fixture(name: &str) -> PathBuf {
    Path::new(FIXTURES_DIR).join(name)
}

fn temporary_directory(name: &str) -> PathBuf {
    let directory = Path::new(TEMP_DIR).join(name);
    fs::create_dir_all(&directory).unwrap();
    directory
}

fn fingerprint_for(output: &Output, command: &str) -> String {
    stdout(output)
        .lines()
        .find(|line| line.contains(command))
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_string()
}

#[test]
fn fingerprint_selection_compatibility_aliases_match_canonical_menu() {
    let file = fixture("crontab_file_one.cron");
    let file = file.to_str().unwrap();

    let canonical = crn()
        .args(["--fingerprint", "--file", file, "--list-only"])
        .output()
        .unwrap();
    let safe = crn()
        .args(["--safe", "--file", file, "--list-only"])
        .output()
        .unwrap();
    let safe_shorthand = crn()
        .args(["-s", "--file", file, "--list-only"])
        .output()
        .unwrap();
    let canonical_environment = crn()
        .env("CRONRUNNER_FINGERPRINT", "1")
        .args(["--file", file, "--list-only"])
        .output()
        .unwrap();
    let safe_environment = crn()
        .env("CRONRUNNER_SAFE", "1")
        .args(["--file", file, "--list-only"])
        .output()
        .unwrap();

    let canonical_stdout = stdout(&canonical);
    for output in [
        &canonical,
        &safe,
        &safe_shorthand,
        &canonical_environment,
        &safe_environment,
    ] {
        assert!(output.status.success(), "{}", stderr(output));
        assert_eq!(stdout(output), canonical_stdout);
    }

    assert!(stderr(&canonical).is_empty());
    assert_eq!(
        stderr(&safe),
        "warning: '--safe' is deprecated; use '--fingerprint' instead.\n"
    );
    assert_eq!(
        stderr(&safe_shorthand),
        "warning: '-s' is deprecated; use '--fingerprint' instead.\n"
    );
    assert!(stderr(&canonical_environment).is_empty());
    assert!(stderr(&safe_environment).is_empty());
}

#[test]
fn lists_jobs_from_multiple_files_without_invoking_crontab() {
    let first = fixture("crontab_file_one.cron");
    let second = fixture("crontab_file_two.cron");

    let output = crn()
        .env("PATH", "")
        .args(["--file", first.to_str().unwrap()])
        .args(["--file", second.to_str().unwrap(), "--list-only"])
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    let stdout = stdout(&output);
    assert!(stdout.contains("1. First file job."));
    assert!(stdout.contains("2. Second file job."));
    assert!(stdout.contains("1. First file job. @daily :\n\n2. Second file job."));
}

#[test]
fn user_includes_live_crontab_before_explicit_files() {
    let bin_directory = temporary_directory("user_with_file");
    let mock = bin_directory.join("crontab");
    fs::copy(fixture("crontab_example.sh"), &mock).unwrap();
    let mut permissions = fs::metadata(&mock).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&mock, permissions).unwrap();
    let file = fixture("crontab_file_one.cron");

    let output = crn()
        .env("PATH", format!("{}:/bin:/usr/bin", bin_directory.display()))
        .args(["--user", "--file", file.to_str().unwrap(), "--list-only"])
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    let stdout = stdout(&output);
    let user_position = stdout.find("$HOME/bin/daily.job").unwrap();
    let file_position = stdout.find("First file job.").unwrap();
    assert!(user_position < file_position, "{stdout}");
    assert!(stdout.contains("6. First file job."), "{stdout}");
}

#[test]
fn sources_follow_command_line_order_when_file_precedes_user() {
    let bin_directory = temporary_directory("file_before_user");
    let mock = bin_directory.join("crontab");
    fs::copy(fixture("crontab_example.sh"), &mock).unwrap();
    let mut permissions = fs::metadata(&mock).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&mock, permissions).unwrap();
    let file = fixture("crontab_file_one.cron");

    let output = crn()
        .env("PATH", format!("{}:/bin:/usr/bin", bin_directory.display()))
        .args(["--file", file.to_str().unwrap(), "--user", "--list-only"])
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    let stdout = stdout(&output);
    let file_position = stdout.find("First file job.").unwrap();
    let user_position = stdout.find("$HOME/bin/daily.job").unwrap();
    assert!(file_position < user_position, "{stdout}");
    assert!(stdout.contains("1. First file job."), "{stdout}");
}

#[test]
fn user_alone_matches_the_default_source() {
    let bin_directory = temporary_directory("user_matches_default");
    let mock = bin_directory.join("crontab");
    fs::copy(fixture("crontab_example.sh"), &mock).unwrap();
    let mut permissions = fs::metadata(&mock).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&mock, permissions).unwrap();
    let path = format!("{}:/bin:/usr/bin", bin_directory.display());

    let default = crn()
        .env("PATH", &path)
        .arg("--list-only")
        .output()
        .unwrap();
    let explicit = crn()
        .env("PATH", path)
        .args(["--user", "--list-only"])
        .output()
        .unwrap();

    assert!(default.status.success(), "{}", stderr(&default));
    assert!(explicit.status.success(), "{}", stderr(&explicit));
    assert_eq!(stdout(&explicit), stdout(&default));
}

#[test]
fn system_file_lists_jobs_with_their_user() {
    let file = fixture("crontab_system.cron");

    let output = crn()
        .env("PATH", "")
        .args(["--system-file", file.to_str().unwrap(), "--list-only"])
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    let stdout = stdout(&output);
    // `root` keeps `*` in `NO_COLOR` mode.
    assert!(stdout.contains("root* echo 'system job ran'"), "{stdout}");
    assert!(stdout.contains("www-data echo 'other user'"), "{stdout}");
}

#[test]
fn system_file_job_runs_as_the_current_user_ignoring_its_user_field() {
    let file = fixture("crontab_system.cron");

    // The job declares `root`, but cronrunner does not escalate.
    let output = crn()
        .env("HOME", "/tmp")
        .args(["--system-file", file.to_str().unwrap(), "1"])
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).contains("system job ran"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn empty_file_does_not_fall_back_to_crontab() {
    let empty = temporary_directory("empty_file").join("empty.cron");
    fs::write(&empty, "").unwrap();

    let output = crn()
        .env("PATH", "")
        .args(["--file", empty.to_str().unwrap(), "--list-only"])
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "No jobs to run.\n");
}

#[test]
fn runs_a_job_with_its_owning_crontab_environment() {
    let first = fixture("crontab_file_one.cron");
    let second = fixture("crontab_file_two.cron");

    let output = crn()
        .env("HOME", "/tmp")
        .args(["--file", first.to_str().unwrap()])
        .args(["--file", second.to_str().unwrap(), "2"])
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("test -z \"${FIRST_ONLY+x}\""));
}

#[test]
fn adding_another_file_does_not_change_a_job_fingerprint() {
    let first = fixture("crontab_file_one.cron");
    let second = fixture("crontab_file_two.cron");

    let alone = crn()
        .args([
            "--fingerprint",
            "--file",
            first.to_str().unwrap(),
            "--list-only",
        ])
        .output()
        .unwrap();
    let after_another_file = crn()
        .args(["--fingerprint", "--file", second.to_str().unwrap()])
        .args(["--file", first.to_str().unwrap(), "--list-only"])
        .output()
        .unwrap();

    assert!(alone.status.success(), "{}", stderr(&alone));
    assert!(
        after_another_file.status.success(),
        "{}",
        stderr(&after_another_file)
    );
    assert_eq!(
        fingerprint_for(&alone, "First file job."),
        fingerprint_for(&after_another_file, "First file job.")
    );
}

#[test]
fn canonical_path_spelling_does_not_change_a_job_fingerprint() {
    let directory = temporary_directory("canonical_fingerprint");
    let file = directory.join("jobs.cron");
    let link = directory.join("jobs-link.cron");
    fs::write(&file, "@daily echo canonical\n").unwrap();
    if link.exists() {
        fs::remove_file(&link).unwrap();
    }
    symlink(&file, &link).unwrap();
    let absolute = fs::canonicalize(&file).unwrap();

    let relative_output = crn()
        .current_dir(&directory)
        .args(["--fingerprint", "--file", "jobs.cron", "--list-only"])
        .output()
        .unwrap();
    let absolute_output = crn()
        .args([
            "--fingerprint",
            "--file",
            absolute.to_str().unwrap(),
            "--list-only",
        ])
        .output()
        .unwrap();
    let symlink_output = crn()
        .args([
            "--fingerprint",
            "--file",
            link.to_str().unwrap(),
            "--list-only",
        ])
        .output()
        .unwrap();

    for output in [&relative_output, &absolute_output, &symlink_output] {
        assert!(output.status.success(), "{}", stderr(output));
    }
    assert_eq!(
        fingerprint_for(&relative_output, "echo canonical"),
        fingerprint_for(&absolute_output, "echo canonical")
    );
    assert_eq!(
        fingerprint_for(&relative_output, "echo canonical"),
        fingerprint_for(&symlink_output, "echo canonical")
    );
}

#[test]
fn duplicate_canonical_files_are_rejected() {
    let directory = temporary_directory("duplicate_files");
    let file = directory.join("jobs.cron");
    let link = directory.join("jobs-link.cron");
    fs::write(&file, "@daily :\n").unwrap();
    if link.exists() {
        fs::remove_file(&link).unwrap();
    }
    symlink(&file, &link).unwrap();

    let output = crn()
        .args(["--file", file.to_str().unwrap()])
        .args(["--file", link.to_str().unwrap(), "--list-only"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = stderr(&output);
    assert!(stderr.contains(file.to_str().unwrap()));
    assert!(stderr.contains(link.to_str().unwrap()));
    assert!(stderr.contains("refers to the same document as"));
}

#[test]
fn missing_file_is_a_file_read_error() {
    let missing = temporary_directory("missing_file").join("missing.cron");
    let io_reason = fs::canonicalize(&missing).unwrap_err().to_string();

    let output = crn()
        .args(["--file", missing.to_str().unwrap(), "--list-only"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr(&output);
    assert!(stderr.contains("Cannot read crontab file"));
    assert!(stderr.contains(missing.to_str().unwrap()));
    assert!(stderr.contains(&io_reason));
}

#[test]
fn default_cli_source_still_invokes_crontab() {
    let bin_directory = temporary_directory("default_source");
    let mock = bin_directory.join("crontab");
    fs::copy(fixture("crontab_example.sh"), &mock).unwrap();
    let mut permissions = fs::metadata(&mock).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&mock, permissions).unwrap();

    let output = crn()
        .env("PATH", format!("{}:/bin:/usr/bin", bin_directory.display()))
        .arg("--list-only")
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("$HOME/bin/daily.job"));
}
