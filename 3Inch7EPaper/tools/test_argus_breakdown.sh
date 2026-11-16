#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
test_binary=$(mktemp "${TMPDIR:-/tmp}/epaper-breakdown.XXXXXX")
trap 'rm -f "$test_binary"' EXIT HUP INT TERM
${CC:-clang} -std=c11 -D_POSIX_C_SOURCE=200809L -g -Wall -Wextra -Werror -fsanitize=address,undefined \
 -I main -I managed_components/espressif__cjson/cJSON tests/test_usage_breakdown.c \
 main/usage_breakdown.c main/usage.c main/protocol.c managed_components/espressif__cjson/cJSON/cJSON.c -lm -o "$test_binary"
"$test_binary"
