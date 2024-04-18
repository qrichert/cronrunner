mod utils;

use crate::utils::{mock_crontab, mock_shell, read_output_file};
use cronrunner::crontab::tokens::{Comment, CronJob, Token, Variable};
use cronrunner::crontab::{make_instance, ReadError, ReadErrorDetail, Reader, RunResultDetail};
use std::env;

// Warning: These tests MUST be run sequentially. Running them in
// parallel threads may cause conflicts with environment variables,
// as a variable may be overridden before it is used.

// Really, this is a unit test. But here we've got the mocking machinery
// available at no extra cost.
#[test]
fn correct_argument_is_passed_to_crontab() {
    mock_crontab("output_args");

    let crontab = Reader::read().expect("should be an ok");

    // crontab -l
    assert_eq!(crontab.trim(), "-l");
}

#[test]
fn run_job_success() {
    mock_crontab("crontab_runnable_jobs");
    mock_shell("do_nothing");

    let crontab = make_instance().expect("should be an ok");
    let job = crontab.get_job_from_uid(2).expect("job exists");

    let res = crontab.run(job);

    assert!(res.was_successful);
    assert_eq!(res.detail, RunResultDetail::DidRun { exit_code: Some(0) });
}

#[test]
fn run_job_error_shell_executable_not_found() {
    mock_crontab("crontab_bad_shell");

    let crontab = make_instance().expect("should be an ok");
    let job = crontab.get_job_from_uid(1).expect("job exists");

    let res = crontab.run(job);

    assert!(!res.was_successful);
    assert_eq!(
        res.detail,
        RunResultDetail::DidNotRun {
            reason: String::from("Failed to run command (does shell exist?).")
        }
    );
}

#[test]
fn run_job_error_other_reason() {
    mock_crontab("crontab_runnable_jobs");

    let crontab = make_instance().expect("should be an ok");
    let job_not_in_crontab = CronJob {
        uid: 42,
        schedule: String::from("@never"),
        command: String::from("sleep infinity"),
        description: String::new(),
    };

    // We could trigger any error here, besides obviously a problem with
    // the shell executable.
    let res = crontab.run(&job_not_in_crontab);

    assert!(!res.was_successful);
    assert_eq!(
        res.detail,
        RunResultDetail::DidNotRun {
            reason: String::from("The given job is not in the crontab.")
        }
    );
}

#[test]
fn correct_job_is_run() {
    mock_crontab("crontab_runnable_jobs");
    mock_shell("output_args_to_file");

    let crontab = make_instance().expect("should be an ok");
    let job = crontab.get_job_from_uid(2).expect("job exists");

    let res = crontab.run(job);

    assert!(res.was_successful);

    let output = read_output_file("output_args");

    assert_eq!(output.trim(), "-c echo \":)\"");
}

#[test]
fn edge_cases_with_variables() {
    mock_crontab("crontab_variables_edge_cases");
    mock_shell("output_stdout_stderr_to_file");

    let crontab = make_instance().expect("should be an ok");
    let job = crontab.get_job_from_uid(1).expect("job exists");

    let res = crontab.run(job);

    assert!(res.was_successful);

    let output = read_output_file("output_stdout_stderr");

    assert_eq!(
        output.trim().split_terminator('\n').collect::<Vec<&str>>(),
        vec![
            "double_quoted_identifier",
            "single_quoted_identifier",
            "double_quoted_value",
            "single_quoted_value",
            "double_quoted_identifier_and_value",
            "single_quoted_identifier_and_value",
            "quoted # hash",
            "unquoted # hash",
            "$UNEXPANDED_QUOTED",
            "$UNEXPANDED_UNQUOTED",
        ]
    );
}

#[test]
fn make_instance_success() {
    mock_crontab("crontab_example");

    let crontab = make_instance().expect("should be an ok");

    assert_eq!(
        crontab.tokens,
        vec![
            Token::Comment(Comment {
                value: String::from(
                    "# use /bin/sh to run commands, overriding the default set by cron"
                )
            }),
            Token::Variable(Variable {
                identifier: String::from("SHELL"),
                value: String::from("/bin/sh")
            }),
            Token::Comment(Comment {
                value: String::from("# mail any output to `paul', no matter whose crontab this is")
            }),
            Token::Variable(Variable {
                identifier: String::from("MAILTO"),
                value: String::from("paul")
            }),
            Token::Comment(Comment {
                value: String::from("#")
            }),
            Token::Comment(Comment {
                value: String::from("# run five minutes after midnight, every day")
            }),
            Token::CronJob(CronJob {
                uid: 1,
                schedule: String::from("5 0 * * *"),
                command: String::from("      $HOME/bin/daily.job >> $HOME/tmp/out 2>&1"),
                description: String::new()
            }),
            Token::Comment(Comment {
                value: String::from(
                    "# run at 2:15pm on the first of every month -- output mailed to paul"
                )
            }),
            Token::CronJob(CronJob {
                uid: 2,
                schedule: String::from("15 14 1 * *"),
                command: String::from("    $HOME/bin/monthly"),
                description: String::new()
            }),
            Token::Comment(Comment {
                value: String::from("# run at 10 pm on weekdays, annoy Joe")
            }),
            Token::CronJob(CronJob {
                uid: 3,
                schedule: String::from("0 22 * * 1-5"),
                command: String::from("   mail -s \"It's 10pm\" joe%Joe,%%Where are your kids?%"),
                description: String::new()
            }),
            Token::CronJob(CronJob {
                uid: 4,
                schedule: String::from("23 0-23/2 * * *"),
                command: String::from("echo \"run 23 minutes after midn, 2am, 4am ..., everyday\""),
                description: String::new()
            }),
            Token::CronJob(CronJob {
                uid: 5,
                schedule: String::from("5 4 * * sun"),
                command: String::from("    echo \"run at 5 after 4 every sunday\""),
                description: String::new()
            })
        ]
    );
}

#[test]
fn make_instance_error_reading_crontab() {
    mock_crontab("exit_non_zero");

    let crontab = make_instance();
    let error = crontab.expect_err("should be an error");

    assert_eq!(
        error,
        ReadError {
            reason: String::from("Cannot read crontab of current user."),
            detail: ReadErrorDetail::NonZeroExit {
                exit_code: Some(2),
                stderr: Some(String::from("crontab: illegal option -- <test>\n")),
            }
        }
    );
}

#[test]
fn make_instance_error_running_crontab_command() {
    // Make `crontab` executable inaccessible.
    env::set_var("PATH", "");

    let crontab = make_instance();
    let error = crontab.expect_err("should be an error");

    assert_eq!(
        error,
        ReadError {
            reason: String::from("Unable to locate the crontab executable on the system."),
            detail: ReadErrorDetail::CouldNotRunCommand,
        }
    );
}
