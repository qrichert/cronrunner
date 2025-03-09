#!/usr/bin/env sh

THIS_SCRIPTS_PARENT_DIR=$(dirname "$0")

echo "$@" > $THIS_SCRIPTS_PARENT_DIR/output_args.txt
