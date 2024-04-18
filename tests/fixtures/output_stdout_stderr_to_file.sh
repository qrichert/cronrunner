#!/usr/bin/env sh

THIS_SCRIPTS_PARENT_DIR=$(dirname "$0")

/usr/bin/env sh "$@" > $THIS_SCRIPTS_PARENT_DIR/output_stdout_stderr.txt 2>&1
