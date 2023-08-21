import unittest
from unittest.mock import Mock

import cronrunner.cronrunner as cronrunner
from cronrunner.cronrunner import (
    Comment,
    CronJob,
    Crontab,
    CrontabParser,
    Unknown,
    Variable,
)


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
                Variable(value="FOO=bar"),
                Comment(value="## Print variable."),
                CronJob(
                    schedule="* * * * *", job="echo $FOO", description="Print variable."
                ),
                Comment(value="# Do nothing (this is a regular comment)."),
                CronJob(schedule="@reboot", job=":", description=""),
            ],
        )

    def test_crontab_with_unknown_job_shortcut(self) -> None:
        parser = CrontabParser()
        nodes: list = parser.parse("# The following line is unknown:\nunknown :")
        self.assertListEqual(
            nodes,
            [
                Comment(value="# The following line is unknown:"),
                Unknown(value="unknown :"),
            ],
        )


class TestCrontab(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.nodes: list = [
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
            Variable(value="FOO=bar"),
            Comment(value="## Print variable."),
            CronJob(
                schedule="* * * * *", job="echo $FOO", description="Print variable."
            ),
            Comment(value="# Do nothing (this is a regular comment)."),
            CronJob(schedule="@reboot", job=":", description=""),
        ]

    def setUp(self) -> None:
        cronrunner.subprocess.run = Mock()

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

    def test_run_cron_without_variable(self) -> None:
        crontab = Crontab(self.nodes)
        crontab.run(crontab.jobs[0])
        cronrunner.subprocess.run.assert_called_with(
            ["/bin/sh", "-c", "/usr/bin/bash ~/startup.sh"]
        )

    def test_run_cron_with_variable(self) -> None:
        crontab = Crontab(self.nodes)
        crontab.run(crontab.jobs[2])
        cronrunner.subprocess.run.assert_called_with(
            ["/bin/sh", "-c", "FOO=bar;echo $FOO"]
        )

    def test_run_cron_after_variable_but_not_stuck_to_it(self) -> None:
        crontab = Crontab(self.nodes)
        crontab.run(crontab.jobs[3])
        cronrunner.subprocess.run.assert_called_with(["/bin/sh", "-c", "FOO=bar;:"])

    def test_run_job_not_in_crontab(self) -> None:
        crontab = Crontab(self.nodes)
        with self.assertRaises(ValueError):
            crontab.run(CronJob(schedule="", job="", description=""))


if __name__ == "__main__":
    unittest.main()
