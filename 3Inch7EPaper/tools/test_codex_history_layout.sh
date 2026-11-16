#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
test_dir=$(mktemp -d "${TMPDIR:-/tmp}/epaper-codex-history.XXXXXX")
trap 'rm -rf "$test_dir"' EXIT HUP INT TERM
${CC:-clang} -std=c11 -D_POSIX_C_SOURCE=200809L -Dcanvas_text=codex_audit_text -Dcanvas_round_box=codex_audit_box -g -Wall -Wextra -Werror -fsanitize=address,undefined -I main -c main/dashboard.c -o "$test_dir/dashboard.o"
${CC:-clang} -std=c11 -Dcanvas_text=codex_audit_text -g -Wall -Wextra -Werror -fsanitize=address,undefined -I main -c main/token_typography.c -o "$test_dir/type.o"
${CC:-clang} -std=c11 -D_POSIX_C_SOURCE=200809L -Wno-deprecated-declarations -g -fsanitize=address,undefined -I main -I managed_components/espressif__cjson/cJSON -I managed_components/espressif__qrcode \
 tests/test_codex_history_layout.c "$test_dir/dashboard.o" "$test_dir/type.o" main/quota_reset.c main/lunar_calendar.c main/argus_page.c main/usage_breakdown.c main/canvas.c main/fonts.c main/brand_assets.c main/music_page.c main/music_fonts.c main/music_library.c main/protocol.c main/usage.c main/pairing_qr.c \
 managed_components/espressif__cjson/cJSON/cJSON.c managed_components/espressif__qrcode/qrcodegen.c -lm -o "$test_dir/test"
"$test_dir/test"
