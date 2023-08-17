#!/usr/bin/env python3

import re
import subprocess
import sys
from dataclasses import dataclass


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
    job: str
    description: str


@dataclass
class Variable:
    value: str


@dataclass
class Comment:
    value: str


@dataclass
class UnknownOrEmpty:
    value: str


class CrontabParser:
    def parse(self, crontab: str) -> list:
        res: list = []
        line: str
        for line in crontab.splitlines():
            line = line.strip()
            if self._is_job(line):
                schedule, job = self._split_schedule_and_job(line)
                description: str = ""
                if res and isinstance(res[-1], Comment):
                    preceding_comment: str = res[-1].value
                    if preceding_comment.startswith("##"):
                        description = preceding_comment[2:].lstrip(" ")
                res.append(CronJob(schedule, job, description))
            elif self._is_variable(line):
                res.append(Variable(line))
            elif self._is_comment(line):
                res.append(Comment(line))
            else:
                res.append(UnknownOrEmpty(line))

        return res

    @staticmethod
    def _is_job(line: str) -> bool:
        return bool(re.match(r"(\d+|\*|@)", line))

    @staticmethod
    def _split_schedule_and_job(line: str) -> tuple:
        """Split schedule and job parts of a job line.

        This is a naive splitter that assumes a schedule consists of
        either one element if it is a shortcut (e.g., @daily), or five
        elements if not (e.g., * * * * *, 0 12 * * *, etc.).

        Once the appropriate number of elements is consumed (i.e., the
        schedule is consumed), it considers the rest to be the job
        itself.
        """
        schedule_length: int = 1 if line.startswith("@") else 5
        schedule: list = []
        job: list = []
        i: int = 0
        for element in line.split(" "):
            # Schedule
            if i < schedule_length:
                schedule.append(element)
                if element:
                    i += 1
            # Job
            else:
                job.append(element)
        schedule: str = " ".join(schedule).strip()
        job: str = " ".join(job).strip()
        return schedule, job

    @staticmethod
    def _is_variable(line: str) -> bool:
        return bool(re.match(r"[a-zA-Z_][a-zA-Z0-9_]*", line))

    @staticmethod
    def _is_comment(line: str) -> bool:
        return line.startswith("#")


@dataclass
class Crontab:
    nodes: list

    def __bool__(self) -> bool:
        return len(self.jobs) > 0

    @property
    def jobs(self) -> list:
        return [node for node in self.nodes if isinstance(node, CronJob)]

    def run(self, job: CronJob) -> None:
        if job not in self.nodes:
            raise ValueError(f"Unknown job: {job}.")
        out: list = self._extract_variables_and_target_job(job)
        subprocess.run(["bash", "-c", ";".join(out)])

    def _extract_variables_and_target_job(self, job: CronJob) -> list:
        out: list = []
        for node in self.nodes:
            if isinstance(node, Variable):
                out.append(node.value)
            if node == job:
                out.append(node.job)
                break  # Variables coming after the job are not used.
        return out


def get_crontab() -> Crontab:
    crontab: str = CrontabReader().read()
    nodes: list = CrontabParser().parse(crontab)
    return Crontab(nodes)


def _color_error(string: str) -> str:
    return "\033[0;91m{}\033[0m".format(string)


def _color_highlight(string: str) -> str:
    return "\033[0;92m{}\033[0m".format(string)


def _color_attenuate(string: str) -> str:
    return "\033[0;90m{}\033[0m".format(string)


def _main() -> int:
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

    for i, job in enumerate(crontab.jobs):
        job_number: str = _color_highlight(str(i + 1)) + "."
        description: str = f"{job.description} " if job.description else ""
        schedule: str = _color_attenuate(job.schedule)
        command: str = job.job
        print(f"{job_number} {description}{schedule} {command}")

    job_number: str = input(">>> Select a job to run: ")
    if not job_number:
        return 0
    try:
        job_number: int = int(job_number)
        if not 0 < job_number <= len(crontab.jobs):
            raise ValueError
    except ValueError:
        print(_color_error("Invalid job number."))
        return 1

    job: CronJob = crontab.jobs[job_number - 1]
    print(_color_highlight("$"), job.job)
    crontab.run(job)

    return 0


if __name__ == "__main__":
    try:
        sys.exit(_main())
    except KeyboardInterrupt:
        sys.exit(1)
