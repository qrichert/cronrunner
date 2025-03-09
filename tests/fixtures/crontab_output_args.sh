#!/usr/bin/env sh

# Instead of printing a mock crontab, this outputs the args that have
# been passed to the `crontab` executable (e.g., `crontab -l` → `-l`).
# This won't work with normal lib usage, but we can `Reader::read()` the
# crontab to get whatever this script outputs in a `String` variable.

echo "$@"
