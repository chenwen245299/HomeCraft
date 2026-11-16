#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include "driver/usb_serial_jtag.h"
#include "esp_heap_caps.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "esp_pm.h"
#include "energy_policy.h"
#include "esp_sleep.h"
#include "esp_app_desc.h"
#include "freertos/FreeRTOS.h"
#include "freertos/queue.h"
#include "freertos/task.h"
#include "board.h"
#include "power_events.h"
#include "music.h"
#include "controls.h"
#include "dashboard.h"
#include "epaper_port.h"
#include "network.h"
#include "environment_log.h"
#include "refresh_policy.h"
#include "sample_refresh.h"
#include "shutdown.h"
#include "nvs.h"

static const char *TAG = "dashboard";
static QueueHandle_t quota_queue, input_queue, telemetry_queue, display_queue;
static TaskHandle_t hardware_input_handle;
typedef enum { DISPLAY_FULL=1,DISPLAY_TEST,DISPLAY_PARTIAL_ON,DISPLAY_PARTIAL_OFF,DISPLAY_STATUS } display_command_t;
static bool power_key_available, imu_available;
typedef struct { unsigned key; gesture_t gesture; } key_event_t;
typedef struct {
    float temperature, humidity;
    bool sensor_valid, imu_valid;
    unsigned rotation;
    int64_t sampled_at;
    uint64_t sampled_ms;
    uint32_t sample_id;
    power_status_t power;
    uint64_t power_event_ms;
} telemetry_t;

typedef struct {uint64_t at_ms;bool shown;power_status_t power;} power_trace_t;
static power_trace_t power_trace[8];
static unsigned power_trace_count;
static portMUX_TYPE power_trace_lock=portMUX_INITIALIZER_UNLOCKED;
static void power_trace_add(const power_status_t *power,bool shown,uint64_t at_ms)
{
    portENTER_CRITICAL(&power_trace_lock);
    power_trace[power_trace_count++%8]=(power_trace_t){at_ms,shown,*power};
    portEXIT_CRITICAL(&power_trace_lock);
}

static void usb_input_task(void *unused)
{
    (void)unused;
    enum { LINE_CAPACITY = 33024 };
    char *line = malloc(LINE_CAPACITY);
    assert(line);
    size_t used = 0;
    bool overflow = false;
    uint8_t chunk[128];
    for (;;) {
        int count = usb_serial_jtag_read_bytes(chunk, sizeof(chunk), pdMS_TO_TICKS(100));
        for (int i = 0; i < count; ++i) {
            char ch = (char)chunk[i];
            if (ch == '\n') {
                line[used] = 0;
                quota_snapshot_t q;
                if (!overflow && strncmp(line, "EPAPER_CONFIG ", 14) == 0) {
                    esp_err_t result = network_apply_config_json(line + 14);
                    memset(line, 0, used);
                    printf("EPAPER_CONFIG_ACK %d\n", result);
                } else if (!overflow && quota_parse(line, &q)) {
                    xQueueOverwrite(quota_queue, &q);
                    printf("EPAPER_ACK %lld\n", (long long)q.fetched_at);
                } else if (!overflow && strcmp(line, "EPAPER_HELLO") == 0) {
                    printf("EPAPER_READY 3\n");
                } else if (!overflow && strcmp(line,"EPAPER_INFO")==0) {
                    const esp_app_desc_t *app=esp_app_get_description();
                    printf("EPAPER_INFO version=%s project=%s idf=%s\n",app->version,app->project_name,app->idf_ver);
                } else if (!overflow && !strcmp(line,"EPAPER_BLE_STATUS")) {
                    network_ble_status_t b;network_get_ble_status(&b);
                    printf("EPAPER_BLE_STATUS initialized=%d host=%d synced=%d advertising=%d connections=%u data=%d enrolling=%d link=%d recoveries=%u error=%d\n",
                        b.initialized,b.enabled,b.synced,b.advertising,b.connections,b.data_mode,b.enrolling,b.authenticated,b.recoveries,b.recovery_error);
                } else if (!overflow && !strcmp(line,"EPAPER_BLE_RECOVER")) {
                    printf("EPAPER_BLE_RECOVER accepted=%d\n",network_request_ble_recovery());
                } else if (!overflow && !strcmp(line,"EPAPER_USAGE_STATS")) {
                    static network_snapshot_t n;network_get_snapshot(&n);
                    for(unsigned i=0;i<PROVIDER_COUNT;++i){
                        const usage_record_t *r=&n.providers[i];
                        if(!r->configured && !r->has_data)continue;
                        printf("EPAPER_USAGE_STATS provider=%u status=%u day=%d daily=%lld week=%d weekly=%lld month=%d monthly=%lld\n",
                            i,r->status,r->token_day,(long long)r->daily_tokens,r->token_week,(long long)r->weekly_tokens,r->token_month,(long long)r->monthly_tokens);
                        if(i==PROVIDER_CODEX)printf("EPAPER_YESTERDAY day=%d tokens=%lld week_used=%d\n",r->yesterday_day,(long long)r->yesterday_tokens,r->yesterday_week_used_tenths);
                    }
                    printf("EPAPER_TOTALS stored=%d day=%d daily=%lld week=%d weekly=%lld month=%d monthly=%lld\n",
                        n.hub_totals.received,n.hub_totals.periods[0],(long long)n.hub_totals.tokens[0],
                        n.hub_totals.periods[1],(long long)n.hub_totals.tokens[1],n.hub_totals.periods[2],(long long)n.hub_totals.tokens[2]);
                } else if (!overflow && strcmp(line,"EPAPER_ENERGY")==0) {
                    esp_pm_config_t pm;esp_pm_get_configuration(&pm);
                    uint64_t count,us;energy_counters(&count,&us);
                    static network_snapshot_t n;network_get_snapshot(&n);
                    printf("EPAPER_ENERGY cpu_min=%d cpu_max=%d auto_sleep=%d sleep_count=%llu slept_us=%llu wifi_running=%d sensor_ms=%u imu_ms=%u\n",pm.min_freq_mhz,pm.max_freq_mhz,pm.light_sleep_enable,(unsigned long long)count,(unsigned long long)us,n.wifi_running,ENV_SAMPLE_MS,IMU_SAMPLE_MS);
                    uint64_t telemetry,history;energy_work_counters(&telemetry,&history);
                    printf("EPAPER_WORK uptime_ms=%llu telemetry=%llu history=%llu\n",(unsigned long long)(esp_timer_get_time()/1000),(unsigned long long)telemetry,(unsigned long long)history);
                } else if (!overflow && strcmp(line,"EPAPER_IMU_MODE")==0) {
                    uint8_t ctrl2=0,ctrl7=0;esp_err_t err=board_read_imu_config(&ctrl2,&ctrl7);
                    printf("EPAPER_IMU_MODE valid=%d ctrl2=0x%02x ctrl7=0x%02x\n",err==ESP_OK,ctrl2,ctrl7);
                } else if (!overflow && strcmp(line,"EPAPER_HARNESS_STATS")==0) {
                    network_snapshot_t n;network_get_snapshot(&n);
                    const usage_record_t *r=&n.providers[PROVIDER_EXTRA1];
                    printf("EPAPER_HARNESS_STATS devices=%u day=%d daily=%lld week=%d weekly=%lld valid=%d\n",(unsigned)n.harness.count,r->token_day,(long long)r->daily_tokens,r->token_week,(long long)r->weekly_tokens,harness_stats_valid(&n.harness));
                } else if (!overflow && strcmp(line,"EPAPER_ARGUS_STATS")==0) {
                    network_snapshot_t n;network_get_snapshot(&n);
                    const usage_breakdown_t *b=&n.argus_breakdown;
                    printf("EPAPER_ARGUS_STATS received=%d matched=%d day=%d models=%u providers=%u model_rows=%u provider_rows=%u\n",
                        b->received,usage_breakdown_matches(b,&n.providers[PROVIDER_ARGUS]),b->day,b->model_count,b->provider_count,b->model_rows,b->provider_rows);
                } else if (!overflow && strcmp(line, "EPAPER_NETWORK") == 0) {
                    network_snapshot_t n; network_get_snapshot(&n);
                    printf("EPAPER_NETWORK configured=%d connected=%d clock=%d ble=%d ap=%d phase=%u reason=%u ip=%s data=%d link=%d received=%lld enrolling=%d pairing_open=%d\n",
                           n.wifi_configured, n.connected, n.clock_ready, n.ble_active, n.setup_active,
                           n.ble_status, n.wifi_disconnect_reason, n.ip[0] ? n.ip : "--", n.data_mode,n.ble_link,(long long)n.ble_received_at,n.data_enrollment,n.data_pairing_open);
                } else if (!overflow && strcmp(line, "EPAPER_POWER") == 0) {
                    power_status_t power;
                    board_read_power(&power);
                    printf("EPAPER_POWER valid=%d usb=%d battery=%d charging=%d percent=%d\n",
                           power.valid, power.usb_present, power.battery_present,
                           power.charging, power.percent);
                } else if (!overflow && strcmp(line,"EPAPER_POWER_EVENTS")==0) {
                    power_trace_t trace[8];unsigned total;
                    portENTER_CRITICAL(&power_trace_lock);
                    memcpy(trace,power_trace,sizeof(trace));total=power_trace_count;
                    portEXIT_CRITICAL(&power_trace_lock);
                    for(unsigned j=total>8?total-8:0;j<total;++j) {
                        const power_trace_t *p=&trace[j%8];
                        printf("EPAPER_POWER_EVENT kind=%s at_ms=%llu valid=%d usb=%d battery=%d charging=%d percent=%d\n",
                            p->shown?"shown":"source",(unsigned long long)p->at_ms,p->power.valid,p->power.usb_present,p->power.battery_present,p->power.charging,p->power.percent);
                    }
                } else if(!overflow&&!strcmp(line,"EPAPER_MUSIC")) {
                    music_snapshot_t m;music_snapshot(&m);
                    printf("EPAPER_MUSIC state=%u count=%u selected=%u volume=%u elapsed=%u rate=%u bits=%u channels=%u\n",m.state,m.count,m.selected,m.volume,(unsigned)m.elapsed_seconds,(unsigned)m.sample_rate,m.source_bits,m.source_channels);
                    printf("EPAPER_MUSIC_MEMORY internal_free=%u dma_largest=%u psram_free=%u\n",(unsigned)heap_caps_get_free_size(MALLOC_CAP_INTERNAL|MALLOC_CAP_8BIT),(unsigned)heap_caps_get_largest_free_block(MALLOC_CAP_INTERNAL|MALLOC_CAP_DMA),(unsigned)heap_caps_get_free_size(MALLOC_CAP_SPIRAM));
                } else if(!overflow&&!strcmp(line,"EPAPER_MUSIC_SCAN")) {
                    printf("EPAPER_MUSIC_ACK accepted=%d\n",music_command(MUSIC_CMD_SCAN,0));
                } else if(!overflow&&!strcmp(line,"EPAPER_MUSIC_TOGGLE")) {
                    printf("EPAPER_MUSIC_ACK accepted=%d\n",music_command(MUSIC_CMD_TOGGLE,0));
                } else if(!overflow&&!strcmp(line,"EPAPER_MUSIC_STOP")) {
                    printf("EPAPER_MUSIC_ACK accepted=%d\n",music_command(MUSIC_CMD_STOP,0));
                } else if(!overflow&&!strncmp(line,"EPAPER_MUSIC_PLAY ",18)) {
                    char *end;unsigned long index=strtoul(line+18,&end,10);
                    bool valid=end!=line+18&&!*end&&index<MUSIC_MAX_TRACKS;
                    printf("EPAPER_MUSIC_ACK accepted=%d\n",valid&&music_command(MUSIC_CMD_PLAY_INDEX,(unsigned)index));
                } else if (!overflow && strcmp(line,"EPAPER_SD")==0) {
                    environment_history_t h;environment_log_snapshot(&h);
                    printf("EPAPER_SD mounted=%d clock=%d day=%d observed=%u valid=%u stored=%u write_ok=%d error=%d latest=%lld\n",
                           h.mounted,h.clock_ready,h.day,h.observed,h.valid_samples,h.stored_samples,h.write_ok,h.last_error,(long long)h.last_saved_at);
                } else if(!overflow && !strcmp(line,"EPAPER_SD_VERIFY")) {
                    environment_log_verify_request();
                } else if(!overflow && (!strcmp(line,"EPAPER_FULL") || !strcmp(line,"EPAPER_PARTIAL_TEST") || !strcmp(line,"EPAPER_PARTIAL_ON") || !strcmp(line,"EPAPER_PARTIAL_OFF") || !strcmp(line,"EPAPER_DISPLAY_STATUS"))) {
                    display_command_t command=!strcmp(line,"EPAPER_FULL")?DISPLAY_FULL:(!strcmp(line,"EPAPER_PARTIAL_TEST")?DISPLAY_TEST:(!strcmp(line,"EPAPER_PARTIAL_ON")?DISPLAY_PARTIAL_ON:(!strcmp(line,"EPAPER_PARTIAL_OFF")?DISPLAY_PARTIAL_OFF:DISPLAY_STATUS)));
                    printf("EPAPER_REFRESH queued=%d\n",xQueueSend(display_queue,&command,0)==pdPASS);
                } else if (!overflow && strcmp(line,"EPAPER_PAIR") == 0) {
                    char pairing[320];
                    if (network_export_ble_pairing(pairing,sizeof(pairing))==ESP_OK) printf("EPAPER_PAIR %s\n",pairing);
                    else printf("EPAPER_PAIR_DENIED\n");
                    memset(pairing,0,sizeof(pairing));
                }
                used = 0;
                overflow = false;
            } else if (ch != '\r') {
                if (used + 1 < LINE_CAPACITY && !overflow) line[used++] = ch;
                else overflow = true;
            }
        }
    }
}

static void emit_key(unsigned key, gesture_t gesture)
{
    if (gesture == GESTURE_NONE) return;
    static const char *names[] = {"none", "single", "double", "triple", "long", "shutdown"};
    key_event_t event = {key, gesture};
    printf("EPAPER_KEY %s %s\n", key ? "PWR" : "BOOT", names[gesture]);
    if (gesture == GESTURE_SHUTDOWN) {
        if(xQueueSendToFront(input_queue,&event,0)!=pdPASS){xQueueReset(input_queue);xQueueSendToFront(input_queue,&event,0);}
    } else if (xQueueSend(input_queue, &event, 0) != pdPASS)
        ESP_LOGW(TAG, "Input queue full");
}

static void hardware_input_task(void *unused)
{
    (void)unused;
    button_state_t boot = {0};
    power_button_t power = {0};
    bool power_down = false;
    orientation_filter_t orientation = {0};
    telemetry_t t = {0};
    uint64_t next_sensor = 0, next_imu = 0, next_log = 0,next_power=0;
    power_edge_filter_t power_filter={0};
    for (;;) {
        uint64_t now = esp_timer_get_time() / 1000;
        bool publish=false;
        emit_key(0, button_update_with_hold(&boot, board_boot_down(), now, 3000));
        if (power_key_available) {
            uint8_t irq = 0;
            if (board_power_key_irq(&irq) == ESP_OK) {
                if (irq) printf("EPAPER_PWR_IRQ 0x%02x\n", irq);
                if (irq & 0x02) power_down = true;
                if (irq & 0x01) power_down = false;
            } else { power_down=false;power=(power_button_t){0}; } // Never infer shutdown from a stale key state.
            emit_key(1, power_button_update(&power, power_down, now));
        }
        if (now >= next_imu) {
            next_imu = now + IMU_SAMPLE_MS;
            float xyz[3];
            bool previous_valid=t.imu_valid;unsigned previous_rotation=t.rotation;
            t.imu_valid = imu_available && board_read_acceleration(xyz) == ESP_OK;
            if (t.imu_valid) {
                orientation_update(&orientation, xyz[0], xyz[1], xyz[2], now);
                t.rotation = orientation.rotation;
                if (now >= next_log) {
                    printf("EPAPER_IMU %.3f %.3f %.3f rotation=%u\n",
                           (double)xyz[0], (double)xyz[1], (double)xyz[2], t.rotation);
                    next_log = now + 5000;
                }
            } else orientation.initialized = false;
            publish=previous_valid!=t.imu_valid || previous_rotation!=t.rotation;
        }
        if(now>=next_power) {
            next_power=now+POWER_SOURCE_POLL_MS;
            power_status_t observed={.percent=-1};
            board_read_power_source(&observed);
            if(power_edge_observe(&power_filter,&t.power,&observed,now)) {
                power_status_t fresh;
                if(board_read_power(&fresh)==ESP_OK && power_source_equal(&fresh,&observed)) {
                    t.power=fresh;t.power_event_ms=now;publish=true;
                    power_trace_add(&fresh,false,now);
                    printf("EPAPER_POWER_EDGE usb=%d battery=%d charging=%d percent=%d at_ms=%llu\n",
                        fresh.usb_present,fresh.battery_present,fresh.charging,fresh.percent,(unsigned long long)now);
                }
            }
        }
        uint32_t requested_sample=0;
        if (xTaskNotifyWait(0,UINT32_MAX,&requested_sample,0)==pdTRUE) next_sensor=0;
        if (now >= next_sensor) {
            next_sensor = now + ENV_SAMPLE_MS;
            publish=true;
            esp_err_t err = board_read_environment(&t.temperature, &t.humidity);
            t.sensor_valid = err == ESP_OK;
            t.sampled_ms=esp_timer_get_time()/1000;t.sampled_at=time(NULL);
            if(requested_sample)t.sample_id=requested_sample;
            if(t.sampled_at<1735689600)t.sampled_at=0;
            if (t.sensor_valid)
                printf("EPAPER_SENSOR %.2f %.2f\n", (double)t.temperature, (double)t.humidity);
            else ESP_LOGW(TAG, "SHTC3 read failed: %s", esp_err_to_name(err));
            power_status_t fresh;
            board_read_power(&fresh);
            if(!power_source_equal(&fresh,&t.power)) {
                t.power_event_ms=now;power_trace_add(&fresh,false,now);
            }
            t.power=fresh;
            environment_log_sample(t.temperature,t.humidity,t.sensor_valid,t.power.percent);
        }
        if(publish) {xQueueOverwrite(telemetry_queue, &t);energy_note_telemetry();}
        vTaskDelay(pdMS_TO_TICKS(INPUT_POLL_MS));
    }
}

static uint8_t *image_alloc(void)
{
    uint8_t *p = heap_caps_malloc(SCREEN_BYTES, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT);
    return p ? p : heap_caps_malloc(SCREEN_BYTES, MALLOC_CAP_8BIT);
}

static bool display_frame(const uint8_t *frame)
{
    EPD_Init();
    if (!epaper_last_operation_ok()) return false;
    EPD_Display_Base(frame);
    return epaper_last_operation_ok();
}

static bool shutdown_storage(void *unused)
{
    (void)unused;
    return music_prepare_shutdown(2500)&&environment_log_prepare_shutdown(2000)==ESP_OK;
}
static bool shutdown_network(void *unused)
{
    (void)unused;
    return network_prepare_shutdown(3000)==ESP_OK;
}
static bool shutdown_draw(void *context)
{
    canvas_t *c=context;
    dashboard_render_shutdown(c,false);
    return display_frame(c->pixels); // Always full refresh and wait for BUSY completion.
}
static void shutdown_display_sleep(void *unused)
{
    (void)unused;
    EPD_Sleep();
    board_display_power(false);
}
static void shutdown_power(void *unused)
{
    (void)unused;
    esp_err_t err=board_power_off();
    printf("EPAPER_SHUTDOWN pmu_request=%d\n",err);
}
static void shutdown_standby(void *unused)
{
    (void)unused;
    vTaskDelay(pdMS_TO_TICKS(250));
    // Some USB power paths may keep the MCU supplied after the PMU request.
    // No timer wake: do not resume sampling/BLE behind a persistent OFF image.
    esp_sleep_disable_wakeup_source(ESP_SLEEP_WAKEUP_ALL);
    esp_deep_sleep_start();
}
static void shutdown_resume(void *unused)
{
    (void)unused;
    network_resume();
    environment_log_resume();
    music_resume();
}
static const shutdown_ops_t shutdown_ops={shutdown_storage,shutdown_network,shutdown_draw,
    shutdown_display_sleep,shutdown_power,shutdown_standby,shutdown_resume};

static bool partial_setting(bool write,bool enabled)
{
    nvs_handle_t handle;uint32_t value=0;
    if(nvs_open_from_partition("dashboard","display",write?NVS_READWRITE:NVS_READONLY,&handle)!=ESP_OK)return false;
    if(write){if(nvs_set_u32(handle,"partial_v1",enabled?1:0)==ESP_OK)nvs_commit(handle);}
    else nvs_get_u32(handle,"partial_v1",&value);
    nvs_close(handle);return write?enabled:value==1;
}

static bool partial_test(canvas_t *c,uint8_t *previous)
{
    uint8_t *test=image_alloc();if(!test)return false;
    canvas_t t={.pixels=test,.rotation=c->rotation};bool ok=true;
    int y=canvas_width(c)<canvas_height(c)?588:340;
    for(unsigned i=0;i<=8;++i) {
        memcpy(test,c->pixels,SCREEN_BYTES);
        canvas_fill(&t,24,y+4,canvas_width(c)<canvas_height(c)?432:400,74,false);
        canvas_text(&t,36,y+12,"局刷测试",&font24,1,true);
        char step[16];snprintf(step,sizeof(step),"%u/8",i);
        canvas_text(&t,250,y+6,step,&digits48,1,true);
        for(unsigned corner=0;corner<4;++corner)canvas_fill(&t,corner%2?canvas_width(c)-16:8,corner/2?canvas_height(c)-12:4,8,8,i%2!=0);
        int64_t began=esp_timer_get_time();
        ok=i==0?display_frame(test):EPD_Display_Differential(previous,test);
        printf("EPAPER_PARTIAL_TEST step=%u success=%d ms=%lld\n",i,ok,(long long)((esp_timer_get_time()-began)/1000));
        if(!ok)break;
        memcpy(previous,test,SCREEN_BYTES);vTaskDelay(pdMS_TO_TICKS(3000));
    }
    bool restored=ok?EPD_Display_Differential(previous,c->pixels):display_frame(c->pixels);
    if(!restored)restored=display_frame(c->pixels);
    if(restored)memcpy(previous,c->pixels,SCREEN_BYTES);
    free(test);printf("EPAPER_PARTIAL_TEST done=%d restored=%d visual_confirmation_required=1\n",ok,restored);
    return ok&&restored;
}

static void apply_key(key_event_t event, dashboard_state_t *s, canvas_t *c)
{
    if(s->page==PAGE_MUSIC) {
        if(event.key==0&&event.gesture==GESTURE_SINGLE){music_command(MUSIC_CMD_NEXT,0);return;}
        if(event.key==1&&event.gesture==GESTURE_SINGLE){music_command(MUSIC_CMD_PREVIOUS,0);return;}
        if(event.key==0&&event.gesture==GESTURE_DOUBLE){music_command(MUSIC_CMD_TOGGLE,0);return;}
        if(event.key==1&&event.gesture==GESTURE_DOUBLE){music_command(MUSIC_CMD_VOLUME_UP,0);return;}
        if(event.key==1&&event.gesture==GESTURE_LONG){music_command(MUSIC_CMD_VOLUME_DOWN,0);return;}
        if(event.key==0&&event.gesture==GESTURE_TRIPLE){s->page=dashboard_step_page(s,1);return;}
        if(event.key==1&&event.gesture==GESTURE_TRIPLE)music_command(MUSIC_CMD_STOP,0);
    }
    if (event.key == 0) {
        switch (event.gesture) {
            case GESTURE_SINGLE: s->page = dashboard_step_page(s,1); break;
            case GESTURE_DOUBLE:
                s->paused = false;
                if (s->page==PAGE_NETWORK && s->network.data_mode) network_open_data_pairing();
                else network_request_refresh();
                printf("EPAPER_REFRESH_WIFI\n");
                break;
            case GESTURE_TRIPLE:
                if (s->page == PAGE_NETWORK) network_toggle_setup();
                else s->auto_rotate = !s->auto_rotate;
                break;
            case GESTURE_LONG:
                network_toggle_ble(); s->page = PAGE_NETWORK; s->paused = false;
                break;
            default: break;
        }
    } else {
        switch (event.gesture) {
            case GESTURE_SINGLE: s->page = dashboard_step_page(s,-1); break;
            case GESTURE_DOUBLE:
                s->auto_rotate = false;
                c->rotation = (c->rotation + 90) % 360;
                break;
            case GESTURE_TRIPLE:
                s->page = PAGE_OVERVIEW; s->auto_rotate = true; s->paused = false;
                break;
            case GESTURE_LONG: s->paused = !s->paused; break;
            default: break;
        }
    }
}

void app_main(void)
{
    setenv("TZ", "SGT-8", 1);
    tzset();
    energy_start();
    ESP_LOGI(TAG,"EPAPER_ENERGY cpu_min=%d cpu_max=%d auto_light_sleep=1 sensor_ms=%u imu_ms=%u",CPU_MIN_MHZ,CPU_MAX_MHZ,ENV_SAMPLE_MS,IMU_SAMPLE_MS);
    if (!usb_serial_jtag_is_driver_installed()) {
        usb_serial_jtag_driver_config_t usb = {.tx_buffer_size = 2048, .rx_buffer_size = 2048};
        ESP_ERROR_CHECK(usb_serial_jtag_driver_install(&usb));
    }
    quota_queue = xQueueCreate(1, sizeof(quota_snapshot_t));
    telemetry_queue = xQueueCreate(1, sizeof(telemetry_t));
    input_queue = xQueueCreate(16, sizeof(key_event_t));
    display_queue=xQueueCreate(4,sizeof(display_command_t));
    assert(quota_queue && telemetry_queue && input_queue && display_queue);
    assert(xTaskCreate(usb_input_task, "quota_usb", 8192, NULL, 5, NULL) == pdPASS);
    printf("EPAPER_READY 3\n");
    esp_err_t err = board_init();
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Board initialization failed: %s", esp_err_to_name(err));
        return;
    }
    power_key_available = board_inputs_init() == ESP_OK;
    if (!power_key_available) ESP_LOGW(TAG, "PWR input unavailable; BOOT remains usable");
    imu_available = board_imu_init() == ESP_OK;
    if (!imu_available) ESP_LOGW(TAG, "IMU unavailable; use PWR double click for manual rotation");
    if(environment_log_start()!=ESP_OK)ESP_LOGW(TAG,"SD logger unavailable");
    music_start();
    assert(xTaskCreate(hardware_input_task, "board_inputs", 4096, NULL, 4, &hardware_input_handle) == pdPASS);
    err = network_start();
    if (err != ESP_OK) ESP_LOGE(TAG, "Wi-Fi initialization failed: %s", esp_err_to_name(err));
    epaper_port_init();
    uint8_t *frame = image_alloc(), *previous = image_alloc();
    if (!frame || !previous) {
        ESP_LOGE(TAG, "Not enough memory for display buffers");
        free(frame); free(previous); return;
    }
    memset(previous, 0xFF, SCREEN_BYTES);
    canvas_t canvas = {.pixels = frame};
    static dashboard_state_t state = {.auto_rotate = true};
    network_get_snapshot(&state.network);
    if (err == ESP_OK && !state.network.wifi_configured) state.page = PAGE_NETWORK;
    telemetry_t telemetry = {0};
    int64_t quota_received = 0, last_draw = 0,last_power_draw=0;
    power_status_t displayed_power={.percent=-1};
    bool first = true, dirty = true, retry_display = false, urgent = false;
    bool manual_refresh_waiting = false;
    bool force_full=false,test_requested=false,test_complete=false;
    refresh_policy_t refresh={.enabled=partial_setting(false,false)};
    int64_t next_history=0;
    device_frame_t confirmed_frame={0};
    sample_refresh_t sample={0};
    uint32_t reported_sample=0;unsigned reported_result=0;
    xQueueReceive(telemetry_queue, &telemetry, pdMS_TO_TICKS(100));
    for (;;) {
        int64_t now = esp_timer_get_time();
        static network_snapshot_t network;
        network_get_snapshot(&network);
        if (network.revision != state.network.revision) {
            if (network.ble_active != state.network.ble_active || network.ble_status != state.network.ble_status) urgent = true;
            if (network.data_mode != state.network.data_mode || network.ble_link != state.network.ble_link || network.data_pairing_open != state.network.data_pairing_open || network.data_enrollment != state.network.data_enrollment) urgent=true;
            if (network.setup_active != state.network.setup_active || network.connected != state.network.connected) urgent = true;
            for (unsigned i = 0; i < PROVIDER_COUNT; ++i) {
                if (network.providers[i].has_data &&
                    (!state.network.providers[i].has_data ||
                     (manual_refresh_waiting && network.providers[i].fetched_at != state.network.providers[i].fetched_at))) {
                    urgent = true; manual_refresh_waiting = false;
                }
            }
            dirty = true;
        }
        state.network = network;
        static music_snapshot_t music;music_snapshot(&music);
        if(music.revision!=state.music.revision&&state.page==PAGE_MUSIC)dirty=urgent=true;
        state.music=music;
        dashboard_page_t available_page=dashboard_step_page(&state,0);
        if(available_page!=state.page){state.page=available_page;dirty=urgent=true;}
        quota_snapshot_t q;
        if (xQueueReceive(quota_queue, &q, 0) == pdTRUE &&
            (!state.has_quota || q.fetched_at >= state.quota.fetched_at)) {
            if (!state.has_quota || q.fetched_at != state.quota.fetched_at) {
                if (!state.has_quota || manual_refresh_waiting) urgent = true;
                manual_refresh_waiting = false;
                quota_received = now; dirty = true;
            }
            if (q.status != state.quota.status) dirty = true;
            state.quota = q; state.has_quota = true;
        }
        unsigned previous_age = state.quota_age_sec;
        if (state.has_quota) state.quota_age_sec = (unsigned)((now - quota_received) / 1000000);
        if (previous_age <= 180 && state.quota_age_sec > 180) dirty = true;
        xQueueReceive(telemetry_queue, &telemetry, 0);
        if (state.sensor_valid != telemetry.sensor_valid || state.imu_valid != telemetry.imu_valid) dirty = true;
        state.sensor_valid = telemetry.sensor_valid;
        state.temperature = telemetry.temperature; state.humidity = telemetry.humidity;
        state.imu_valid = telemetry.imu_valid;
        if(!power_icon_equal(&state.power,&telemetry.power))dirty=true;
        state.power = telemetry.power;
        if(sample_refresh_observe(&sample,telemetry.sample_id,telemetry.sampled_ms,telemetry.sensor_valid,(uint64_t)now/1000))dirty=urgent=true;
        if(now>=next_history) {
            environment_history_t h;environment_log_snapshot(&h);
            if(h.revision!=state.history.revision)dirty=true;
            state.history=h;next_history=now+1000000;
        }
        key_event_t event;
        display_command_t command;
        while(xQueueReceive(display_queue,&command,0)==pdTRUE) {
            if(command==DISPLAY_TEST){test_requested=true;dirty=urgent=true;}
            else if(command==DISPLAY_FULL){force_full=true;dirty=urgent=true;}
            else if(command==DISPLAY_PARTIAL_ON){if(test_complete){refresh.enabled=true;partial_setting(true,true);}printf("EPAPER_PARTIAL enabled=%d tested=%d\n",refresh.enabled,test_complete);}
            else if(command==DISPLAY_PARTIAL_OFF){refresh.enabled=false;partial_setting(true,false);force_full=true;dirty=urgent=true;}
            else printf("EPAPER_DISPLAY_STATUS partial=%d count=%u tested=%d ready=%d\n",refresh.enabled,refresh.partial_count,test_complete,refresh.ready);
        }
        while (xQueueReceive(input_queue, &event, 0) == pdTRUE) {
            if(event.key==1 && event.gesture==GESTURE_SHUTDOWN) {
                shutdown_result_t result=shutdown_run(&shutdown_ops,&canvas);
                printf("EPAPER_SHUTDOWN cancelled=%d\n",result);
                // Reaching here means the safe preparation or display step failed.
                dashboard_render_shutdown(&canvas,true);
                if(display_frame(frame))memcpy(previous,frame,SCREEN_BYTES);
                refresh.ready=false;first=false;retry_display=true;
                dirty=true;urgent=false;last_draw=esp_timer_get_time();
                continue;
            }
            if (state.page!=PAGE_MUSIC&&event.key == 0 && event.gesture == GESTURE_DOUBLE) {manual_refresh_waiting = true;force_full=true;}
            apply_key(event, &state, &canvas);
            dirty = true; urgent = true;
        }
        device_command_t remote;
        while(network_take_device_command(&remote)) {
            if(remote.operation==DEVICE_PAGE) {
                if(dashboard_page_available(&state,remote.value))state.page=(dashboard_page_t)remote.value;
                state.paused=false;
            } else if(remote.operation==DEVICE_ROTATE) {
                state.auto_rotate=remote.value==4;
                if(!state.auto_rotate)canvas.rotation=remote.value;
            } else if(remote.operation==DEVICE_REFRESH)force_full=true;
            else if(remote.operation==DEVICE_SAMPLE) {
                sample_refresh_begin(&sample,remote.value,esp_timer_get_time()/1000);
                xTaskNotify(hardware_input_handle,remote.value,eSetValueWithOverwrite);
            }
            if(remote.operation!=DEVICE_SAMPLE)dirty=urgent=true;
            printf("EPAPER_REMOTE op=%u value=%u\n",remote.operation,remote.value);
        }
        if (state.auto_rotate && state.imu_valid && !state.paused && canvas.rotation != telemetry.rotation) {
            canvas.rotation = telemetry.rotation;
            printf("EPAPER_ROTATE %u\n", canvas.rotation);
            dirty = true; urgent = true;
        }
        bool regular_due = first || (urgent && now - last_draw >= INT64_C(2000000)) ||
                   (!state.paused && ((dirty && now - last_draw >= INT64_C(30000000)) ||
                                     now - last_draw >= INT64_C(60000000))) ||
                   (retry_display && now - last_draw >= INT64_C(30000000));
        bool power_changed=!power_icon_equal(&displayed_power,&state.power);
        bool power_due=power_changed && !first && refresh.ready && refresh.enabled &&
            !retry_display && !force_full && !test_requested &&
            refresh.layout==dashboard_layout_key(&state,&canvas) && now-last_power_draw>=200000;
        bool power_only=power_due && !regular_due;
        bool due=regular_due || power_due;
        device_snapshot_t device={.sensor_valid=state.sensor_valid,.temperature=state.temperature,.humidity=state.humidity,
            .sampled_at=telemetry.sampled_at,.sampled_ms=telemetry.sampled_ms,.uptime_ms=(uint64_t)now/1000,
            .page=state.page,.rotation=canvas.rotation,.auto_rotate=state.auto_rotate,.paused=state.paused,
            .power=state.power,.busy=due||sample_refresh_pending(&sample),.partial=refresh.enabled,.frame=confirmed_frame,
            .sample_id=sample.id,.sample_result=sample.result};
        _Static_assert(PAGE_COUNT==13,"Device protocol page mapping needs updating");
        for(unsigned p=0;p<PAGE_COUNT;++p) {
            if(dashboard_page_available(&state,p))device.pages|=1U<<p;
        }
        network_publish_device(&device,&state.history);
        if (due) {
            if(power_only) {
                memcpy(frame,previous,SCREEN_BYTES);
                dashboard_render_power(&canvas,&state.power,state.paused);
            } else dashboard_render(&canvas, &state);
            uint32_t key=dashboard_layout_key(&state,&canvas);
            bool forced=first||retry_display||force_full||test_requested;
            refresh_kind_t kind=refresh_decide(&refresh,memcmp(frame,previous,SCREEN_BYTES)!=0,forced,key,now);
            const char *reason=kind==REFRESH_DIFFERENTIAL?"change":(forced?"requested":(!refresh.ready?"base":(!refresh.enabled?"disabled":(key!=refresh.layout?"layout":(refresh.partial_count>=20?"clean_count":"clean_time")))));
            if(kind!=REFRESH_SKIP) {
                int64_t began=esp_timer_get_time();
                bool success=kind==REFRESH_DIFFERENTIAL?EPD_Display_Differential(previous,frame):display_frame(frame);
                if(!success && kind==REFRESH_DIFFERENTIAL){kind=REFRESH_FULL;reason="recover_io";success=display_frame(frame);}
                refresh_commit(&refresh,kind,success,key,now);
                if (success) {
                    memcpy(previous, frame, SCREEN_BYTES);
                    if(power_only) {
                        confirmed_frame.at=time(NULL);++confirmed_frame.sequence;
                    } else confirmed_frame=(device_frame_t){.sensor_valid=state.sensor_valid,.temperature=state.temperature,.humidity=state.humidity,
                            .at=time(NULL),.page=state.page,.rotation=canvas.rotation,.auto_rotate=state.auto_rotate,.sequence=confirmed_frame.sequence+1};
                    displayed_power=state.power;last_power_draw=esp_timer_get_time();
                    if(power_changed)power_trace_add(&state.power,true,(uint64_t)last_power_draw/1000);
                    first = false; retry_display = false;
                    ESP_LOGI(TAG, "EPAPER_DISPLAY %s page=%u rotation=%u auto=%d paused=%d ms=%lld reason=%s",
                             kind==REFRESH_DIFFERENTIAL?"partial":"full",state.page, canvas.rotation, state.auto_rotate, state.paused,(long long)((esp_timer_get_time()-began)/1000),reason);
                    if(!power_only && state.page==PAGE_OVERVIEW){
                        time_t at=time(NULL);struct tm local;localtime_r(&at,&local);
                        int day=(local.tm_year+1900)*10000+(local.tm_mon+1)*100+local.tm_mday;int64_t totals[3];
                        usage_summary_totals(state.network.providers,PROVIDER_COUNT,&state.network.hub_totals,day,totals);
                        printf("EPAPER_TOTALS_FRAME frame=%u day=%d daily=%lld weekly=%lld monthly=%lld hub=%d\n",
                            confirmed_frame.sequence,day,(long long)totals[0],(long long)totals[1],(long long)totals[2],state.network.hub_totals.received);
                    }
                    if(power_only)printf("EPAPER_POWER_DISPLAY usb=%d percent=%d elapsed_ms=%llu\n",state.power.usb_present,state.power.percent,
                        (unsigned long long)(telemetry.power_event_ms?(uint64_t)last_power_draw/1000-telemetry.power_event_ms:0));
                } else {
                    first = false; retry_display = true;
                    ESP_LOGE(TAG, "Display command failed; retrying in 30 seconds");
                }
                if(!power_only)sample_refresh_drawn(&sample,true,success);
            }
            if(kind==REFRESH_SKIP && !power_only)sample_refresh_drawn(&sample,false,true);
            if(kind==REFRESH_SKIP && power_only) {
                displayed_power=state.power;last_power_draw=now;
            }
            if(test_requested && !retry_display){test_complete=partial_test(&canvas,previous);test_requested=false;refresh.ready=test_complete;refresh.partial_count=9;refresh.layout=key;refresh.last_full_us=esp_timer_get_time();if(!test_complete){refresh.enabled=false;partial_setting(true,false);retry_display=true;}}
            force_full=false;
            if(!power_only || retry_display) {
                dirty = false; urgent = false; last_draw = esp_timer_get_time();
            }
            if(kind==REFRESH_SKIP)confirmed_frame.auto_rotate=state.auto_rotate;
            device.frame=confirmed_frame;device.busy=sample_refresh_pending(&sample);
            device.sample_id=sample.id;device.sample_result=sample.result;
            network_publish_device(&device,&state.history);
        }
        if(sample.id && sample.result && (sample.id!=reported_sample || sample.result!=reported_result)) {
            printf("EPAPER_SAMPLE id=%lu result=%u frame=%u\n",(unsigned long)sample.id,sample.result,confirmed_frame.sequence);
            reported_sample=sample.id;reported_result=sample.result;
        }
        vTaskDelay(pdMS_TO_TICKS(DASHBOARD_POLL_MS));
    }
}
