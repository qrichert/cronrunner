#!/usr/bin/env sh

# This crontab is a valid crontab with many edge cases.

# `mock_shell` is what lets us monkey-patch the shell. It makes
# `Crontab` run `mock_shell` instead of the default `/bin/sh`, which
# will be added to the `PATH` in the tests' setup.

cat <<'EOF'
SHELL=mock_shell

"DOUBLE_QUOTED_IDENTIFIER"=double_quoted_identifier
'SINGLE_QUOTED_IDENTIFIER'=single_quoted_identifier

DOUBLE_QUOTED_VALUE="double_quoted_value"
SINGLE_QUOTED_VALUE='single_quoted_value'

"DOUBLE_QUOTED_IDENTIFIER_AND_VALUE"="double_quoted_identifier_and_value"
'SINGLE_QUOTED_IDENTIFIER_AND_VALUE'='single_quoted_identifier_and_value'

QUOTED_HASH="quoted # hash"
UNQUOTED_HASH=unquoted # hash

UNEXPANDED_QUOTED=this_value_will_never_be_seen
UNEXPANDED_UNQUOTED=and_this_one_neither

_UNEXPANDED_QUOTED_VAR_="$UNEXPANDED_QUOTED"
_UNEXPANDED_UNQUOTED_VAR_=$UNEXPANDED_UNQUOTED

* * * * * echo "$DOUBLE_QUOTED_IDENTIFIER\n$SINGLE_QUOTED_IDENTIFIER\n$DOUBLE_QUOTED_VALUE\n$SINGLE_QUOTED_VALUE\n$DOUBLE_QUOTED_IDENTIFIER_AND_VALUE\n$SINGLE_QUOTED_IDENTIFIER_AND_VALUE\n$QUOTED_HASH\n$UNQUOTED_HASH\n$_UNEXPANDED_QUOTED_VAR_\n$_UNEXPANDED_UNQUOTED_VAR_"
EOF
