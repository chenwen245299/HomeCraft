#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
test_dir=$(mktemp -d "${TMPDIR:-/tmp}/epaper-overview-cards.XXXXXX")
trap 'rm -rf "$test_dir"' EXIT HUP INT TERM
for unit in dashboard token_typography brand_assets; do
 ${CC:-clang} -std=c11 -D_POSIX_C_SOURCE=200809L -Dcanvas_text=overview_cards_text -Dcanvas_round_box=overview_cards_box \
  -g -Wall -Wextra -Werror -fsanitize=address,undefined -I main \
  -c "main/$unit.c" -o "$test_dir/$unit.o"
done
${CC:-clang} -std=c11 -D_POSIX_C_SOURCE=200809L -g -Wall -Wextra -Werror -fsanitize=address,undefined \
 -I main -I managed_components/espressif__cjson/cJSON -I managed_components/espressif__qrcode \
 tests/test_overview_cards.c "$test_dir/dashboard.o" "$test_dir/token_typography.o" "$test_dir/brand_assets.o" \
 main/quota_reset.c main/lunar_calendar.c main/argus_page.c main/usage_breakdown.c main/canvas.c main/fonts.c \
 main/music_page.c main/music_fonts.c main/music_library.c main/protocol.c main/usage.c main/pairing_qr.c \
 managed_components/espressif__cjson/cJSON/cJSON.c managed_components/espressif__qrcode/qrcodegen.c -lm -o "$test_dir/test"
"$test_dir/test" "$@"
