#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
test_binary=$(mktemp "${TMPDIR:-/tmp}/epaper-music.XXXXXX")
trap 'rm -f "$test_binary"' EXIT HUP INT TERM
${CC:-clang} -std=c11 -D_POSIX_C_SOURCE=200809L -g -Wall -Wextra -Werror -fsanitize=address,undefined -I main \
 tests/test_music.c main/music_library.c main/music_pcm.c -o "$test_binary"
"$test_binary"
${CC:-clang} -std=c11 -D_POSIX_C_SOURCE=200809L -g -Wall -Wextra -Werror -fsanitize=address,undefined \
 -I main -I managed_components/espressif__cjson/cJSON -I managed_components/espressif__qrcode \
 tests/render_music.c main/dashboard.c main/music_page.c main/music_fonts.c main/music_library.c \
 main/quota_reset.c main/lunar_calendar.c main/argus_page.c main/usage_breakdown.c main/token_typography.c \
 main/canvas.c main/fonts.c main/brand_assets.c main/protocol.c main/usage.c main/pairing_qr.c main/refresh_policy.c \
 managed_components/espressif__cjson/cJSON/cJSON.c managed_components/espressif__qrcode/qrcodegen.c -lm -o "$test_binary"
"$test_binary" "${1:-/private/tmp/homecraft-music-0236}"
