#!/usr/bin/env python3

# cronrunner — Run cron jobs manually.
# Copyright (C) 2023  Quentin Richert
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <http://www.gnu.org/licenses/>.

import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import cast


class CrontabReadError(Exception):
    def __init__(self, *args, exit_code: int = 1, detail: str = "") -> None:
        self.exit_code: int = exit_code
        self.detail: str = detail
        super().__init__(*args)


class CrontabReader:
    @staticmethod
    def read() -> str:
        try:
            process: subprocess.CompletedProcess = subprocess.run(
                ["crontab", "-l"],
                capture_output=True,
                text=True,
                check=True,
            )
        except subprocess.CalledProcessError as e:
            raise CrontabReadError(
                "Cannot read crontab of current user.",
                exit_code=e.returncode,
                detail=e.stderr,
            )
        except FileNotFoundError:
            raise CrontabReadError("Unable to locate crontab executable on the system.")
        return process.stdout


@dataclass
class CronJob:
    schedule: str
    command: str
    description: str


@dataclass
class Variable:
    identifier: str
    value: str

    @property
    def declaration(self) -> str:
        return f"{self.identifier}={self.value}"


@dataclass
class Comment:
    value: str


@dataclass
class Unknown:
    value: str


type Token = CronJob | Variable | Comment | Unknown


class CrontabParser:
    def parse(self, crontab: str) -> list[Token]:
        tokens: list[Token] = []
        line: str
        for line in crontab.splitlines():
            line = line.strip()
            if not line:
                continue
            if self._is_job(line):
                schedule, command = self._split_schedule_and_command(line)
                description: str = ""
                if self._is_previous_token_a_description_comment(tokens):
                    description_comment: str = cast(Comment, tokens[-1]).value
                    description = description_comment[2:].lstrip()
                tokens.append(CronJob(schedule, command, description))
            elif self._is_variable(line):
                identifier, value = self._split_identifier_and_value(line)
                tokens.append(Variable(identifier, value))
            elif self._is_comment(line):
                tokens.append(Comment(line))
            else:
                tokens.append(Unknown(line))

        return tokens

    @staticmethod
    def _is_job(line: str) -> bool:
        return bool(re.match(r"([0-9]|\*|@)", line))

    @staticmethod
    def _split_schedule_and_command(line: str) -> tuple[str, str]:
        """Split schedule and command parts of a job line.

        This is a naive splitter that assumes a schedule consists of
        either one element if it is a shortcut (e.g., `@daily`), or five
        elements if not (e.g., `* * * * *`, `0 12 * * *`, etc.).

        Once the appropriate number of elements is consumed (i.e., the
        schedule is consumed), it considers the rest to be the command
        itself.
        """
        schedule_length: int = 1 if line.startswith("@") else 5
        schedule_elements: list[str] = []
        command_elements: list[str] = []
        i: int = 0
        for element in line.split(" "):
            # Schedule.
            if i < schedule_length:
                schedule_elements.append(element)
                if element:
                    i += 1
            # Command.
            else:
                command_elements.append(element)
        schedule: str = " ".join(schedule_elements).strip()
        command: str = " ".join(command_elements).strip()
        return schedule, command

    @staticmethod
    def _is_previous_token_a_description_comment(tokens: list[Token]) -> bool:
        """Whether the previous token is a job description.

        Description comments are comments that start with "##" and
        immediately precede a job. They are used in the job list menu to
        give a human-readable description to sometimes obscure commands.

        This is cronrunner specific, and has nothing to do with Cron
        itself.
        """
        if not tokens:
            return False
        last_token: Token = tokens[-1]
        return isinstance(last_token, Comment) and last_token.value.startswith("##")

    @staticmethod
    def _is_variable(line: str) -> bool:
        return "=" in line and bool(re.match(r"[a-zA-Z_]", line))

    @staticmethod
    def _split_identifier_and_value(line: str) -> tuple[str, str]:
        identifier, value = line.split("=", maxsplit=1)
        return identifier.strip(), value.strip()

    @staticmethod
    def _is_comment(line: str) -> bool:
        return line.startswith("#")


class Crontab:
    DEFAULT_SHELL: str = "/bin/sh"

    def __init__(self, tokens: list[Token]) -> None:
        self.tokens: list[Token] = tokens
        self._shell: str = ""

    @property
    def jobs(self) -> list[CronJob]:
        return [token for token in self.tokens if isinstance(token, CronJob)]

    def __bool__(self) -> bool:
        return len(self.jobs) > 0

    def run(self, job: CronJob) -> None:
        if job not in self.tokens:
            raise ValueError(f"Unknown job: {job}.")
        self._shell = self.DEFAULT_SHELL
        out: list[str] = self._extract_variables_and_target_command(job)
        subprocess.run([self._shell, "-c", ";".join(out)], cwd=Path().home())

    def _extract_variables_and_target_command(self, job: CronJob) -> list[str]:
        out: list[str] = []
        for token in self.tokens:
            if isinstance(token, Variable):
                self._detect_shell_change(token)
                out.append(token.declaration)
            elif token is job:
                out.append(cast(CronJob, token).command)
                break  # Variables coming after the job are not used.
        return out

    def _detect_shell_change(self, variable: Variable) -> None:
        if variable.identifier == "SHELL":
            self._shell = variable.value


def get_crontab() -> Crontab:
    crontab: str = CrontabReader().read()
    tokens: list[Token] = CrontabParser().parse(crontab)
    return Crontab(tokens)


def _color_error(string: str) -> str:
    return "\x1b[0;91m{}\x1b[0m".format(string)


def _color_highlight(string: str) -> str:
    return "\x1b[0;92m{}\x1b[0m".format(string)


def _color_attenuate(string: str) -> str:
    return "\x1b[0;90m{}\x1b[0m".format(string)


def main() -> int:
    try:
        crontab: Crontab = get_crontab()
    except CrontabReadError as e:
        print(_color_error(str(e)))
        if e.detail:
            print(e.detail)
        return e.exit_code

    if not crontab:
        print("No jobs to run.")
        return 0

    # Print jobs available, numbered.
    for i, job in enumerate(crontab.jobs):
        job_number: str = _color_highlight(f"{i + 1}.")
        description: str = f"{job.description} " if job.description else ""
        schedule: str = _color_attenuate(job.schedule)
        command: str = _color_attenuate(job.command) if description else job.command
        print(f"{job_number} {description}{schedule} {command}")

    job_selected: str = input(">>> Select a job to run: ")
    if not job_selected:
        return 0
    try:
        job_index: int = int(job_selected) - 1
        if not 0 <= job_index < len(crontab.jobs):
            raise ValueError
    except ValueError:
        print(_color_error("Invalid job number."))
        return 1

    job: CronJob = crontab.jobs[job_index]
    print(_color_highlight("$"), job.command)
    crontab.run(job)

    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        sys.exit(1)
