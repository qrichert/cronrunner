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
)

CWD: dict = {"cwd": Path().home()}


class TestCrontabReader(unittest.TestCase):
    def setUp(self) -> None:
        cronrunner.subprocess.run = Mock()

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
        cronrunner.subprocess.run.assert_called_with(
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
        nodes: list = parser.parse(
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
            nodes,
            [
                Comment(value="# CronRunner Demo"),
                Comment(value="# ---------------"),
                CronJob(
                    schedule="@reboot", job="/usr/bin/bash ~/startup.sh", description=""
                ),
                Comment(
                    value="# Double-hash comments (##) immediately preceding a job are used as"
                ),
                Comment(value="# description. See below:"),
                Comment(value="## Update brew."),
                CronJob(
                    schedule="30 20 * * *",
                    job="/usr/local/bin/brew update && /usr/local/bin/brew upgrade",
                    description="Update brew.",
                ),
                Variable(identifier="FOO", value="bar"),
                Comment(value="## Print variable."),
                CronJob(
                    schedule="* * * * *", job="echo $FOO", description="Print variable."
                ),
                Comment(value="# Do nothing (this is a regular comment)."),
                CronJob(schedule="@reboot", job=":", description=""),
            ],
        )

    def test_description_detection_does_not_fail_if_nothing_precedes_job(self) -> None:
        parser = CrontabParser()
        nodes: list = parser.parse("* * * * * printf 'hello, world'")
        self.assertListEqual(
            nodes,
            [
                CronJob(
                    schedule="* * * * *",
                    job="printf 'hello, world'",
                    description="",
                )
            ],
        )

    def test_unknown_job_shortcut(self) -> None:
        parser = CrontabParser()
        nodes: list = parser.parse("# The following line is unknown:\nunknown :")
        self.assertListEqual(
            nodes,
            [
                Comment(value="# The following line is unknown:"),
                Unknown(value="unknown :"),
            ],
        )

    def test_whitespace_is_cleared_around_variables(self) -> None:
        parser = CrontabParser()
        nodes: list = parser.parse("   FOO     =   bar   ")
        self.assertListEqual(
            nodes,
            [
                Variable(identifier="FOO", value="bar"),
            ],
        )

    def test_variable_with_value_containing_equal_sign(self) -> None:
        parser = CrontabParser()
        nodes: list = parser.parse(
            "DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus"
        )
        self.assertListEqual(
            nodes,
            [
                Variable(
                    identifier="DBUS_SESSION_BUS_ADDRESS",
                    value="unix:path=/run/user/1000/bus",
                ),
            ],
        )

    def test_extra_whitespace_in_schedule_is_ignored(self) -> None:
        parser = CrontabParser()
        nodes: list = parser.parse("*   *    *   *   * printf 'hello, world'")
        self.assertListEqual(
            nodes,
            [
                CronJob(
                    schedule="*   *    *   *   *",
                    job="printf 'hello, world'",
                    description="",
                )
            ],
        )


class TestCrontab(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.nodes: list = [
            Comment(value="# CronRunner Demo"),
            Comment(value="# ---------------"),
            CronJob(
                schedule="@reboot",
                job="/usr/bin/bash ~/startup.sh",
                description="",
            ),
            Comment(
                value="# Double-hash comments (##) immediately preceding a job are used as"
            ),
            Comment(value="# description. See below:"),
            Comment(value="## Update brew."),
            CronJob(
                schedule="30 20 * * *",
                job="/usr/local/bin/brew update && /usr/local/bin/brew upgrade",
                description="Update brew.",
            ),
            Variable(identifier="FOO", value="bar"),
            Comment(value="## Print variable."),
            CronJob(
                schedule="* * * * *",
                job="echo $FOO",
                description="Print variable.",
            ),
            Comment(value="# Do nothing (this is a regular comment)."),
            CronJob(
                schedule="@reboot",
                job=":",
                description="",
            ),
            Variable(identifier="SHELL", value="/bin/bash"),
            CronJob(
                schedule="@hourly",
                job="echo 'I am echoed by bash!'",
                description="",
            ),
        ]

    def setUp(self) -> None:
        cronrunner.subprocess.run = Mock()

    def test_default_shell(self) -> None:
        self.assertEqual(Crontab.DEFAULT_SHELL, "/bin/sh")

    def test_bool_true(self) -> None:
        crontab = Crontab(self.nodes)
        self.assertTrue(bool(crontab))

    def test_bool_false(self) -> None:
        crontab = Crontab([])
        self.assertFalse(bool(crontab))

    def test_list_of_jobs(self) -> None:
        crontab = Crontab(self.nodes)
        self.assertListEqual(
            crontab.jobs, [node for node in self.nodes if isinstance(node, CronJob)]
        )

    def test_working_directory_is_home_directory(self) -> None:
        crontab = Crontab(self.nodes)
        crontab.run(crontab.jobs[0])
        self.assertEqual(
            cronrunner.subprocess.run.call_args.kwargs["cwd"],
            Path().home(),
        )

    def test_run_cron_without_variable(self) -> None:
        crontab = Crontab(self.nodes)
        crontab.run(crontab.jobs[0])
        cronrunner.subprocess.run.assert_called_with(
            [Crontab.DEFAULT_SHELL, "-c", "/usr/bin/bash ~/startup.sh"], **CWD
        )

    def test_run_cron_with_variable(self) -> None:
        crontab = Crontab(self.nodes)
        crontab.run(crontab.jobs[2])
        cronrunner.subprocess.run.assert_called_with(
            [Crontab.DEFAULT_SHELL, "-c", "FOO=bar;echo $FOO"], **CWD
        )

    def test_run_cron_after_variable_but_not_stuck_to_it(self) -> None:
        crontab = Crontab(self.nodes)
        crontab.run(crontab.jobs[3])
        cronrunner.subprocess.run.assert_called_with(
            [Crontab.DEFAULT_SHELL, "-c", "FOO=bar;:"], **CWD
        )

    def test_run_cron_with_default_shell(self) -> None:
        crontab = Crontab(self.nodes)
        crontab.run(crontab.jobs[0])
        self.assertEqual(
            cronrunner.subprocess.run.call_args.args[0][0], Crontab.DEFAULT_SHELL
        )

    def test_run_cron_with_different_shell(self) -> None:
        crontab = Crontab(self.nodes)
        crontab.run(crontab.jobs[4])
        self.assertEqual(cronrunner.subprocess.run.call_args.args[0][0], "/bin/bash")
        cronrunner.subprocess.run.assert_called_with(
            ["/bin/bash", "-c", "FOO=bar;SHELL=/bin/bash;echo 'I am echoed by bash!'"],
            **CWD,
        )

    def test_shell_is_reset_between_two_executions(self) -> None:
        crontab = Crontab(self.nodes)

        crontab.run(crontab.jobs[4])
        self.assertEqual(cronrunner.subprocess.run.call_count, 1)
        self.assertEqual(cronrunner.subprocess.run.call_args.args[0][0], "/bin/bash")

        crontab.run(crontab.jobs[0])
        self.assertEqual(cronrunner.subprocess.run.call_count, 2)
        self.assertEqual(
            cronrunner.subprocess.run.call_args.args[0][0], Crontab.DEFAULT_SHELL
        )

    def test_run_job_not_in_crontab(self) -> None:
        crontab = Crontab(self.nodes)
        with self.assertRaises(ValueError):
            crontab.run(CronJob(schedule="", job="", description=""))


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
            crontab.nodes,
            [
                CronJob(
                    schedule="@reboot",
                    job="/usr/bin/bash ~/startup.sh",
                    description="",
                ),
                Comment(value="## Update brew."),
                CronJob(
                    schedule="30 20 * * *",
                    job="/usr/local/bin/brew update && /usr/local/bin/brew upgrade",
                    description="Update brew.",
                ),
            ],
        )


if __name__ == "__main__":
    unittest.main()
