#!/bin/sh
# Host-only countdown semantics; never accesses USB, BLE or account data.
set -eu
cd "$(dirname "$0")/.."
test_binary=$(mktemp "${TMPDIR:-/tmp}/epaper-reset-time.XXXXXX")
trap 'rm -f "$test_binary"' EXIT HUP INT TERM
${CC:-clang} -std=c11 -Wall -Wextra -Werror -fsanitize=address,undefined \
    -I main tests/test_quota_reset.c main/quota_reset.c -o "$test_binary"
"$test_binary"
