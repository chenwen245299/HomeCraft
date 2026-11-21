#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
test_binary=$(mktemp "${TMPDIR:-/tmp}/epaper-ble-advertising.XXXXXX")
trap 'rm -f "$test_binary"' EXIT HUP INT TERM
${CC:-clang} -std=c11 -g -Wall -Wextra -Werror -fsanitize=address,undefined \
 -I main -I tests/ble_stubs tests/test_ble_advertising.c main/ble_advertising.c -o "$test_binary"
"$test_binary"
