#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
test_dir=$(mktemp -d "${TMPDIR:-/tmp}/epaper-totals.XXXXXX")
trap 'rm -rf "$test_dir"' EXIT HUP INT TERM
${CC:-clang} -std=c11 -D_POSIX_C_SOURCE=200809L -g -Wall -Wextra -Werror \
    -fsanitize=address,undefined -I main -I managed_components/espressif__cjson/cJSON \
    tests/test_usage_totals.c main/usage.c managed_components/espressif__cjson/cJSON/cJSON.c \
    -lm -o "$test_dir/totals"
"$test_dir/totals"
