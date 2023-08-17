import unittest

from cronrunner import Comment, CronJob, CrontabParser, Unknown, Variable

from .fixtures import CRONTAB


class TestCrontabParser(unittest.TestCase):
    def test_regular_crontab(self) -> None:
        parser = CrontabParser()
        nodes: list = parser.parse(CRONTAB)
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


if __name__ == "__main__":
    unittest.main()
