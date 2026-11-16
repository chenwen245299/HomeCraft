#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
test_binary=$(mktemp "${TMPDIR:-/tmp}/epaper-sample-refresh.XXXXXX")
trap 'rm -f "$test_binary"' EXIT HUP INT TERM
${CC:-clang} -std=c11 -g -fsanitize=address,undefined -I main \
    tests/test_sample_refresh.c main/sample_refresh.c main/refresh_policy.c -o "$test_binary"
"$test_binary"
${CC:-clang} -std=c11 -g -fsanitize=address,undefined \
    -I main -I managed_components/espressif__cjson/cJSON \
    tests/test_device_protocol.c main/device_protocol.c managed_components/espressif__cjson/cJSON/cJSON.c \
    -lm -o "$test_binary"
"$test_binary"
