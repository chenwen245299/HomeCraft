// Render real UI frames and use the same layout key/policy as the firmware.
// No sensor, SD card, BLE radio or display controller is accessed by this test.
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include "dashboard.h"
#include "refresh_policy.h"

static uint8_t before[SCREEN_BYTES], after[SCREEN_BYTES];

static dashboard_state_t initial_state(dashboard_page_t page)
{
    dashboard_state_t s = {
        .page = page, .auto_rotate = true, .imu_valid = true,
        .sensor_valid = true, .temperature = 25.0f, .humidity = 50.0f,
        .power = {.valid = true, .usb_present = true, .percent = -1},
        .history = {
            .mounted = true, .clock_ready = true, .write_ok = true,
            .day = 20261021, .observed = 2, .valid_samples = 2, .stored_samples = 2,
            .temperature_min = 25, .temperature_max = 25,
            .humidity_min = 50, .humidity_max = 50,
        },
    };
    for (unsigned i = 0; i < PROVIDER_COUNT; ++i) usage_record_init(&s.network.providers[i]);
    usage_record_t *codex = &s.network.providers[PROVIDER_CODEX];
    codex->configured = codex->has_data = true;
    codex->status = USAGE_OK;
    codex->fetched_at = time(NULL);
    codex->windows[0] = (quota_window_t){600, 300, time(NULL) + 3600};
    codex->windows[1] = (quota_window_t){840, 10080, time(NULL) + 86400};
    s.network.started = s.network.data_mode = s.network.ble_active = s.network.ble_link = true;
    s.network.ble_received_at = time(NULL);
    s.history.valid[0] = true;
    s.history.temperature[0] = 2500;
    s.history.humidity[0] = 5000;
    return s;
}

static void expect_partial(const dashboard_state_t *old, const dashboard_state_t *next,
                           unsigned rotation)
{
    canvas_t a = {.pixels = before, .rotation = rotation};
    canvas_t b = {.pixels = after, .rotation = rotation};
    dashboard_render(&a, old);
    dashboard_render(&b, next);
    assert(memcmp(before, after, SCREEN_BYTES) != 0); // The update must reach the pixels.
    uint32_t key = dashboard_layout_key(old, &a);
    assert(key == dashboard_layout_key(next, &b));
    refresh_policy_t policy = {.enabled = true};
    refresh_commit(&policy, REFRESH_FULL, true, key, 1000000);
    assert(refresh_decide(&policy, true, false, dashboard_layout_key(next, &b), 61000000) == REFRESH_DIFFERENTIAL);
    assert(refresh_decide(&policy, true, true, key, 61000000) == REFRESH_FULL); // BOOT recovery still wins.
    assert(refresh_decide(&policy, true, false, key, 1801000000) == REFRESH_DIFFERENTIAL); // No timed flashing.
}

int main(void)
{
    setenv("TZ", "SGT-8", 1);
    tzset();
    unsigned cases = 0;
    for (unsigned rotation = 0; rotation < 360; rotation += 90) {
        for (unsigned i = 0; i < 2; ++i) {
            dashboard_state_t s = initial_state(i ? PAGE_ENVIRONMENT : PAGE_OVERVIEW);
            dashboard_state_t changed = s;
            changed.temperature = 26.4f;
            expect_partial(&s, &changed, rotation); ++cases;
            changed = s; changed.humidity = 61.2f;
            expect_partial(&s, &changed, rotation); ++cases;
            changed = s; changed.sensor_valid = false;
            expect_partial(&s, &changed, rotation); ++cases;
        }
        dashboard_state_t s = initial_state(PAGE_ENVIRONMENT), changed = s;
        // A new minute also updates the in-progress fifteen-minute average.
        changed.history.temperature[0] = 2550; changed.history.humidity[0] = 5100;
        expect_partial(&s, &changed, rotation); ++cases;
        changed = s;
        changed.history.valid[1] = true;
        changed.history.temperature[1] = 2650; changed.history.humidity[1] = 6000;
        expect_partial(&s, &changed, rotation); ++cases;
        changed = s;
        changed.history.temperature_min = 22; changed.history.temperature_max = 30;
        changed.history.humidity_min = 30; changed.history.humidity_max = 80;
        expect_partial(&s, &changed, rotation); ++cases;
        changed = s; ++changed.history.stored_samples;
        expect_partial(&s, &changed, rotation); ++cases;
        changed = s; changed.history.mounted = false;
        expect_partial(&s, &changed, rotation); ++cases;
        // Midnight clears yesterday's curve; page geometry remains unchanged.
        changed = s;
        changed.history = (environment_history_t){.mounted = true, .clock_ready = true, .day = 20261022};
        expect_partial(&s, &changed, rotation); ++cases;

        // A minute of reset countdown changes pixels without a layout/full-refresh change.
        for (unsigned p = 0; p < 2; ++p) {
            dashboard_state_t quota = initial_state(p ? PAGE_QUOTA : PAGE_OVERVIEW);
            dashboard_state_t next = quota;
            next.network.providers[PROVIDER_CODEX].windows[0].reset_at -= 60;
            expect_partial(&quota, &next, rotation); ++cases;
        }
        dashboard_state_t overview=initial_state(PAGE_OVERVIEW),updated=overview;
        time_t now=time(NULL);struct tm local;localtime_r(&now,&local);
        int day=(local.tm_year+1900)*10000+(local.tm_mon+1)*100+local.tm_mday;
        updated.network.providers[PROVIDER_CODEX].token_week=usage_week_start(day);
        updated.network.providers[PROVIDER_CODEX].weekly_tokens=123456789;
        expect_partial(&overview,&updated,rotation);++cases;
        updated=overview;
        updated.network.providers[PROVIDER_CODEX].token_month=usage_month_start(day);
        updated.network.providers[PROVIDER_CODEX].monthly_tokens=230000000;
        expect_partial(&overview,&updated,rotation);++cases;

        canvas_t canvas = {.pixels = before, .rotation = rotation};
        uint32_t key = dashboard_layout_key(&s, &canvas);
        refresh_policy_t policy = {.enabled = true};
        refresh_commit(&policy, REFRESH_FULL, true, key, 1000000);
        canvas.rotation = (rotation + 90) % 360;
        assert(refresh_decide(&policy, true, false, dashboard_layout_key(&s, &canvas), 61000000) == REFRESH_FULL);
        canvas.rotation = rotation; changed = s; changed.page = PAGE_OVERVIEW;
        assert(refresh_decide(&policy, true, false, dashboard_layout_key(&changed, &canvas), 61000000) == REFRESH_FULL);
    }
    printf("Environment/history/SD/midnight/quota-countdown updates: %u rendered cases use differential refresh; rotation/page/manual recovery retained; no timed full refresh.\n", cases);
}
