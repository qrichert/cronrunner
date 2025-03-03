#!/usr/bin/env sh

# `mock_shell` is what lets us monkey-patch the shell. It makes
# `Crontab` run `mock_shell` instead of the default `/bin/sh`, which
# will be added to the `PATH` in the tests' setup.

cat <<EOF
SHELL=mock_shell

## First job.
@hourly echo "Job numero uno"

FOO=miam

## Second job.
@daily echo ":)"

## Third job.
@yearly echo "hello, world"
EOF
