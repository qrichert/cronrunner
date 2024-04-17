#!/usr/bin/env sh

cat <<EOF
SHELL=/bad/shell

@hourly echo "Mock job that can't run, sadly."

EOF
