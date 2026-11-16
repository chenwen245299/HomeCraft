#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
test_dir=$(mktemp -d "${TMPDIR:-/tmp}/epaper-argus-layout.XXXXXX")
trap 'rm -rf "$test_dir"' EXIT HUP INT TERM
${CC:-clang} -std=c11 -D_POSIX_C_SOURCE=200809L -Dcanvas_text=argus_audit_text \
    -g -Wall -Wextra -Werror -fsanitize=address,undefined -I main -c main/argus_page.c -o "$test_dir/argus.o"
${CC:-clang} -std=c11 -D_POSIX_C_SOURCE=200809L -g -Wall -Wextra -Werror -fsanitize=address,undefined \
    -I main -I managed_components/espressif__cjson/cJSON tests/test_argus_layout.c "$test_dir/argus.o" \
    main/token_typography.c main/canvas.c main/fonts.c main/usage_breakdown.c main/usage.c main/protocol.c \
    managed_components/espressif__cjson/cJSON/cJSON.c -lm -o "$test_dir/layout"
"$test_dir/layout"
