// Portable UI preview and protocol boundary checks; fixtures never run on the device.
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <math.h>
#include "dashboard.h"

int main(int argc, char **argv)
{
    quota_snapshot_t q;
    assert(quota_parse("EPAPER1 1792496164 440 300 1792496653 570 10080 1792890838 0\n", &q));
    assert(q.primary.remaining_tenths == 440 && q.secondary.window_mins == 10080);
    assert(quota_parse("EPAPER1 1792496164 -1 0 0 -1 0 0 1", &q));
    assert(q.primary.remaining_tenths == -1);
    assert(!quota_parse("EPAPER1 1792496164 1001 300 0 500 10080 0 0", &q));
    assert(!quota_parse("EPAPER1 1792496164 500 300 0 500 10080 0 0 trailing", &q));
    assert(!quota_parse("EPAPER1 1792496164 999999999999999999999999999 300 0 500 10080 0 0", &q));
    assert(!quota_parse("EPAPER1 1792496164 500 300 0 500 10080 0", &q));
    assert(!quota_parse("EPAPER1x 1792496164 500 300 0 500 10080 0 0", &q));
    assert(!quota_parse("EPAPER1 1792496164 4294967796 300 0 500 10080 0 0", &q));
    assert(!quota_parse("EPAPER1 1 500 300 0 500 10080 0 0", &q));
    setenv("TZ", "SGT-8", 1);
    tzset();
    uint8_t *guarded = malloc(SCREEN_BYTES + 32);
    memset(guarded, 0xA5, SCREEN_BYTES + 32);
    canvas_t canvas = {.pixels = guarded + 16};
    if (argc > 3) canvas.rotation = (unsigned)atoi(argv[3]);
    dashboard_state_t state = {
        .quota = {.fetched_at = 1792496164,
                  .primary = {440, 300, 1792496653},
                  .secondary = {570, 10080, 1792890838}},
        .has_quota = true, .quota_age_sec = 15,
        .sensor_valid = true, .temperature = 25.8f, .humidity = 61.2f,
        .auto_rotate = true, .imu_valid = true,
        .power = {.valid = true, .usb_present = true, .percent = -1},
    };
    if (argc > 4) state.page = (dashboard_page_t)atoi(argv[4]);
    state.network.started = state.network.connected = state.network.clock_ready = true;
    state.network.wifi_configured = true; state.network.interval_seconds = 300;
    strcpy(state.network.ssid, "Preview WiFi");
    strcpy(state.network.ip, "192.168.1.100");
    strcpy(state.network.ap_ssid, "EPaper-DEMO");
    strcpy(state.network.ap_password, "preview-only");
    strcpy(state.network.setup_code, "DEMO1234");
    state.network.setup_active = state.page == PAGE_NETWORK;
    if (argc > 2 && strncmp(argv[2], "ble", 3) == 0) {
        state.network.setup_active = false; state.network.ble_active = true;
        state.network.ble_status = strcmp(argv[2], "ble-failed") == 0 ? 3 : 0;
        strcpy(state.network.ble_name, "PROV_DEMO01");
        strcpy(state.network.ble_pop, "00112233445566778899aabbccddeeff");
    }
    if (argc>2 && strncmp(argv[2],"data",4)==0) {
        state.network.setup_active=false;
        state.network.data_mode=true;state.network.ble_active=true;
        state.network.ble_link=strcmp(argv[2],"data-wait")!=0;
        state.network.data_pairing_open=!state.network.ble_link;
        state.network.ble_received_at=state.network.ble_link?time(NULL):0;
        strcpy(state.network.data_name,"EPaper_DEMO01");
        if(!strcmp(argv[2],"data-pairing")) {
            state.network.data_enrollment=state.network.data_pairing_open=true;
            strcpy(state.network.pairing_code,"01234567");
        }
        if(!strcmp(argv[2],"data-paired"))state.network.data_enrollment=true;
    }
    for (unsigned i = 0; i < PROVIDER_COUNT; ++i) {
        usage_record_t *r = &state.network.providers[i];
        usage_record_init(r);
        if(i>=3)continue;
        r->configured = r->has_data = true; r->status = USAGE_OK; r->fetched_at = time(NULL);
        if (i < 2) {
            r->windows[0] = state.quota.primary; r->windows[1] = state.quota.secondary;
        } else {
            r->balance_available = true; r->balance = 110; strcpy(r->currency, "CNY");
        }
    }
    if(argc>2 && !strncmp(argv[2],"data",4)){
        time_t now=time(NULL);struct tm local;localtime_r(&now,&local);
        int day=(local.tm_year+1900)*10000+(local.tm_mon+1)*100+local.tm_mday;
        state.network.providers[0].token_day=day;
        state.network.providers[0].daily_tokens=12345678;
        state.network.providers[0].token_week=usage_week_start(day);
        state.network.providers[0].weekly_tokens=118459404;
        state.network.providers[0].token_month=usage_month_start(day);
        state.network.providers[0].monthly_tokens=230000000;
        state.network.providers[0].yesterday_day=usage_previous_day(day);
        state.network.providers[0].yesterday_tokens=100015606;
        state.network.providers[0].yesterday_week_used_tenths=280;
        state.network.providers[0].latest_token_day=day;
        state.network.providers[0].latest_daily_tokens=12345678;
        if(!strcmp(argv[2],"data-partial")) {
            usage_record_init(&state.network.providers[1]);usage_record_init(&state.network.providers[2]);
            state.network.providers[0].daily_tokens=-1;
            state.network.providers[0].latest_token_day=20261019;
            state.network.providers[0].latest_daily_tokens=28298767;
        }
    }
    if (argc > 2 && strcmp(argv[2], "waiting") == 0) {
        state.has_quota = false;
        for (unsigned i = 0; i < PROVIDER_COUNT; ++i) usage_record_init(&state.network.providers[i]);
    }
    if (argc > 2 && strcmp(argv[2], "stale") == 0) { state.quota_age_sec = 181; state.network.connected = false; }
    if (argc > 2 && strcmp(argv[2], "sensor-offline") == 0) state.sensor_valid = false;
    if (argc > 2 && strcmp(argv[2], "data-full") == 0) {
        state.power = (power_status_t){.valid = true, .battery_present = true, .usb_present = true, .charging = true, .percent = 100};
        state.network.providers[0].windows[0].remaining_tenths = 1000;
        state.network.providers[1].windows[0].remaining_tenths = 1000;
        state.network.providers[0].windows[1].remaining_tenths = 1000;
        state.network.providers[1].windows[1].remaining_tenths = 1000;
        state.network.providers[0].daily_tokens = 9007199254740991LL;
        state.network.providers[0].weekly_tokens = 9007199254740991LL;
        state.network.providers[2].balance = -999999999999.99;
        state.paused = true;
    }
    if (argc > 2 && strcmp(argv[2], "data-empty") == 0) {
        state.power = (power_status_t){.valid = true, .battery_present = true, .percent = 0};
        state.network.providers[0].windows[0].remaining_tenths = 0;
        state.network.providers[0].windows[1].remaining_tenths = 0;
        state.network.providers[0].daily_tokens = 0;
    }
    if (argc > 2 && strcmp(argv[2], "data-unknown-power") == 0)
        state.power = (power_status_t){.percent = -1};
    if(argc>2 && !strncmp(argv[2],"data-argus",10)) {
        usage_record_init(&state.network.providers[1]);usage_record_init(&state.network.providers[2]);
        usage_record_t *r=&state.network.providers[3];r->configured=r->has_data=r->usage_available=r->daily_cost_available=true;
        strcpy(r->name,"Argus");r->status=USAGE_OK;r->fetched_at=time(NULL);
        r->token_day=state.network.providers[0].token_day;r->daily_tokens=3624581;
        r->token_week=usage_week_start(r->token_day);r->weekly_tokens=28542381;
        r->token_month=usage_month_start(r->token_day);r->monthly_tokens=93000000;
        r->input_tokens=3524581;r->output_tokens=100000;r->cached_tokens=1900000;r->calls=138;r->daily_cost_usd=.42;
        if(!strcmp(argv[2],"data-argus-zero")) {
            state.network.providers[0].daily_tokens=-1;
            r->daily_tokens=r->input_tokens=r->output_tokens=r->cached_tokens=r->calls=0;
            r->daily_cost_available=false;
        }
        if(!strcmp(argv[2],"data-argus-missing-week"))r->weekly_tokens=-1;
        if(!strcmp(argv[2],"data-argus-old-week"))r->token_week=20241230;
        if(!strcmp(argv[2],"data-argus-three")) {
            usage_record_t *extra=&state.network.providers[2];extra->configured=extra->has_data=extra->balance_available=true;
            extra->status=USAGE_OK;extra->fetched_at=time(NULL);extra->balance=110;strcpy(extra->currency,"CNY");
        }
        if(!strcmp(argv[2],"data-argus-breakdown")) {
            state.network.providers[0].daily_tokens=-1;
            r->daily_tokens=19000000;r->weekly_tokens=59400000;
            r->input_tokens=18300000;r->output_tokens=700000;r->cached_tokens=11300000;r->calls=2699;
            r->daily_cost_available=false;
            usage_breakdown_t *b=&state.network.argus_breakdown;
            *b=(usage_breakdown_t){.received=true,.day=r->token_day,.fetched_at=r->fetched_at,.total=r->daily_tokens,
                .model_count=3,.provider_count=2,.model_rows=3,.provider_rows=2};
            strcpy(b->models[0].label,"x-test-2");b->models[0].tokens=17400000;
            strcpy(b->models[1].label,"deepseek-v4-flash");b->models[1].tokens=1200000;
            strcpy(b->models[2].label,"deepseek-flash");b->models[2].tokens=400000;
            strcpy(b->providers[0].label,"ID a18b5c90");b->providers[0].tokens=12000000;
            strcpy(b->providers[1].label,"ID 7f6e3d20");b->providers[1].tokens=7000000;
        }
        if(argc>5) {
            r->daily_tokens=strtoll(argv[5],NULL,10);
            if(argc>6)r->weekly_tokens=strtoll(argv[6],NULL,10);
            state.network.providers[0].daily_tokens=-1;
            r->input_tokens=r->daily_tokens;r->output_tokens=r->cached_tokens=0;
        }
    }
    if (argc > 2 && !strncmp(argv[2], "data-reset", 10)) {
        usage_record_t *codex = &state.network.providers[PROVIDER_CODEX];
        usage_record_init(&state.network.providers[PROVIDER_CLAUDE]);
        if (strcmp(argv[2], "data-reset-three")) usage_record_init(&state.network.providers[PROVIDER_DEEPSEEK]);
        codex->windows[0].reset_at = time(NULL) + 8280;
        codex->windows[1].reset_at = time(NULL) + 3*86400;
        if (argc > 5) codex->windows[0].remaining_tenths = atoi(argv[5]);
        if (!strcmp(argv[2], "data-reset-soon")) codex->windows[0].reset_at = time(NULL) + 30;
        if (!strcmp(argv[2], "data-reset-due")) codex->windows[0].reset_at = time(NULL) - 1;
        if (!strcmp(argv[2], "data-reset-unknown")) codex->windows[0].reset_at = 0;
        if (!strcmp(argv[2], "data-reset-two") || !strcmp(argv[2], "data-reset-three")) {
            usage_record_t *argus = &state.network.providers[PROVIDER_ARGUS];
            argus->configured = argus->has_data = true; argus->status = USAGE_OK; argus->fetched_at = time(NULL);
            strcpy(argus->name, "Argus"); argus->token_day = codex->token_day;
            argus->daily_tokens = 14200000; argus->token_week = usage_week_start(argus->token_day);
            argus->weekly_tokens = 54600000;
            argus->token_month = usage_month_start(argus->token_day); argus->monthly_tokens = 93000000;
        }
    }
    if(argc>2 && !strcmp(argv[2],"data-uniform-three")) {
        usage_record_init(&state.network.providers[1]);usage_record_init(&state.network.providers[2]);
        state.network.providers[0].windows[0].reset_at=time(NULL)+8280;
        if(argc>5)state.network.providers[0].windows[0].remaining_tenths=atoi(argv[5]);
        state.network.providers[0].daily_tokens=-1;
        state.network.providers[0].weekly_tokens=702762979;state.network.providers[0].monthly_tokens=1266368358;
        usage_record_t *a=&state.network.providers[3];*a=state.network.providers[0];
        strcpy(a->name,"Argus");a->windows[0].remaining_tenths=a->windows[1].remaining_tenths=-1;
        a->daily_tokens=3561453;a->weekly_tokens=102695426;a->monthly_tokens=245296538;
        usage_record_t *h=&state.network.providers[4];*h=*a;strcpy(h->name,"DeepSeek Harness");
        h->daily_tokens=0;h->weekly_tokens=105206140;h->monthly_tokens=138443538;
    }
    if(argc>2 && !strncmp(argv[2],"data-yesterday",14)) {
        usage_record_init(&state.network.providers[1]);usage_record_init(&state.network.providers[2]);
        usage_record_t *r=&state.network.providers[0];
        if(!strcmp(argv[2],"data-yesterday-missing")){r->yesterday_tokens=-1;r->yesterday_week_used_tenths=-1;}
        if(!strcmp(argv[2],"data-yesterday-zero")){r->yesterday_tokens=0;r->yesterday_week_used_tenths=0;}
        if(!strcmp(argv[2],"data-yesterday-max")){r->yesterday_tokens=9007199254740991LL;r->yesterday_week_used_tenths=1000;}
    }
    if(argc>2 && !strcmp(argv[2],"data-history")) {
        state.history.mounted=state.history.clock_ready=state.history.write_ok=true;
        state.history.observed=state.history.valid_samples=state.history.stored_samples=900;
        state.history.temperature_min=22;state.history.temperature_max=28;state.history.humidity_min=40;state.history.humidity_max=65;
        for(unsigned i=0;i<60;++i){state.history.valid[i]=i<15||i>18;state.history.temperature[i]=2500+300*sin(i/10.0);state.history.humidity[i]=5200+1200*cos(i/13.0);}
    }
    if (argc > 2 && !strncmp(argv[2], "shutdown", 8))
        dashboard_render_shutdown(&canvas, !strcmp(argv[2], "shutdown-failed"));
    else dashboard_render(&canvas, &state);
    canvas_pixel(&canvas, -1, 0, true);
    canvas_pixel(&canvas, 800, 480, true);
    for (unsigned i = 0; i < 16; ++i) {
        assert(guarded[i] == 0xA5);
        assert(guarded[SCREEN_BYTES + 16 + i] == 0xA5);
    }
    if (argc > 1) {
        FILE *out = fopen(argv[1], "wb");
        assert(out);
        fprintf(out, "P4\n%d %d\n", SCREEN_WIDTH, SCREEN_HEIGHT);
        for (unsigned i = 0; i < SCREEN_BYTES; ++i) fputc(~canvas.pixels[i] & 0xFF, out);
        fclose(out);
    }
    free(guarded);
    puts("UI bounds and protocol checks: OK");
    return 0;
}
