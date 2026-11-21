#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
test_binary=$(mktemp "${TMPDIR:-/tmp}/homecraft-display.XXXXXX")
trap 'rm -f "$test_binary"' EXIT HUP INT TERM
${CC:-clang} -std=c11 -D_POSIX_C_SOURCE=200809L -g -Wall -Wextra -Werror -fsanitize=address,undefined \
 -I main -I components/cjson tests/test_display_snapshots.c main/usage.c components/cjson/cJSON.c -lm -o "$test_binary"
"$test_binary"
