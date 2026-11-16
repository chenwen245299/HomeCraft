#!/bin/sh
# Host-only render/policy regression. Never opens the board, BLE or SD card.
set -eu
cd "$(dirname "$0")/.."
test_binary=$(mktemp "${TMPDIR:-/tmp}/epaper-env-refresh.XXXXXX")
trap 'rm -f "$test_binary"' EXIT HUP INT TERM
${CC:-clang} -std=c11 -D_POSIX_C_SOURCE=200809L -g -fsanitize=address,undefined \
    -I main -I managed_components/espressif__cjson/cJSON -I managed_components/espressif__qrcode \
    tests/test_environment_refresh.c main/dashboard.c main/quota_reset.c main/lunar_calendar.c main/argus_page.c main/usage_breakdown.c main/token_typography.c main/canvas.c main/fonts.c \
    main/brand_assets.c main/music_page.c main/music_fonts.c main/music_library.c main/protocol.c main/usage.c main/pairing_qr.c main/refresh_policy.c \
    managed_components/espressif__cjson/cJSON/cJSON.c \
    managed_components/espressif__qrcode/qrcodegen.c -lm -o "$test_binary"
"$test_binary"
