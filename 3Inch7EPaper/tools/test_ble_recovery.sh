#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
test_binary=$(mktemp "${TMPDIR:-/tmp}/epaper-ble-recovery.XXXXXX")
trap 'rm -f "$test_binary"' EXIT HUP INT TERM
${CC:-clang} -std=c11 -g -Wall -Wextra -Werror -fsanitize=address,undefined -I main \
 tests/test_ble_recovery.c main/ble_recovery.c -o "$test_binary"
"$test_binary"
