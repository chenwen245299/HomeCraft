#!/bin/sh
# Host-only synthetic layout checks. Does not open USB or BLE.
set -eu
cd "$(dirname "$0")/.."
test_binary=$(mktemp "${TMPDIR:-/tmp}/epaper-token-type.XXXXXX")
trap 'rm -f "$test_binary"' EXIT HUP INT TERM
${CC:-clang} -std=c11 -g -Wall -Wextra -Werror -fsanitize=address,undefined \
    -I main tests/test_token_typography.c main/token_typography.c main/canvas.c main/fonts.c -o "$test_binary"
"$test_binary"
