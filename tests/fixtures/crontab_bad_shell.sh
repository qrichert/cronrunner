#!/usr/bin/env sh

# This crontab contains an error: the shell does not exist.

cat <<EOF
SHELL=/bad/shell

@hourly echo "Mock job that can't run, sadly."
EOF
