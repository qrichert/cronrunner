# CronRunner

_Run cron jobs without the need to schedule them the next minute._

```console
$ cronrunner
1) Update brew 30 20 * * * /usr/local/bin/brew update && /usr/local/bin/brew upgrade
2) Do something @hourly cd $HOME && date > date.txt
3) Print bar * * * * * echo $FOO
4) * * * * * : # Job without header
>>> Select a job to run: 3
$ echo $FOO
bar
```

```crontab
# CronRunner Demo
# ---------------

# Update brew
30 20 * * * /usr/local/bin/brew update && /usr/local/bin/brew upgrade

# Do something
@hourly cd $HOME && date > date.txt

FOO=bar
# Print bar
* * * * * echo $FOO

* * * * * : # Job without header
```

## Installation

```console
$ git clone https://github.com/qrichert/cronrunner.git
$ cd cronrunner
$ sudo make install
```
