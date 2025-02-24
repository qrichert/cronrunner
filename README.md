# cronrunner

[![license: GPL v3+](https://img.shields.io/badge/license-GPLv3+-blue)](https://www.gnu.org/licenses/gpl-3.0)
![GitHub Tag](https://img.shields.io/github/v/tag/qrichert/cronrunner?sort=semver&filter=*.*.*&label=release)
[![crates.io](https://img.shields.io/crates/d/cronrunner?logo=rust&logoColor=white&color=orange)](https://crates.io/crates/cronrunner)
[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/qrichert/cronrunner/run-tests.yml?label=tests)](https://github.com/qrichert/cronrunner/actions)

_Run cron jobs manually._[^1]

<p align="center">
  <img src="./cronrunner.png" alt="cronrunner">
</p>

```crontab
# m h  dom mon dow   command

@reboot /usr/bin/bash ~/startup.sh

## Track disk space.
30 4 * * * echo $(date) $(df -h | grep "/dev/sda3") >> .disk-space.txt

FOO=:)
0 12 * * * echo $FOO

### Housekeeping

## Prune dangling Docker images.
@daily docker image prune --force
```

## Get `--help`

```
Run cron jobs manually.

Usage: cr [OPTIONS] [ID]

Options:
  -h, --help           Show this message and exit.
  -v, --version        Show the version and exit.
  -l, --list-only      List available jobs and exit.
      --as-json        Render `--list-only` as JSON.
  -s, --safe           Use job fingerprints.
  -t, --tag <TAG>      Run specific tag.
  -d, --detach         Run job in the background.
```

### Examples

If you know the ID of a job, you can run it directly:

```console
# Run job number 1.
$ cr 1
Running...
```

If the job takes a long time to run, you can detach it:

```console
# Prints the PID and exits.
$ cr --detach 3
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

### Safe mode

Job IDs are attributed in the order of appearance in the crontab. This
can be dangerous if used in scripts, because if the crontab changes, the
wrong job may get run.

Instead, you can activate `--safe` mode, in which jobs are identified by
a fingerprint. This is less user-friendly, but if the jobs get
reordered, or if the command changes, that fingerprint will be
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
$ cr --tag my-tag
Running...
```

### Tips

If you have jobs you only want to execute manually, you can schedule
them to run on February 31st:

```crontab
0 0 31 2 * echo "I never run on my own!"
```

## Installation

### Directly

```console
$ wget https://github.com/qrichert/cronrunner/releases/download/X.X.X/cr-X.X.X-xxx
$ sudo install ./cr-* /usr/local/bin/cr
```

### Manual Build

#### System-wide

```console
$ git clone https://github.com/qrichert/cronrunner.git
$ cd cronrunner
$ make build
$ sudo make install
```

#### Through Cargo

```shell
cargo install cronrunner
cargo install --git https://github.com/qrichert/cronrunner.git
```

[^1]:
    cronrunner used to be a Python project, see
    [1.1.4](https://github.com/qrichert/cronrunner/tree/1.1.4).
