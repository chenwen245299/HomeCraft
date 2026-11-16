#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
test_binary=$(mktemp "${TMPDIR:-/tmp}/epaper-power.XXXXXX")
trap 'rm -f "$test_binary"' EXIT HUP INT TERM
${CC:-clang} -std=c11 -Wall -Wextra -Werror -g -fsanitize=address,undefined -I main \
    tests/test_power_events.c main/power_events.c -o "$test_binary"
"$test_binary"
${CC:-clang} -std=c11 -D_POSIX_C_SOURCE=200809L -Wall -Wextra -Werror -g -fsanitize=address,undefined \
    -I main -I managed_components/espressif__cjson/cJSON -I managed_components/espressif__qrcode \
    tests/test_power_render.c main/dashboard.c main/quota_reset.c main/lunar_calendar.c main/argus_page.c \
    main/usage_breakdown.c main/token_typography.c main/canvas.c main/fonts.c main/brand_assets.c main/music_page.c main/music_fonts.c main/music_library.c \
    main/protocol.c main/usage.c main/pairing_qr.c main/refresh_policy.c \
    managed_components/espressif__cjson/cJSON/cJSON.c managed_components/espressif__qrcode/qrcodegen.c \
    -lm -o "$test_binary"
"$test_binary"
