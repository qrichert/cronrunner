# cronrunner

[![license (bin): GPL v3+](https://img.shields.io/badge/license-GPLv3+-blue)](https://www.gnu.org/licenses/gpl-3.0)
[![license (lib): MIT](https://img.shields.io/badge/license-MIT-blue)](https://opensource.org/license/mit)
![GitHub Tag](https://img.shields.io/github/v/tag/qrichert/cronrunner?sort=semver&filter=*.*.*&label=release)
[![tokei (loc)](https://tokei.rs/b1/github/qrichert/cronrunner?label=loc&style=flat)](https://github.com/XAMPPRocky/tokei)
[![crates.io](https://img.shields.io/crates/d/cronrunner?logo=rust&logoColor=white&color=orange)](https://crates.io/crates/cronrunner)
[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/qrichert/cronrunner/ci.yml?label=tests)](https://github.com/qrichert/cronrunner/actions)

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

Usage: crn [OPTIONS] [ID]

Options:
  -h, --help           Show this message and exit.
  -v, --version        Show the version and exit.
  -l, --list-only      List available jobs and exit.
      --as-json        Render `--list-only` as JSON.
  -s, --safe           Use job fingerprints.
  -t, --tag <TAG>      Run specific tag.
  -d, --detach         Run job in the background.
  -e, --env <FILE>     Override job environment.
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
$ crn --tag my-tag
Running...
```

### Environment

Cron runs jobs in a very minimalistic environment, which you may want to
replicate. The content of this environment is platform-specific and can
vary a lot. The best way to capture it accurately is to export it
directly from Cron. To do this, let Cron run this job once:

```crontab
* * * * * env > ~/.cron.env
```

Then, you can tell cronrunner to use this file as the environment for
the child process:

```console
$ crn --env ~/.cron.env 3
Running...
```

### Configuration

Some arguments have corresponding environment variables, allowing you to
set values permanently in a shell startup file (e.g., `~/.bashrc`).

```
--safe        CRONRUNNER_SAFE=1
--env <FILE>  CRONRUNNER_ENV=<FILE>
```

### Tips

If you have jobs you only want to execute manually, you can schedule
them to run on February 31<sup>st</sup>:

```crontab
0 0 31 2 * echo "I never run on my own!"
```

## Installation

### Directly

```console
$ wget https://github.com/qrichert/cronrunner/releases/download/X.X.X/crn-X.X.X-xxx
$ sudo install ./crn-* /usr/local/bin/crn
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

## License

This project is dual-licensed:

- The **binary** as a product is licensed under GPLv3+.
- The **library** is available under the MIT license.

If you are using only the library in your own projects, you may use it
under the MIT license. However, if you are redistributing the binary or
a modified version of it, you must comply with GPLv3+.

[^1]:
    cronrunner used to be a Python project, see
    [1.1.4](https://github.com/qrichert/cronrunner/tree/1.1.4).
