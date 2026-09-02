# cronrunner

![Crates.io License](https://img.shields.io/crates/l/cronrunner)
[![GitHub Tag](https://img.shields.io/github/v/tag/qrichert/cronrunner?sort=semver&filter=*.*.*&label=release)](https://github.com/qrichert/cronrunner/releases/latest)
[![crates.io](https://img.shields.io/crates/d/cronrunner?logo=rust&logoColor=white&color=orange)](https://crates.io/crates/cronrunner)
[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/qrichert/cronrunner/ci.yml?label=tests)](https://github.com/qrichert/cronrunner/actions)

_Run cron jobs manually._[^1]

<p align="center">
  <img src="https://raw.githubusercontent.com/qrichert/cronrunner/main/cronrunner.png" alt="cronrunner">
</p>

```crontab
# m h  dom mon dow   command

@reboot /usr/bin/bash ~/startup.sh

## Track disk space.
30 4 * * * echo $(date) $(df -h | grep "/dev/sda3") >> .disk-space.txt

## %{ignore} Stealthy job.
* * * * * env > ~/.cron.env

FOO=:)
0 12 * * * echo $FOO

### Housekeeping

## Prune dangling Docker images.
@daily docker image prune --force
```

## Get `--help`

```
Run cron jobs manually.

Usage: crn [OPTIONS] [ID]

Options:
  -l, --list-only           List available jobs and exit.
      --as-json             Render `--list-only` as JSON.
      --fingerprint         Use job fingerprints.
  -t, --tag <TAG>           Run specific tag.
  -d, --detach              Run job in the background.
  -e, --env <FILE>          Override job environment.
      --user                Add the current user's crontab.
      --system              Add Cron's system crontabs.
      --system-lsb          Add system crontabs using Cron's `-l` names.
  -f, --file <FILE>         Add jobs from a file (repeatable).
  -F, --system-file <FILE>  Add jobs from a system file (repeatable).

  -h, --help                Show this message and exit.
  -V, --version             Show the version and exit.
```

### Examples

If you know the ID of a job, you can run it directly:

```console
# Run job number 1.
$ crn 1
Running...
```

If the job takes a long time to run, you can detach it:

```console
# Prints the PID and exits.
$ crn --detach 3
1337
$ _
```

### Extras

Comments that start with two hashes (`##`) and immediately precede a job
are used as the description for that job.

```crontab
## Say hello.
@hourly echo "hello"
```

This job will be presented like this:

```
1. Say hello. @hourly echo "hello"
```

Comments that start with three hashes (`###`) are used as section
headers, up until a new section starts or up until the end.

```crontab
### Housekeeping

@daily docker image prune --force
```

This job will be presented like this:

```
Housekeeping

1. @daily docker image prune --force
```

Descriptions and sections are independent from one another.

### Fingerprints and tags

Job IDs are attributed in the order of appearance in the crontab. This
can be dangerous if used in scripts, because if the crontab changes, the
wrong job may get run.

Instead, you can activate `--fingerprint` mode, in which jobs are
identified by a fingerprint. This is less user-friendly, but if the jobs
get reordered, or if the command changes, that fingerprint will be
invalidated and the run will fail.

Or, you could tag a specific job and run it with `--tag`. Tags are
stable even if the underlying job changes. This is great for scripts,
but it does not guarantee that the command remains the same.

To define a tag, add a description comment starting with `%{...}`:

```crontab
## %{my-tag} Scriptable job.
@reboot /usr/bin/bash ~/startup.sh
```

Then you can run it like this:

```console
$ crn --tag my-tag
Running...
```

### Ignore jobs

To ignore jobs, tag them with the special `%{ignore}` tag:

```crontab
## %{ignore} Ignored job.
@daily /should/not/be/run/manually
```

### Environment

Cron runs jobs in a very minimalistic environment, which you may want to
replicate. The content of this environment is platform-specific and can
vary a lot. The best way to capture it accurately is to export it
directly from Cron. To do this, let Cron run this job once:

```crontab
## %{ignore}
* * * * * env > ~/.cron.env
```

Then, you can tell cronrunner to use this file as the environment for
the child process:

```console
$ crn --env ~/.cron.env 3
Running...
```

### Crontab sources

Source options are additive and may be combined. With no source option,
jobs are read from the current user's crontab through `crontab -l`. Use
`--user` to include it explicitly when combining it with other sources.

To read a user crontab from an arbitrary file, pass `--file`:

```console
$ crn --file ./crontab.export --list-only
$ crn -f personal.cron -f project.cron
```

If you pass multiple file sources, they are read and run in isolation.
Variables from one crontab don't leak into the other, and job
fingerprints remain stable even if you reorder the sources.

Use `--system` to include `/etc/crontab` and files under `/etc/cron.d`
that follow Cron's default naming rules. If Cron runs with `-l`, use
`--system-lsb` instead to apply its LSB naming rules. Use `--system-file`
to read an explicit system crontab file.

### System crontabs

System crontabs typically live in `/etc/cron.d/*` and have an additional
`user` field in-between the schedule and the command.

cronrunner will display that user in the jobs list, but it will not use
it to run jobs. If you want to run jobs as a different user, do it
yourself (e.g., `su - <user> -c "crn"`). cronrunner doesn't handle
privilege escalations.

### Configuration

Some arguments have corresponding environment variables, allowing you to
set values permanently in a shell startup file (e.g., `~/.bashrc`).

```
--fingerprint  CRONRUNNER_FINGERPRINT=1
--env <FILE>   CRONRUNNER_ENV=<FILE>
```

### Tips

If you have jobs you only want to execute manually, you can schedule
them to run on February 31<sup>st</sup>:

```crontab
0 0 31 2 * echo "I never run on my own!"
```

[^1]:
    cronrunner used to be a Python project, see
    [1.1.4](https://github.com/qrichert/cronrunner/tree/1.1.4).

## Installation

Install the `crn` command from [crates.io] with Cargo:

```shell
cargo install cronrunner
```

Pre-built binaries for Linux and macOS are available on the [latest
GitHub release].

[Documentation] is available on docs.rs.

[crates.io]: https://crates.io/crates/cronrunner
[latest GitHub release]:
  https://github.com/qrichert/cronrunner/releases/latest
[Documentation]: https://docs.rs/cronrunner
