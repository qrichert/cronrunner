# CronRunner

_Run cron jobs manually._

```console
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

- Requires Python 3.9+

### Directly

```console
$ git clone https://github.com/qrichert/cronrunner.git
$ cd cronrunner
$ sudo make install
```

### Through `pip`

```console
$ python3 -m pip install git+https://github.com/qrichert/cronrunner.git
```
