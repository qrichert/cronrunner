import subprocess
import unittest
from pathlib import Path
from unittest.mock import Mock

import cronrunner.cronrunner as cronrunner
from cronrunner.cronrunner import (
    Comment,
    CronJob,
    Crontab,
    CrontabParser,
    CrontabReader,
    CrontabReadError,
    Unknown,
    Variable,
    get_crontab,
    main,
)

CWD: dict[str, Path] = {"cwd": Path().home()}


class TestCrontabReader(unittest.TestCase):
    def setUp(self) -> None:
        self.subprocess_run_mock = Mock()
        cronrunner.subprocess.run = self.subprocess_run_mock

    def test_regular_read(self) -> None:
        run_result = Mock()
        run_result.stdout = "<crontab>"
        cronrunner.subprocess.run = Mock(return_value=run_result)
        reader = CrontabReader()
        crontab: str = reader.read()
        self.assertEqual(crontab, "<crontab>")

    def test_is_read_with_correct_arguments(self) -> None:
        reader = CrontabReader()
        reader.read()
        self.subprocess_run_mock.assert_called_with(
            ["crontab", "-l"],
            capture_output=True,
            text=True,
            check=True,
        )

    def test_error_if_exit_code_not_0_is_handled(self) -> None:
        cronrunner.subprocess.run = Mock(
            side_effect=subprocess.CalledProcessError(1337, cmd="", stderr="<stderr>")
        )
        reader = CrontabReader()
        with self.assertRaises(CrontabReadError) as ctx:
            reader.read()
        self.assertEqual(ctx.exception.exit_code, 1337)
        self.assertEqual(ctx.exception.detail, "<stderr>")

    def test_error_if_executable_not_found_is_handled(self) -> None:
        cronrunner.subprocess.run = Mock(side_effect=FileNotFoundError)
        reader = CrontabReader()
        with self.assertRaises(CrontabReadError):
            reader.read()


class TestCrontabParser(unittest.TestCase):
    def test_regular_crontab(self) -> None:
        parser = CrontabParser()
        tokens: list = parser.parse(
            """
            # CronRunner Demo
            # ---------------

            @reboot /usr/bin/bash ~/startup.sh

            # Double-hash comments (##) immediately preceding a job are used as
            # description. See below:

            ## Update brew.
            30 20 * * * /usr/local/bin/brew update && /usr/local/bin/brew upgrade

            FOO=bar
            ## Print variable.
            * * * * * echo $FOO

            # Do nothing (this is a regular comment).
            @reboot :
            """
        )
        self.assertListEqual(
            tokens,
            [
                Comment(value="# CronRunner Demo"),
                Comment(value="# ---------------"),
                CronJob(
                    schedule="@reboot",
                    command="/usr/bin/bash ~/startup.sh",
                    description="",
                ),
                Comment(
                    value="# Double-hash comments (##) immediately preceding a job are used as"
                ),
                Comment(value="# description. See below:"),
                Comment(value="## Update brew."),
                CronJob(
                    schedule="30 20 * * *",
                    command="/usr/local/bin/brew update && /usr/local/bin/brew upgrade",
                    description="Update brew.",
                ),
                Variable(identifier="FOO", value="bar"),
                Comment(value="## Print variable."),
                CronJob(
                    schedule="* * * * *",
                    command="echo $FOO",
                    description="Print variable.",
                ),
                Comment(value="# Do nothing (this is a regular comment)."),
                CronJob(schedule="@reboot", command=":", description=""),
            ],
        )

    def test_description_detection_does_not_fail_if_nothing_precedes_job(self) -> None:
        parser = CrontabParser()
        tokens: list = parser.parse("* * * * * printf 'hello, world'")
        self.assertListEqual(
            tokens,
            [
                CronJob(
                    schedule="* * * * *",
                    command="printf 'hello, world'",
                    description="",
                )
            ],
        )

    def test_unknown_job_shortcut(self) -> None:
        parser = CrontabParser()
        tokens: list = parser.parse("# The following line is unknown:\nunknown :")
        self.assertListEqual(
            tokens,
            [
                Comment(value="# The following line is unknown:"),
                Unknown(value="unknown :"),
            ],
        )

    def test_whitespace_is_cleared_around_variables(self) -> None:
        parser = CrontabParser()
        tokens: list = parser.parse("   FOO     =   bar   ")
        self.assertListEqual(
            tokens,
            [
                Variable(identifier="FOO", value="bar"),
            ],
        )

    def test_variable_with_value_containing_equal_sign(self) -> None:
        parser = CrontabParser()
        tokens: list = parser.parse(
            "DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus"
        )
        self.assertListEqual(
            tokens,
            [
                Variable(
                    identifier="DBUS_SESSION_BUS_ADDRESS",
                    value="unix:path=/run/user/1000/bus",
                ),
            ],
        )

    def test_extra_whitespace_in_schedule_is_ignored(self) -> None:
        parser = CrontabParser()
        tokens: list = parser.parse("*   *    *   *   * printf 'hello, world'")
        self.assertListEqual(
            tokens,
            [
                CronJob(
                    schedule="*   *    *   *   *",
                    command="printf 'hello, world'",
                    description="",
                )
            ],
        )


class TestCrontab(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.tokens: list = [
            Comment(value="# CronRunner Demo"),
            Comment(value="# ---------------"),
            CronJob(
                schedule="@reboot",
                command="/usr/bin/bash ~/startup.sh",
                description="",
            ),
            Comment(
                value="# Double-hash comments (##) immediately preceding a job are used as"
            ),
            Comment(value="# description. See below:"),
            Comment(value="## Update brew."),
            CronJob(
                schedule="30 20 * * *",
                command="/usr/local/bin/brew update && /usr/local/bin/brew upgrade",
                description="Update brew.",
            ),
            Variable(identifier="FOO", value="bar"),
            Comment(value="## Print variable."),
            CronJob(
                schedule="* * * * *",
                command="echo $FOO",
                description="Print variable.",
            ),
            Comment(value="# Do nothing (this is a regular comment)."),
            CronJob(
                schedule="@reboot",
                command=":",
                description="",
            ),
            Variable(identifier="SHELL", value="/bin/bash"),
            CronJob(
                schedule="@hourly",
                command="echo 'I am echoed by bash!'",
                description="",
            ),
        ]

    def setUp(self) -> None:
        self.subprocess_run_mock = Mock()
        cronrunner.subprocess.run = self.subprocess_run_mock

    def test_default_shell(self) -> None:
        self.assertEqual(Crontab.DEFAULT_SHELL, "/bin/sh")

    def test_bool_true(self) -> None:
        crontab = Crontab(self.tokens)
        self.assertTrue(bool(crontab))

    def test_bool_false(self) -> None:
        crontab = Crontab([])
        self.assertFalse(bool(crontab))

    def test_list_of_jobs(self) -> None:
        crontab = Crontab(self.tokens)
        self.assertListEqual(
            crontab.jobs, [token for token in self.tokens if isinstance(token, CronJob)]
        )

    def test_working_directory_is_home_directory(self) -> None:
        crontab = Crontab(self.tokens)
        crontab.run(crontab.jobs[0])
        self.assertEqual(
            self.subprocess_run_mock.call_args.kwargs["cwd"],
            Path().home(),
        )

    def test_run_cron_without_variable(self) -> None:
        crontab = Crontab(self.tokens)
        crontab.run(crontab.jobs[0])
        self.subprocess_run_mock.assert_called_with(
            [Crontab.DEFAULT_SHELL, "-c", "/usr/bin/bash ~/startup.sh"], **CWD
        )

    def test_run_cron_with_variable(self) -> None:
        crontab = Crontab(self.tokens)
        crontab.run(crontab.jobs[2])
        self.subprocess_run_mock.assert_called_with(
            [Crontab.DEFAULT_SHELL, "-c", "FOO=bar;echo $FOO"], **CWD
        )

    def test_run_cron_after_variable_but_not_stuck_to_it(self) -> None:
        crontab = Crontab(self.tokens)
        crontab.run(crontab.jobs[3])
        self.subprocess_run_mock.assert_called_with(
            [Crontab.DEFAULT_SHELL, "-c", "FOO=bar;:"], **CWD
        )

    def test_run_cron_with_default_shell(self) -> None:
        crontab = Crontab(self.tokens)
        crontab.run(crontab.jobs[0])
        self.assertEqual(
            self.subprocess_run_mock.call_args.args[0][0],
            Crontab.DEFAULT_SHELL,
        )

    def test_run_cron_with_different_shell(self) -> None:
        crontab = Crontab(self.tokens)
        crontab.run(crontab.jobs[4])
        self.assertEqual(self.subprocess_run_mock.call_args.args[0][0], "/bin/bash")
        self.subprocess_run_mock.assert_called_with(
            ["/bin/bash", "-c", "FOO=bar;SHELL=/bin/bash;echo 'I am echoed by bash!'"],
            **CWD,
        )

    def test_shell_is_reset_between_two_executions(self) -> None:
        crontab = Crontab(self.tokens)

        crontab.run(crontab.jobs[4])
        self.assertEqual(self.subprocess_run_mock.call_count, 1)
        self.assertEqual(self.subprocess_run_mock.call_args.args[0][0], "/bin/bash")

        crontab.run(crontab.jobs[0])
        self.assertEqual(self.subprocess_run_mock.call_count, 2)
        self.assertEqual(
            self.subprocess_run_mock.call_args.args[0][0],
            Crontab.DEFAULT_SHELL,
        )

    def test_run_job_not_in_crontab(self) -> None:
        crontab = Crontab(self.tokens)
        with self.assertRaises(ValueError):
            crontab.run(CronJob(schedule="", command="", description=""))

    def test_two_equal_jobs_are_treated_as_different_jobs(self) -> None:
        crontab = Crontab(
            [
                CronJob(
                    schedule="@daily",
                    command="df -h > ~/track_disk_usage.txt",
                    description="Track disk usage.",
                ),
                Variable(identifier="FOO", value="bar"),
                CronJob(
                    schedule="@daily",
                    command="df -h > ~/track_disk_usage.txt",
                    description="Track disk usage.",
                ),
            ]
        )
        crontab.run(crontab.jobs[1])
        # If 'FOO=bar' is not included, it means the first of the twin
        # jobs was used instead of the second that we selected.
        self.subprocess_run_mock.assert_called_with(
            ["/bin/sh", "-c", "FOO=bar;df -h > ~/track_disk_usage.txt"],
            **CWD,
        )


class TestGetCrontab(unittest.TestCase):
    def setUp(self) -> None:
        cronrunner.subprocess.run = Mock()

    def test_get_crontab(self) -> None:
        run_result = Mock()
        run_result.stdout = """
            @reboot /usr/bin/bash ~/startup.sh

            ## Update brew.
            30 20 * * * /usr/local/bin/brew update && /usr/local/bin/brew upgrade
            """
        cronrunner.subprocess.run = Mock(return_value=run_result)
        crontab: Crontab = get_crontab()
        self.assertEqual(
            crontab.tokens,
            [
                CronJob(
                    schedule="@reboot",
                    command="/usr/bin/bash ~/startup.sh",
                    description="",
                ),
                Comment(value="## Update brew."),
                CronJob(
                    schedule="30 20 * * *",
                    command="/usr/local/bin/brew update && /usr/local/bin/brew upgrade",
                    description="Update brew.",
                ),
            ],
        )


class TestUI(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls._get_crontab_saved = cronrunner.get_crontab

        run_result = Mock()
        run_result.stdout = """
            @reboot /usr/bin/bash ~/startup.sh

            FOO=bar

            ## Update brew.
            30 20 * * * /usr/local/bin/brew update && /usr/local/bin/brew upgrade
            """
        cronrunner.subprocess.run = Mock(return_value=run_result)
        cls.crontab: Crontab = get_crontab()

    @classmethod
    def tearDownClass(cls) -> None:
        cronrunner.get_crontab = cls._get_crontab_saved

    def setUp(self) -> None:
        self.subprocess_run_mock = Mock()
        cronrunner.subprocess.run = self.subprocess_run_mock

        self.print_mock = Mock()
        cronrunner.print = self.print_mock  # type: ignore

        self.get_crontab_mock = Mock(return_value=self.crontab)
        cronrunner.get_crontab = self.get_crontab_mock

    def test_error_getting_crontab(self) -> None:
        cronrunner.get_crontab = Mock(
            side_effect=CrontabReadError(
                "Some error happened.", exit_code=1337, detail="I don't know why."
            )
        )

        exit_code: int = main()

        self.assertEqual(exit_code, 1337)
        self.assertEqual(
            self.print_mock.call_args_list[0][0][0],
            "\x1b[0;91mSome error happened.\x1b[0m",
        )
        self.assertEqual(self.print_mock.call_args_list[1][0][0], "I don't know why.")

    def test_no_jobs_to_run(self) -> None:
        cronrunner.get_crontab = Mock(return_value=Crontab([]))

        exit_code: int = main()

        self.assertEqual(exit_code, 0)
        self.assertEqual(self.print_mock.call_args_list[0][0][0], "No jobs to run.")

    def test_jobs_menu(self) -> None:
        cronrunner.input = Mock(return_value="")  # type: ignore

        exit_code: int = main()

        self.assertEqual(exit_code, 0)
        self.assertEqual(
            self.print_mock.call_args_list[0][0][0],
            "\x1b[0;92m1.\x1b[0m \x1b[0;90m@reboot\x1b[0m /usr/bin/bash ~/startup.sh",
        )
        self.assertEqual(
            self.print_mock.call_args_list[1][0][0],
            "\x1b[0;92m2.\x1b[0m Update brew. \x1b[0;90m30 20 * * *\x1b[0m \x1b[0;90m/usr/local/bin/brew update && /usr/local/bin/brew upgrade\x1b[0m",
        )

    def test_invalid_job_number_too_low(self) -> None:
        cronrunner.input = Mock(return_value="0")  # type: ignore

        exit_code: int = main()

        self.assertEqual(exit_code, 1)
        self.assertEqual(
            self.print_mock.call_args_list[-1][0][0],
            "\x1b[0;91mInvalid job number.\x1b[0m",
        )

    def test_invalid_job_number_too_high(self) -> None:
        cronrunner.input = Mock(return_value="3")  # type: ignore

        exit_code: int = main()

        self.assertEqual(exit_code, 1)
        self.assertEqual(
            self.print_mock.call_args_list[-1][0][0],
            "\x1b[0;91mInvalid job number.\x1b[0m",
        )

    def test_run_ok(self) -> None:
        cronrunner.input = Mock(return_value="1")  # type: ignore
        self.crontab.run = Mock()

        exit_code: int = main()

        self.assertEqual(exit_code, 0)
        self.assertEqual(
            self.print_mock.call_args_list[-1][0],
            ("\x1b[0;92m$\x1b[0m", "/usr/bin/bash ~/startup.sh"),
        )
        self.assertIs(
            self.crontab.run.call_args[0][0], self.crontab.jobs[0], "Wrong job was run."
        )


if __name__ == "__main__":
    unittest.main()
