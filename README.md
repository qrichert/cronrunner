# cronrunner

_Run cron jobs manually._[^1]

```
$ cronrunner
1. @reboot /usr/bin/bash ~/startup.sh
2. Update brew. 30 20 * * * /usr/local/bin/brew update && /usr/local/bin/brew upgrade
3. Print variable. * * * * * echo $FOO
>>> Select a job to run: 3
$ echo $FOO
bar
```

```crontab
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
```

## Installation

<!--
### Directly

```console
$ wget https://github.com/qrichert/cronrunner/...
```

### Manual Build
-->

### System-wide

```console
$ git clone https://github.com/qrichert/cronrunner.git
$ cd cronrunner
$ make build
$ sudo make install
```

### Through Cargo

```shell
cargo install --git https://github.com/qrichert/cronrunner.git
```

[^1]:
    cronrunner used to be a Python project, see
    [1.1.4](https://github.com/qrichert/cronrunner/tree/1.1.4).
