#!/usr/bin/env sh

# Example from `man 5 crontab`.

cat <<EOF
# use /bin/sh to run commands, overriding the default set by cron
SHELL=/bin/sh
# mail any output to \`paul', no matter whose crontab this is
MAILTO=paul
#
# run five minutes after midnight, every day
5 0 * * *       \$HOME/bin/daily.job >> \$HOME/tmp/out 2>&1
# run at 2:15pm on the first of every month -- output mailed to paul
15 14 1 * *     \$HOME/bin/monthly
# run at 10 pm on weekdays, annoy Joe
0 22 * * 1-5    mail -s "It's 10pm" joe%Joe,%%Where are your kids?%
23 0-23/2 * * * echo "run 23 minutes after midn, 2am, 4am ..., everyday"
5 4 * * sun     echo "run at 5 after 4 every sunday"
EOF
