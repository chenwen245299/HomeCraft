#include "network.h"
#include "ble_enrollment.h"
#include "ble_recovery.h"
#include "ble_transport_probe.h"
#include "energy_policy.h"
#include "cJSON.h"
#include "esp_crt_bundle.h"
#include "esp_event.h"
#include "esp_http_client.h"
#include "esp_http_server.h"
#include "esp_log.h"
#include "esp_mac.h"
#include "esp_netif.h"
#include "esp_netif_sntp.h"
#include "esp_random.h"
#include "esp_timer.h"
#include "esp_wifi.h"
#include "freertos/FreeRTOS.h"
#include "freertos/event_groups.h"
#include "freertos/semphr.h"
#include "freertos/task.h"
#include "freertos/queue.h"
#include "nvs.h"
#include "nvs_flash.h"
#include "network_provisioning/manager.h"
#include "network_provisioning/scheme_ble.h"
#include "protocomm_security.h"
#include "esp_srp.h"
#include "lwip/sockets.h"
#include <ctype.h>
#include <limits.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <sys/time.h>

#define ACCESS_SIZE 8192
#define REFRESH_SIZE 2048
#define RESPONSE_SIZE 32768
#define CONFIG_VERSION 1
static const char *TAG = "network";
typedef struct { char access[ACCESS_SIZE], refresh[REFRESH_SIZE], account[128], client[128]; } provider_auth_t;
typedef struct {
    uint32_t version;
    char ssid[33], password[65], ap_password[17], setup_code[9];
    unsigned interval_seconds;
    provider_auth_t auth[2];
    char deepseek_key[512];
} device_config_t;
static device_config_t *settings;
static network_snapshot_t state;
static network_ble_status_t ble_status_snapshot;
static bool ble_recovery_request;
static unsigned ble_recovery_count;
static int ble_recovery_error;
static SemaphoreHandle_t lock;
static EventGroupHandle_t shutdown_events;
static bool worker_started;
#define STOP_REQUEST BIT0
#define STOP_READY BIT1
#define STOP_FAILED BIT2
static nvs_handle_t storage;
static httpd_handle_t server;
static bool apply_pending, refresh_pending;
static int setup_request;
static int ble_request;
static bool ble_initialized, ble_end_pending, ble_retry_pending, ble_testing;
static uint64_t ble_until, ble_test_until;
static unsigned ble_auth_failures;
static wifi_sta_config_t ble_candidate;
static protocomm_security2_params_t ble_security;
static bool ble_is_data, data_paired, data_open_request;
static char data_key[33];
static ble_enrollment_t enrollment;
// Distinct services prevent an already-bound Mac from authenticating with a long-term
// secret against the temporary PIN verifier while another Mac is being enrolled.
static uint8_t data_service_uuid[16]={0x07,0xed,0x9b,0x2d,0x0f,0x06,0x7c,0x87,0x9b,0x43,0x43,0x6b,0x4d,0x24,0x75,0x17};
static uint8_t enrollment_service_uuid[16]={0x08,0xed,0x9b,0x2d,0x0f,0x06,0x7c,0x87,0x9b,0x43,0x43,0x6b,0x4d,0x24,0x75,0x17};
static uint64_t data_pairing_until, data_block_until;
static unsigned data_refresh_sequence;
static device_snapshot_t device_snapshot;
static environment_history_t device_history;
static QueueHandle_t device_commands;
static uint32_t sample_sequence, sample_inflight;
static int64_t setup_until;
static esp_netif_t *sta_netif;
static unsigned wifi_interfaces;
extern const char setup_html_start[] asm("_binary_settings_page_html_start");
extern const char setup_html_end[] asm("_binary_settings_page_html_end");

static bool harness_dirty;
static uint64_t harness_save_retry;
static uint64_t now_ms(void) { return esp_timer_get_time() / 1000; }
static void take(void) { xSemaphoreTake(lock, portMAX_DELAY); }
static void give(void) { xSemaphoreGive(lock); }
static bool has_auth(const device_config_t *cfg, unsigned p)
{
    if(p>=PROVIDER_ACCOUNT_COUNT) return false;
    return p < 2 ? cfg->auth[p].access[0] != 0 : cfg->deepseek_key[0] != 0;
}
static esp_err_t persist(const device_config_t *cfg)
{
    esp_err_t err = nvs_set_blob(storage, "config", cfg, sizeof(*cfg));
    return err == ESP_OK ? nvs_commit(storage) : err;
}

static int harness_calendar_day(void)
{
    time_t now=time(NULL);struct tm local;localtime_r(&now,&local);
    return (local.tm_year+1900)*10000+(local.tm_mon+1)*100+local.tm_mday;
}
static void save_harness(bool force)
{
    take();
    if(harness_dirty && (force || now_ms()>=harness_save_retry)) {
        esp_err_t err=nvs_set_blob(storage,"harness_stats",&state.harness,sizeof(state.harness));
        if(err==ESP_OK)err=nvs_commit(storage);
        if(err==ESP_OK){harness_dirty=false;harness_save_retry=now_ms()+HARNESS_SAVE_MS;}
        else {harness_save_retry=now_ms()+60000;ESP_LOGW(TAG,"Harness cache persistence failed: %s",esp_err_to_name(err));}
    }
    give();
}

void network_get_snapshot(network_snapshot_t *out)
{
    if (!lock) { memset(out, 0, sizeof(*out)); return; }
    take(); *out = state; give();
    if(out->harness.count)harness_stats_total(&out->harness,harness_calendar_day(),&out->providers[PROVIDER_EXTRA1]);
}
void network_get_ble_status(network_ble_status_t *out)
{
    if(!lock){memset(out,0,sizeof(*out));return;}
    take();*out=ble_status_snapshot;give();
}
bool network_request_ble_recovery(void)
{
    if(!lock)return false;
    take();bool allowed=ble_status_snapshot.initialized && ble_status_snapshot.data_mode &&
        !ble_status_snapshot.enrolling && !ble_status_snapshot.connections && data_paired;
    if(allowed)ble_recovery_request=true;
    give();return allowed;
}
void network_publish_device(const device_snapshot_t *snapshot, const environment_history_t *history)
{
    if(!lock)return;
    take();device_snapshot=*snapshot;device_history=*history;
    if(snapshot->sample_id==sample_inflight && snapshot->sample_result)sample_inflight=0;
    give();
}
bool network_take_device_command(device_command_t *command)
{
    return device_commands && xQueueReceive(device_commands,command,0)==pdTRUE;
}
void network_request_refresh(void)
{
    if (!lock) return;
    take(); refresh_pending = true; ++data_refresh_sequence; give();
}
void network_toggle_setup(void)
{
    if (!lock) return;
    take(); setup_request = state.setup_active ? -1 : 1; give();
}
void network_toggle_ble(void)
{
    if (!lock) return;
    take(); ble_request = state.ble_active && !state.data_mode ? -1 : 1; give();
}
void network_open_data_pairing(void)
{
    if (!lock) return;
    take(); data_open_request = true; data_block_until = 0;
    ++state.revision; give();
}

esp_err_t network_export_ble_pairing(char *out, size_t size)
{
    if (!lock) return ESP_ERR_INVALID_STATE;
    take();
    esp_err_t result = ESP_ERR_INVALID_STATE;
    if (state.ble_active && state.data_mode && !state.data_enrollment && state.data_pairing_open && now_ms() < data_pairing_until) {
        int n = snprintf(out,size,"{\"name\":\"%s\",\"username\":\"%s\",\"password\":\"%s\",\"service_uuid\":\"1775244d-6b43-439b-877c-060f2d9bed07\"}",
                         state.data_name,EPAPER_PROVISION_USERNAME,data_key);
        if (n > 0 && (size_t)n < size) {
            result = ESP_OK; state.data_pairing_open = false; ++state.revision;
        }
    }
    give(); return result;
}

// Protocomm calls this only after Security 2 has authenticated and decrypted the request.
// The permanent random key is never advertised, displayed, returned by HTTP or logged.
static esp_err_t enrollment_handler(uint32_t session, const uint8_t *input, ssize_t length,
                                    uint8_t **output, ssize_t *output_len, void *unused)
{
    (void)session; (void)unused;
    if (length!=3 || memcmp(input,"get",3)) return ESP_ERR_INVALID_ARG;
    char reply[224];
    take();
    if (!state.ble_active || !state.data_enrollment || !state.data_pairing_open ||
        !data_key[0] || enrollment.claimed || now_ms()>=enrollment.expires_at) {
        give(); return ESP_ERR_INVALID_STATE;
    }
    int n=snprintf(reply,sizeof(reply),"{\"name\":\"%s\",\"username\":\"%s\",\"password\":\"%s\",\"service_uuid\":\"1775244d-6b43-439b-877c-060f2d9bed07\"}",
                   state.data_name,EPAPER_PROVISION_USERNAME,data_key);
    if (n<=0 || (size_t)n>=sizeof(reply)) { give(); memset(reply,0,sizeof(reply)); return ESP_ERR_INVALID_SIZE; }
    uint8_t *result=(uint8_t *)malloc((size_t)n);
    if (!result) { give(); memset(reply,0,sizeof(reply)); return ESP_ERR_NO_MEM; }
    memcpy(result,reply,(size_t)n); memset(reply,0,sizeof(reply));
    if (!ble_enrollment_claim(&enrollment,now_ms())) { give(); memset(result,0,(size_t)n); free(result); return ESP_ERR_INVALID_STATE; }
    state.data_pairing_open=false; memset(state.pairing_code,0,sizeof(state.pairing_code));
    ++state.revision; give();
    *output=result; *output_len=n; return ESP_OK;
}

static esp_err_t data_handler(uint32_t session, const uint8_t *input, ssize_t length,
                             uint8_t **output, ssize_t *output_len, void *unused)
{
    (void)session; (void)unused;
    if (length <= 0 || length > 480 || memchr(input,0,length)) return ESP_ERR_INVALID_ARG;
    char text[481]; memcpy(text,input,length); text[length]=0;
    // Device telemetry/control shares the authenticated, encrypted data endpoint.
    // It cannot read configuration or trigger enrollment, account access or shutdown.
    device_command_t command;
    if (device_command_decode(text,&command)) {
        char reply[481];
        bool encoded=true;
        if(command.operation==DEVICE_HARNESS) {
            take();encoded=harness_stats_encode(&state.harness,command.value,reply);give();
        } else if(command.operation==DEVICE_STATUS || command.operation==DEVICE_HISTORY) {
            device_snapshot_t snapshot; environment_history_t history;
            take();snapshot=device_snapshot;history=device_history;give();
            encoded=command.operation==DEVICE_STATUS?device_status_encode(&snapshot,&history,reply):device_history_encode(&history,command.value,reply);
        } else {
            take();
            bool allowed=command.operation!=DEVICE_PAGE || (device_snapshot.pages & (1U<<command.value));
            if(command.operation==DEVICE_SAMPLE) {
                allowed=sample_inflight==0;
                if(++sample_sequence==0)++sample_sequence;
                command.value=sample_sequence;
            }
            bool queued=allowed && device_commands && xQueueSend(device_commands,&command,0)==pdTRUE;
            if(queued && command.operation==DEVICE_SAMPLE)sample_inflight=command.value;
            give();
            if(command.operation==DEVICE_SAMPLE)
                snprintf(reply,sizeof(reply),"{\"ok\":%s,\"queued\":%s,\"sample\":%lu}",queued?"true":"false",queued?"true":"false",(unsigned long)command.value);
            else snprintf(reply,sizeof(reply),"{\"ok\":%s,\"queued\":%s}",queued?"true":"false",queued?"true":"false");
        }
        if(!encoded)return ESP_ERR_INVALID_SIZE;
        *output=(uint8_t *)strdup(reply);if(!*output)return ESP_ERR_NO_MEM;
        *output_len=strlen(reply);return ESP_OK;
    }
    bool ok = length == 1 && text[0] == '?';
    usage_totals_t totals;
    bool is_totals=usage_totals_decode(text,&totals);
    if(is_totals){
        take();state.hub_totals=totals;++state.revision;give();
        ESP_LOGI(TAG,"EPAPER_TOTALS received day=%d daily=%lld weekly=%lld monthly=%lld",
            totals.periods[0],(long long)totals.tokens[0],(long long)totals.tokens[1],(long long)totals.tokens[2]);
        ok=true;
    }
    usage_breakdown_t detail;
    bool is_detail=usage_breakdown_decode(text,&detail);
    if(is_detail) {
        take();
        ok=usage_breakdown_matches(&detail,&state.providers[PROVIDER_ARGUS]);
        if(ok){state.argus_breakdown=detail;++state.revision;}
        give();
        if(!ok)return ESP_ERR_INVALID_ARG;
    }
    provider_id_t provider; usage_record_t record;
    if (!is_detail && !ok) {
        if (!usage_packet_decode(text,&provider,&record)) return ESP_ERR_INVALID_ARG;
        take();
        usage_record_t *current = &state.providers[provider];
        bool harness=provider==PROVIDER_EXTRA1 && !strcmp(record.name,"DeepSeek Harness");
        if(harness && record.status==USAGE_OK) {
            bool changed;
            if(!harness_stats_update(&state.harness,&record,&changed)){give();return ESP_ERR_INVALID_ARG;}
            harness_dirty|=changed;
            harness_stats_total(&state.harness,record.token_day,current);
            state.ble_received_at=record.fetched_at;state.ble_link=true;state.data_pairing_open=false;++state.revision;
        } else if (!harness && (!current->has_data || record.fetched_at >= current->fetched_at)) {
            if (record.status == USAGE_OK || record.status == USAGE_NOT_CONFIGURED) {
                *current = record;
                if(provider==PROVIDER_ARGUS && !usage_breakdown_matches(&state.argus_breakdown,current))
                    memset(&state.argus_breakdown,0,sizeof(state.argus_breakdown));
            }
            else { current->status=record.status; current->configured=record.configured; }
            if (record.fetched_at>state.ble_received_at) state.ble_received_at=record.fetched_at;
            state.ble_link=true;
            state.data_pairing_open=false; ++state.revision;
        }
        if (!data_paired && nvs_set_u8(storage,"ble_paired",1)==ESP_OK && nvs_commit(storage)==ESP_OK) data_paired=true;
        give();
        int64_t current_time=time(NULL), target=usage_clock_target(current_time,record.fetched_at);
        if(target!=current_time) {
            struct timeval tv={.tv_sec=target};
            if(settimeofday(&tv,NULL)==0)ESP_LOGI(TAG,"EPAPER_CLOCK advanced_seconds=%lld",(long long)(target-current_time));
        }
        take();
        bool clock_ready=time(NULL)>=1735689600 && time(NULL)<=4102444800;
        if(state.clock_ready!=clock_ready){state.clock_ready=clock_ready;++state.revision;}
        give();
        ESP_LOGI(TAG,"EPAPER_BLE_DATA provider=%s status=%u",usage_provider_name(provider),record.status);
        ok=true;
    }
    take(); unsigned sequence=data_refresh_sequence; give();
    char reply[304]; int n=snprintf(reply,sizeof(reply),"{\"ok\":%s,\"refresh\":%u,\"daily\":true,\"profiles\":true,\"weekly_tokens\":true,\"monthly_tokens\":true,\"yesterday_usage\":true,\"max_providers\":8,\"device_control\":1,\"argus_breakdown\":1,\"sample_refresh\":1,\"harness_devices\":1,\"totals\":1}",ok?"true":"false",sequence);
    if(n<0 || (size_t)n>=sizeof(reply))return ESP_ERR_INVALID_SIZE;
    *output=(uint8_t *)strdup(reply); if (!*output) return ESP_ERR_NO_MEM;
    *output_len=n; return ESP_OK;
}

static void data_transport_event(void *unused, esp_event_base_t base, int32_t event, void *data)
{
    (void)unused; (void)base; (void)data;
    if (event==PROTOCOMM_TRANSPORT_BLE_DISCONNECTED) {
        take(); if (ble_is_data) { state.ble_link=false; ++state.revision; } give();
    }
}

static void wifi_event(void *arg, esp_event_base_t base, int32_t id, void *data)
{
    (void)arg;
    if(base==WIFI_EVENT && (id==WIFI_EVENT_STA_START || id==WIFI_EVENT_STA_STOP || id==WIFI_EVENT_AP_START || id==WIFI_EVENT_AP_STOP)) {
        take();
        if(id==WIFI_EVENT_STA_START)wifi_interfaces|=1U;
        if(id==WIFI_EVENT_AP_START)wifi_interfaces|=2U;
        if(id==WIFI_EVENT_STA_STOP){wifi_interfaces&=~1U;state.connected=false;state.ip[0]=0;}
        if(id==WIFI_EVENT_AP_STOP)wifi_interfaces&=~2U;
        state.wifi_running=wifi_interfaces!=0;++state.revision;give();
    }
    if (base == IP_EVENT && id == IP_EVENT_STA_GOT_IP) {
        ip_event_got_ip_t *event = data;
        take(); state.connected = true; state.wifi_disconnect_reason = 0; ++state.revision;
        snprintf(state.ip, sizeof(state.ip), IPSTR, IP2STR(&event->ip_info.ip)); give();
        esp_netif_sntp_start();
        ESP_LOGI(TAG, "EPAPER_WIFI connected ip=" IPSTR, IP2STR(&event->ip_info.ip));
    } else if (base == WIFI_EVENT && id == WIFI_EVENT_STA_DISCONNECTED) {
        wifi_event_sta_disconnected_t *event = data;
        take(); state.connected = false; state.ip[0] = 0;
        state.wifi_disconnect_reason = event->reason; ++state.revision; give();
        ESP_LOGI(TAG, "EPAPER_WIFI disconnected reason=%u", event->reason);
    }
}

static void ble_event(void *unused, network_prov_cb_event_t event, void *data)
{
    (void)unused;
    if (event == NETWORK_PROV_SET_WIFI_STA_CONFIG) {
        // Opening BLE preserves STA. Only a submitted replacement disconnects it.
        take(); bool disconnect = state.connected && !ble_testing; give();
        if (disconnect) esp_wifi_disconnect();
    } else if (event == NETWORK_PROV_WIFI_CRED_RECV) {
        take(); ble_candidate = *(wifi_sta_config_t *)data;
        ble_testing = true; ble_test_until = now_ms() + 45000;
        state.ble_status = 1; ++state.revision; give();
        ESP_LOGI(TAG, "EPAPER_BLE credentials received securely; testing connection");
    } else if (event == NETWORK_PROV_WIFI_CRED_SUCCESS) {
        take();
        if (ble_testing) {
            // Commit only after GOT_IP; retain old credentials on failure/power loss.
            device_config_t *next = malloc(sizeof(*next));
            esp_err_t result = ESP_ERR_NO_MEM;
            if (next) {
                *next = *settings;
                memcpy(next->ssid, ble_candidate.ssid, sizeof(ble_candidate.ssid)); next->ssid[32] = 0;
                memcpy(next->password, ble_candidate.password, sizeof(ble_candidate.password)); next->password[64] = 0;
                result = persist(next);
                if (result == ESP_OK) {
                    *settings = *next; state.wifi_configured = true;
                    snprintf(state.ssid, sizeof(state.ssid), "%s", settings->ssid);
                    refresh_pending = true;
                }
                memset(next, 0, sizeof(*next)); free(next);
            }
            state.ble_status = result == ESP_OK ? 2 : 4;
            ble_testing = false; ++state.revision;
            ESP_LOGI(TAG, "EPAPER_BLE connected; credential commit=%d", result);
        }
        give();
    } else if (event == NETWORK_PROV_WIFI_CRED_FAIL) {
        take(); state.ble_status = 3; ble_retry_pending = true;
        ble_testing = false; ++state.revision; give();
        ESP_LOGW(TAG, "EPAPER_BLE Wi-Fi attempt failed; saved network retained");
    } else if (event == NETWORK_PROV_END) {
        take(); ble_end_pending = true; give();
    }
}

static void ble_security_event(void *unused, esp_event_base_t base, int32_t event, void *data)
{
    (void)unused; (void)base; (void)data;
    if (event == PROTOCOMM_SECURITY_SESSION_INVALID_SECURITY_PARAMS ||
        event == PROTOCOMM_SECURITY_SESSION_CREDENTIALS_MISMATCH) {
        take(); if (++ble_auth_failures >= 5) {
            if(state.data_enrollment) { enrollment.expires_at=now_ms(); state.data_pairing_open=false; }
            else { ble_request = -1; data_block_until=now_ms()+600000; }
        } give();
    }
}

static esp_err_t close_ble(void)
{
    if (ble_initialized) {
        esp_err_t err=network_prov_mgr_deinit();
        if(err!=ESP_OK)return err;
        ble_initialized = false;
    }
    free((void *)ble_security.salt); free((void *)ble_security.verifier);
    memset(&ble_security, 0, sizeof(ble_security));
    take(); state.ble_active = false; memset(state.ble_pop, 0, sizeof(state.ble_pop));
    state.data_enrollment=state.data_pairing_open=false;
    memset(state.pairing_code,0,sizeof(state.pairing_code)); memset(&enrollment,0,sizeof(enrollment));
    state.ble_link = false;
    ble_testing = ble_end_pending = ble_retry_pending = false;
    memset(&ble_candidate, 0, sizeof(ble_candidate)); ++state.revision; give();
    esp_wifi_set_storage(WIFI_STORAGE_RAM);
    ESP_LOGI(TAG, "EPAPER_BLE closed");
    return ESP_OK;
}

static esp_err_t open_ble(bool data_mode, bool enrolling)
{
    network_prov_scheme_ble_set_service_uuid(enrolling?enrollment_service_uuid:data_service_uuid);
    network_prov_mgr_config_t cfg = {.scheme = network_prov_scheme_ble,
        .scheme_event_handler = NETWORK_PROV_EVENT_HANDLER_NONE,
        .app_event_handler = {.event_cb = ble_event},
        .network_prov_wifi_conn_cfg = {.wifi_conn_attempts = 3}};
    esp_err_t err = network_prov_mgr_init(cfg);
    if (err != ESP_OK) return err;
    ble_initialized = true;
    char pop[33]={0};
    if (data_mode) {
        if (!data_key[0]) {
            for (unsigned i=0;i<4;++i) snprintf(data_key+i*8,9,"%08lx",(unsigned long)esp_random());
            err=nvs_set_str(storage,"ble_key",data_key);
            if (err==ESP_OK) err=nvs_commit(storage);
            if (err!=ESP_OK) { close_ble(); return err; }
        }
        memcpy(pop,data_key,sizeof(pop));
        if (enrolling) {
            // Rejection sampling gives every 8-digit code the same probability.
            uint32_t random;
            do { random=esp_random(); } while(random>=4200000000U);
            snprintf(pop,sizeof(pop),"%08lu",(unsigned long)(random%100000000U));
        }
        network_prov_mgr_disable_auto_stop(1000);
        err=network_prov_mgr_endpoint_create(enrolling?"pair-device":"usage-data");
        if (err!=ESP_OK) { close_ble(); return err; }
    } else for (unsigned i = 0; i < 4; ++i) snprintf(pop + i*8, 9, "%08lx", (unsigned long)esp_random());
    char *salt = NULL, *verifier = NULL; int verifier_len = 0;
    err = esp_srp_gen_salt_verifier(EPAPER_PROVISION_USERNAME, strlen(EPAPER_PROVISION_USERNAME), pop, strlen(pop), &salt, 16, &verifier, &verifier_len);
    if (err == ESP_OK) {
        ble_security = (protocomm_security2_params_t){.salt = salt, .salt_len = 16,
            .verifier = verifier, .verifier_len = verifier_len};
        char pair_name[20]; snprintf(pair_name,sizeof(pair_name),"EPair_%.6s",state.data_name+7);
        err = network_prov_mgr_start_provisioning(NETWORK_PROV_SECURITY_2, &ble_security, enrolling?pair_name:(data_mode?state.data_name:state.ble_name), NULL);
        if (err==ESP_OK && data_mode) err=network_prov_mgr_endpoint_register(enrolling?"pair-device":"usage-data",enrolling?enrollment_handler:data_handler,NULL);
    }
    if (err == ESP_OK) {
        if(data_mode) {
            // The official provisioning manager starts STA for phone Wi-Fi scans.
            // Data endpoints do not need it; retain its RAM config but stop the radio.
            esp_err_t stopped=esp_wifi_stop();
            if(stopped!=ESP_OK && stopped!=ESP_ERR_WIFI_NOT_STARTED) {
                close_ble();memset(pop,0,sizeof(pop));return stopped;
            }
            take();state.connected=false;state.ip[0]=0;give();
        }
        take(); memset(state.ble_pop,0,sizeof(state.ble_pop));
        if(!data_mode)memcpy(state.ble_pop,pop,sizeof(pop));
        state.ble_active = true;
        state.ble_status = 0; ble_auth_failures = 0; ble_until = now_ms() + 600000;
        ble_is_data=data_mode; state.data_mode=data_mode;
        state.data_enrollment=enrolling;
        if (enrolling) {
            memcpy(state.pairing_code,pop,9); state.data_pairing_open=true;
            ble_enrollment_begin(&enrollment,now_ms()); data_pairing_until=enrollment.expires_at;
        } else if (data_mode && !data_paired) { state.data_pairing_open=true; data_pairing_until=ble_until; }
        ++state.revision; give();
        ESP_LOGI(TAG, "EPAPER_BLE started mode=%s name=%s security=2", data_mode?"data":"wifi",data_mode?state.data_name:state.ble_name);
    } else {
        if (!ble_security.salt) { free(salt); free(verifier); }
        close_ble();
    }
    memset(pop, 0, sizeof(pop)); return err;
}

static bool on_ap_interface(httpd_req_t *req)
{
    struct sockaddr_in local = {0}; socklen_t length = sizeof(local);
    esp_netif_ip_info_t ap_ip;
    esp_netif_t *ap = esp_netif_get_handle_from_ifkey("WIFI_AP_DEF");
    return ap && esp_netif_get_ip_info(ap, &ap_ip) == ESP_OK &&
        getsockname(httpd_req_to_sockfd(req), (struct sockaddr *)&local, &length) == 0 &&
        local.sin_family == AF_INET && local.sin_addr.s_addr == ap_ip.ip.addr;
}

static esp_err_t send_json(httpd_req_t *req, cJSON *json)
{
    char *text = cJSON_PrintUnformatted(json);
    cJSON_Delete(json);
    if (!text) return httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR, "No memory");
    httpd_resp_set_type(req, "application/json; charset=utf-8");
    httpd_resp_set_hdr(req, "Cache-Control", "no-store");
    httpd_resp_set_hdr(req, "X-Content-Type-Options", "nosniff");
    esp_err_t err = httpd_resp_sendstr(req, text);
    free(text); return err;
}

static bool authorized(httpd_req_t *req)
{
    if (!on_ap_interface(req)) {
        httpd_resp_send_err(req, HTTPD_403_FORBIDDEN, "Configuration is available only on the device hotspot"); return false;
    }
    char supplied[16] = {0}, expected[9];
    take(); bool active = state.setup_active; memcpy(expected, settings->setup_code, sizeof(expected)); give();
    if (!active || httpd_req_get_hdr_value_str(req, "X-Setup-Code", supplied, sizeof(supplied)) != ESP_OK) {
        httpd_resp_send_err(req, HTTPD_401_UNAUTHORIZED, "Enter the setup code shown on the device"); return false;
    }
    unsigned diff = strlen(supplied) ^ 8;
    for (unsigned i = 0; i < 8; ++i) diff |= (unsigned char)supplied[i] ^ (unsigned char)expected[i];
    if (diff) { httpd_resp_send_err(req, HTTPD_401_UNAUTHORIZED, "Invalid setup code"); return false; }
    return true;
}

static esp_err_t index_handler(httpd_req_t *req)
{
    if (!on_ap_interface(req)) return httpd_resp_send_err(req, HTTPD_403_FORBIDDEN, "Use the device hotspot");
    httpd_resp_set_type(req, "text/html; charset=utf-8");
    httpd_resp_set_hdr(req, "Cache-Control", "no-store");
    httpd_resp_set_hdr(req, "Content-Security-Policy", "default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; frame-ancestors 'none'");
    return httpd_resp_send(req, setup_html_start, setup_html_end - setup_html_start - 1);
}

static void add_window(cJSON *object, const char *name, const quota_window_t *w)
{
    cJSON *window = cJSON_AddObjectToObject(object, name);
    if (w->remaining_tenths < 0) cJSON_AddNullToObject(window, "remaining_percent");
    else cJSON_AddNumberToObject(window, "remaining_percent", w->remaining_tenths / 10.0);
    cJSON_AddNumberToObject(window, "window_minutes", w->window_mins);
    cJSON_AddNumberToObject(window, "reset_at", (double)w->reset_at);
}

static esp_err_t status_handler(httpd_req_t *req)
{
    if (!authorized(req)) return ESP_OK;
    network_snapshot_t snapshot;
    network_get_snapshot(&snapshot);
    cJSON *json = cJSON_CreateObject();
    cJSON_AddBoolToObject(json, "connected", snapshot.connected);
    cJSON_AddBoolToObject(json, "clock_ready", snapshot.clock_ready);
    cJSON_AddStringToObject(json, "ssid", snapshot.ssid);
    cJSON_AddStringToObject(json, "ip", snapshot.ip);
    cJSON_AddNumberToObject(json, "interval_seconds", snapshot.interval_seconds);
    cJSON *providers = cJSON_AddArrayToObject(json, "providers");
    for (unsigned i = 0; i < PROVIDER_COUNT; ++i) {
        const usage_record_t *r = &snapshot.providers[i];
        cJSON *p = cJSON_CreateObject(); cJSON_AddItemToArray(providers, p);
        cJSON_AddStringToObject(p, "name", usage_provider_name(i));
        cJSON_AddStringToObject(p, "status", usage_status_text(r->status));
        cJSON_AddBoolToObject(p, "configured", r->configured);
        cJSON_AddBoolToObject(p, "has_data", r->has_data);
        cJSON_AddNumberToObject(p, "http_status", r->http_status);
        cJSON_AddNumberToObject(p, "fetched_at", (double)r->fetched_at);
        add_window(p, "short", &r->windows[0]); add_window(p, "week", &r->windows[1]); add_window(p, "month", &r->windows[2]);
        if (r->balance_available) {
            cJSON_AddNumberToObject(p, "balance", r->balance);
            cJSON_AddStringToObject(p, "currency", r->currency);
        }
        if (r->monthly_spend_available) {
            cJSON_AddNumberToObject(p, "extra_monthly_used", r->monthly_used);
            cJSON_AddNumberToObject(p, "extra_monthly_limit", r->monthly_limit);
        }
    }
    // Never return saved passwords, tokens, account IDs or refresh tokens.
    return send_json(req, json);
}

static bool read_string(const cJSON *parent, const char *key, char *dest, size_t size, bool keep_empty)
{
    const cJSON *value = cJSON_GetObjectItemCaseSensitive(parent, key);
    if (!value) return true;
    if (!cJSON_IsString(value) || strlen(value->valuestring) >= size) return false;
    for (const unsigned char *p = (const unsigned char *)value->valuestring; *p; ++p)
        if (*p < 32 || *p == 127) return false;
    if (!keep_empty || value->valuestring[0]) strcpy(dest, value->valuestring);
    return true;
}

esp_err_t network_apply_config_json(const char *body)
{
    if (!lock || !body || strlen(body) > RESPONSE_SIZE) return ESP_ERR_INVALID_ARG;
    take(); bool active = state.setup_active; give();
    if (!active) return ESP_ERR_INVALID_STATE;
    device_config_t *next = malloc(sizeof(*next));
    if (!next) return ESP_ERR_NO_MEM;
    cJSON *json = cJSON_ParseWithOpts(body, NULL, true);
    take(); *next = *settings;
    bool valid = cJSON_IsObject(json);
    const cJSON *wifi = cJSON_GetObjectItemCaseSensitive(json, "wifi");
    if (wifi) {
        char previous[33]; strcpy(previous, next->ssid);
        valid = valid && cJSON_IsObject(wifi) && read_string(wifi, "ssid", next->ssid, sizeof(next->ssid), false);
        if (strcmp(previous, next->ssid)) next->password[0] = 0;
        valid = valid && read_string(wifi, "password", next->password, sizeof(next->password), true);
        if (cJSON_IsTrue(cJSON_GetObjectItemCaseSensitive(wifi, "open_network"))) next->password[0] = 0;
        if (next->password[0] && strlen(next->password) < 8) valid = false;
        if (strlen(next->password) == 64)
            for (unsigned i = 0; i < 64; ++i) if (!isxdigit((unsigned char)next->password[i])) valid = false;
    }
    const cJSON *period = cJSON_GetObjectItemCaseSensitive(json, "interval_seconds");
    if (period) {
        if (!cJSON_IsNumber(period) || period->valuedouble < 60 || period->valuedouble > 3600 || floor(period->valuedouble) != period->valuedouble) valid = false;
        else next->interval_seconds = period->valueint;
    }
    const char *names[] = {"codex", "claude"};
    for (unsigned i = 0; i < 2; ++i) {
        const cJSON *p = cJSON_GetObjectItemCaseSensitive(json, names[i]);
        if (!p) continue;
        if (cJSON_IsTrue(cJSON_GetObjectItemCaseSensitive(p, "clear"))) memset(&next->auth[i], 0, sizeof(next->auth[i]));
        valid = valid && cJSON_IsObject(p) &&
            read_string(p, "access_token", next->auth[i].access, sizeof(next->auth[i].access), true) &&
            read_string(p, "refresh_token", next->auth[i].refresh, sizeof(next->auth[i].refresh), true) &&
            read_string(p, "account_id", next->auth[i].account, sizeof(next->auth[i].account), true) &&
            read_string(p, "client_id", next->auth[i].client, sizeof(next->auth[i].client), true);
    }
    const cJSON *deepseek = cJSON_GetObjectItemCaseSensitive(json, "deepseek");
    if (deepseek) {
        if (cJSON_IsTrue(cJSON_GetObjectItemCaseSensitive(deepseek, "clear"))) next->deepseek_key[0] = 0;
        valid = valid && cJSON_IsObject(deepseek) && read_string(deepseek, "api_key", next->deepseek_key, sizeof(next->deepseek_key), true);
    }
    esp_err_t result = ESP_ERR_INVALID_ARG;
    if (valid) {
        result = persist(next);
        if (result == ESP_OK) {
            for (unsigned i = 0; i < PROVIDER_ACCOUNT_COUNT; ++i) {
                bool changed = i < 2 ? strcmp(settings->auth[i].access, next->auth[i].access) ||
                    strcmp(settings->auth[i].account, next->auth[i].account) : strcmp(settings->deepseek_key, next->deepseek_key);
                if (changed) usage_record_init(&state.providers[i]);
            }
            *settings = *next; apply_pending = true; refresh_pending = true;
        }
    }
    give();
    cJSON_Delete(json); memset(next, 0, sizeof(*next)); free(next);
    return result;
}

static esp_err_t config_handler(httpd_req_t *req)
{
    if (!authorized(req)) return ESP_OK;
    if (req->content_len <= 0 || req->content_len > RESPONSE_SIZE)
        return httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST, "Configuration too large");
    char *body = calloc(1, req->content_len + 1);
    if (!body) return ESP_ERR_NO_MEM;
    int used = 0;
    while (used < req->content_len) {
        int n = httpd_req_recv(req, body + used, req->content_len - used);
        if (n <= 0) { free(body); return ESP_FAIL; }
        used += n;
    }
    esp_err_t result = network_apply_config_json(body);
    memset(body, 0, req->content_len); free(body);
    if (result != ESP_OK) return httpd_resp_send_err(req, HTTPD_400_BAD_REQUEST, "Invalid configuration or storage error");
    return send_json(req, cJSON_Parse("{\"saved\":true,\"message\":\"配置已保存，正在连接并读取用量\"}"));
}

static esp_err_t scan_handler(httpd_req_t *req)
{
    if (!authorized(req)) return ESP_OK;
    wifi_scan_config_t cfg = {.show_hidden = false};
    esp_err_t err = esp_wifi_scan_start(&cfg, true);
    if (err != ESP_OK) {
        httpd_resp_set_status(req, "503 Service Unavailable");
        return httpd_resp_sendstr(req, "Wi-Fi is connecting; enter the SSID manually or retry");
    }
    uint16_t count = 20;
    wifi_ap_record_t *aps = calloc(count, sizeof(*aps));
    if (!aps) return ESP_ERR_NO_MEM;
    err = esp_wifi_scan_get_ap_records(&count, aps);
    cJSON *json = cJSON_CreateArray();
    if (err == ESP_OK) for (unsigned i = 0; i < count; ++i) {
        cJSON *p = cJSON_CreateObject(); cJSON_AddItemToArray(json, p);
        cJSON_AddStringToObject(p, "ssid", (const char *)aps[i].ssid);
        cJSON_AddNumberToObject(p, "rssi", aps[i].rssi);
        cJSON_AddBoolToObject(p, "open", aps[i].authmode == WIFI_AUTH_OPEN);
    }
    free(aps); return send_json(req, json);
}
static esp_err_t refresh_handler(httpd_req_t *req)
{
    if (!authorized(req)) return ESP_OK;
    network_request_refresh(); return send_json(req, cJSON_Parse("{\"queued\":true}"));
}
static esp_err_t close_handler(httpd_req_t *req)
{
    if (!authorized(req)) return ESP_OK;
    take(); setup_request = -1; give();
    return send_json(req, cJSON_Parse("{\"closing\":true}"));
}

static void open_setup(bool enabled)
{
    if (enabled) {
        wifi_config_t ap = {.ap = {.channel = 1, .max_connection = 2, .authmode = WIFI_AUTH_WPA2_PSK}};
        take();
        snprintf(settings->ap_password, sizeof(settings->ap_password), "%08lx%08lx", (unsigned long)esp_random(), (unsigned long)esp_random());
        snprintf(settings->setup_code, sizeof(settings->setup_code), "%08lx", (unsigned long)esp_random());
        memcpy(state.ap_password, settings->ap_password, sizeof(state.ap_password));
        memcpy(state.setup_code, settings->setup_code, sizeof(state.setup_code));
        snprintf((char *)ap.ap.ssid, sizeof(ap.ap.ssid), "%s", state.ap_ssid);
        memcpy(ap.ap.password, settings->ap_password, strlen(settings->ap_password));
        give();
        ap.ap.ssid_len = strlen((char *)ap.ap.ssid);
        ESP_ERROR_CHECK_WITHOUT_ABORT(esp_wifi_set_mode(WIFI_MODE_APSTA));
        ESP_ERROR_CHECK_WITHOUT_ABORT(esp_wifi_set_config(WIFI_IF_AP, &ap));
        ESP_ERROR_CHECK_WITHOUT_ABORT(esp_wifi_start());
        take(); state.setup_active = true; setup_until = now_ms() + 600000; ++state.revision; give();
        if (!server) {
            httpd_config_t cfg = HTTPD_DEFAULT_CONFIG();
            cfg.stack_size = 8192; cfg.max_open_sockets = 4; cfg.lru_purge_enable = true;
            if (httpd_start(&server, &cfg) == ESP_OK) {
                const httpd_uri_t routes[] = {
                    {.uri="/", .method=HTTP_GET, .handler=index_handler},
                    {.uri="/api/status", .method=HTTP_GET, .handler=status_handler},
                    {.uri="/api/scan", .method=HTTP_GET, .handler=scan_handler},
                    {.uri="/api/config", .method=HTTP_POST, .handler=config_handler},
                    {.uri="/api/refresh", .method=HTTP_POST, .handler=refresh_handler},
                    {.uri="/api/close", .method=HTTP_POST, .handler=close_handler},
                };
                for (unsigned i = 0; i < sizeof(routes)/sizeof(routes[0]); ++i) httpd_register_uri_handler(server, &routes[i]);
            }
        }
        ESP_LOGI(TAG, "EPAPER_SETUP opened; password and setup code are shown only on the screen");
    } else {
        if (server) { httpd_stop(server); server = NULL; }
        ESP_ERROR_CHECK_WITHOUT_ABORT(esp_wifi_set_mode(WIFI_MODE_STA));
        take(); state.setup_active = false; ++state.revision; give();
        esp_wifi_stop();
        ESP_LOGI(TAG, "EPAPER_SETUP closed");
    }
}

typedef struct { char *body; size_t used; bool overflow; int retry_after; } http_response_t;
static esp_err_t http_event(esp_http_client_event_t *event)
{
    http_response_t *r = event->user_data;
    if (event->event_id == HTTP_EVENT_ON_DATA) {
        if (event->data_len < 0 || r->used + event->data_len > RESPONSE_SIZE) { r->overflow = true; return ESP_FAIL; }
        memcpy(r->body + r->used, event->data, event->data_len); r->used += event->data_len; r->body[r->used] = 0;
    } else if (event->event_id == HTTP_EVENT_ON_HEADER && !strcasecmp(event->header_key, "Retry-After")) {
        char *end; long seconds = strtol(event->header_value, &end, 10);
        if (!*end && seconds >= 0 && seconds <= 86400) r->retry_after = (int)seconds;
    }
    return ESP_OK;
}

static int https_request(const char *url, const char *token, const char *account,
                         const char *post, bool claude, http_response_t *response)
{
    response->body = calloc(1, RESPONSE_SIZE + 1);
    if (!response->body) return -1;
    esp_http_client_config_t cfg = {.url = url, .event_handler = http_event, .user_data = response,
        .crt_bundle_attach = esp_crt_bundle_attach, .timeout_ms = 12000,
        .buffer_size = 2048, .buffer_size_tx = 12288, .disable_auto_redirect = true,
        .user_agent = "EPaperUsageMonitor/0.3"};
    esp_http_client_handle_t client = esp_http_client_init(&cfg);
    if (!client) return -1;
    if (token && token[0]) {
        char *authorization = malloc(strlen(token) + 8);
        if (!authorization) { esp_http_client_cleanup(client); return -1; }
        sprintf(authorization, "Bearer %s", token);
        esp_http_client_set_header(client, "Authorization", authorization); free(authorization);
    }
    esp_http_client_set_header(client, "Accept", "application/json");
    if (account && account[0]) esp_http_client_set_header(client, "ChatGPT-Account-ID", account);
    if (claude) esp_http_client_set_header(client, "anthropic-beta", "oauth-2025-04-20");
    if (post) {
        esp_http_client_set_method(client, HTTP_METHOD_POST);
        esp_http_client_set_header(client, "Content-Type", "application/json");
        esp_http_client_set_post_field(client, post, strlen(post));
    }
    esp_err_t err = esp_http_client_perform(client);
    int status = err == ESP_OK && !response->overflow ? esp_http_client_get_status_code(client) : -1;
    esp_http_client_cleanup(client); return status;
}

static bool refresh_auth(unsigned provider, provider_auth_t *auth)
{
    if (!auth->refresh[0] || !auth->client[0]) return false;
    const char *url = provider == PROVIDER_CODEX ? "https://auth.openai.com/oauth/token" : "https://console.anthropic.com/v1/oauth/token";
    cJSON *json = cJSON_CreateObject();
    cJSON_AddStringToObject(json, "grant_type", "refresh_token");
    cJSON_AddStringToObject(json, "refresh_token", auth->refresh);
    cJSON_AddStringToObject(json, "client_id", auth->client);
    char *body = cJSON_PrintUnformatted(json); cJSON_Delete(json);
    if (!body) return false;
    http_response_t response = {0};
    int status = https_request(url, NULL, NULL, body, false, &response);
    memset(body, 0, strlen(body)); free(body);
    bool ok = false;
    if (status == 200) {
        cJSON *tokens = cJSON_Parse(response.body);
        const cJSON *access = cJSON_GetObjectItemCaseSensitive(tokens, "access_token");
        const cJSON *refresh = cJSON_GetObjectItemCaseSensitive(tokens, "refresh_token");
        if (cJSON_IsString(access) && access->valuestring[0] && strlen(access->valuestring) < sizeof(auth->access) &&
            (!refresh || (cJSON_IsString(refresh) && strlen(refresh->valuestring) < sizeof(auth->refresh)))) {
            take();
            // Do not overwrite credentials if the user replaced them during this request.
            if (!strcmp(settings->auth[provider].refresh, auth->refresh)) {
                strcpy(auth->access, access->valuestring);
                if (refresh) strcpy(auth->refresh, refresh->valuestring);
                settings->auth[provider] = *auth;
                // Keep a rotated token in memory even if flash storage temporarily fails.
                ok = true;
                if (persist(settings) != ESP_OK) ESP_LOGE(TAG, "Credential renewal could not be persisted");
            }
            give();
        }
        cJSON_Delete(tokens);
    }
    if (response.body) { memset(response.body, 0, response.used); free(response.body); }
    return ok;
}

static unsigned fetch_usage(unsigned provider)
{
    if(provider>=PROVIDER_ACCOUNT_COUNT) return 0;
    provider_auth_t *auth = calloc(1, sizeof(*auth));
    if (!auth) return 300;
    take();
    if (provider < 2) *auth = settings->auth[provider];
    else strcpy(auth->access, settings->deepseek_key);
    give();
    if (!auth->access[0]) { free(auth); return 0; }
    const char *urls[] = {"https://chatgpt.com/backend-api/wham/usage",
                          "https://api.anthropic.com/api/oauth/usage", "https://api.deepseek.com/user/balance"};
    http_response_t response = {0};
    int status = https_request(urls[provider], auth->access, auth->account, NULL, provider == PROVIDER_CLAUDE, &response);
    if (status == 401 && provider < 2 && refresh_auth(provider, auth)) {
        free(response.body); memset(&response, 0, sizeof(response));
        status = https_request(urls[provider], auth->access, auth->account, NULL, provider == PROVIDER_CLAUDE, &response);
    }
    usage_record_t fresh;
    bool decoded = status == 200 && usage_decode(provider, response.body, time(NULL), &fresh);
    take();
    // Tokens changed by the portal invalidate this in-flight response.
    const char *current = provider < 2 ? settings->auth[provider].access : settings->deepseek_key;
    if (!strcmp(current, auth->access) && (provider != PROVIDER_CODEX || !strcmp(settings->auth[provider].account, auth->account))) {
        usage_record_t *record = &state.providers[provider];
        if (decoded) *record = fresh;
        else record->status = status == 401 ? USAGE_AUTH_ERROR : (status == 403 ? USAGE_ACCESS_DENIED : (status == 429 ? USAGE_RATE_LIMITED : (status == 200 ? USAGE_BAD_RESPONSE : USAGE_NETWORK_ERROR)));
        record->configured = true; record->http_status = status; ++state.revision;
    }
    give();
    ESP_LOGI(TAG, "EPAPER_USAGE %s http=%d decoded=%d", usage_provider_name(provider), status, decoded);
    unsigned retry = status == 429 ? (unsigned)(response.retry_after > 900 ? response.retry_after : 900) : (decoded ? 0 : 300);
    free(response.body); memset(auth, 0, sizeof(*auth)); free(auth);
    return retry;
}

static void apply_wifi(void)
{
    wifi_config_t wifi = {0};
    take();
    memcpy(wifi.sta.ssid, settings->ssid, strlen(settings->ssid));
    memcpy(wifi.sta.password, settings->password, strlen(settings->password));
    snprintf(state.ssid, sizeof(state.ssid), "%s", settings->ssid);
    state.wifi_configured = settings->ssid[0] != 0;
    state.interval_seconds = settings->interval_seconds;
    for (unsigned i = 0; i < PROVIDER_ACCOUNT_COUNT; ++i) {
        state.providers[i].configured = has_auth(settings, i);
        if (!state.providers[i].configured) usage_record_init(&state.providers[i]);
        else if (!state.providers[i].has_data) state.providers[i].status = USAGE_WAITING;
    }
    ++state.revision; give();
    wifi.sta.scan_method = WIFI_ALL_CHANNEL_SCAN;
    wifi.sta.threshold.authmode = wifi.sta.password[0] ? WIFI_AUTH_WPA2_PSK : WIFI_AUTH_OPEN;
    wifi.sta.sae_pwe_h2e = WPA3_SAE_PWE_BOTH;
    esp_wifi_disconnect();
    ESP_ERROR_CHECK_WITHOUT_ABORT(esp_wifi_set_config(WIFI_IF_STA, &wifi));
}

esp_err_t network_prepare_shutdown(unsigned timeout_ms)
{
    if (!worker_started) {
        esp_err_t err=esp_wifi_stop();
        return err==ESP_ERR_WIFI_NOT_INIT || err==ESP_ERR_WIFI_NOT_STARTED ? ESP_OK : err;
    }
    xEventGroupClearBits(shutdown_events, STOP_READY | STOP_FAILED);
    xEventGroupSetBits(shutdown_events, STOP_REQUEST);
    EventBits_t bits=xEventGroupWaitBits(shutdown_events,STOP_READY,pdFALSE,pdTRUE,pdMS_TO_TICKS(timeout_ms));
    return !(bits&STOP_READY)?ESP_ERR_TIMEOUT:((bits&STOP_FAILED)?ESP_FAIL:ESP_OK);
}
void network_resume(void)
{
    if(shutdown_events)xEventGroupClearBits(shutdown_events,STOP_REQUEST);
}

static void network_task(void *unused)
{
    (void)unused;
    uint64_t due[PROVIDER_COUNT] = {0}, last[PROVIDER_COUNT] = {0}, blocked[PROVIDER_COUNT] = {0};
    unsigned failures[PROVIDER_COUNT] = {0};
    uint64_t retry_at = 0;
    unsigned reconnect_delay = 2;
    ble_recovery_t recovery={0};uint64_t next_ble_probe=0;
    const unsigned minimum[] = {60, 300, 300};
    for (;;) {
        save_harness(false);
        if (xEventGroupGetBits(shutdown_events) & STOP_REQUEST) {
            save_harness(true);
            esp_err_t ble_result=close_ble(), web_result=ESP_OK;
            if(server){web_result=httpd_stop(server);if(web_result==ESP_OK)server=NULL;}
            esp_err_t wifi_result=esp_wifi_stop();
            bool ok=ble_result==ESP_OK && web_result==ESP_OK &&
                (wifi_result==ESP_OK || wifi_result==ESP_ERR_WIFI_NOT_STARTED || wifi_result==ESP_ERR_WIFI_NOT_INIT);
            take();state.connected=state.setup_active=state.ble_link=false;++state.revision;give();
            ESP_LOGI(TAG,"EPAPER_NETWORK quiescent=%d",ok);
            xEventGroupSetBits(shutdown_events,STOP_READY | (ok?0:STOP_FAILED));
            while(xEventGroupGetBits(shutdown_events)&STOP_REQUEST)vTaskDelay(pdMS_TO_TICKS(50));
            xEventGroupClearBits(shutdown_events,STOP_READY|STOP_FAILED);
            // Resume the requested transport, not an idle Wi-Fi radio in BLE mode.
            take();bool resume_wifi=!state.data_mode;give();
            if(resume_wifi)esp_wifi_start();
        }
        take(); bool apply = apply_pending, refresh = refresh_pending;
        int setup = setup_request, ble = ble_request;
        bool ended = ble_end_pending, retry = ble_retry_pending, testing = ble_testing, data_open=data_open_request;
        apply_pending = refresh_pending = ble_retry_pending = data_open_request = false; setup_request = ble_request = 0; give();
        if (apply) { apply_wifi(); memset(due, 0, sizeof(due)); memset(blocked, 0, sizeof(blocked)); retry_at = 0; }
        if (setup) {
            if (setup > 0 && ble_initialized) { close_ble(); if (testing) apply_wifi(); }
            open_setup(setup > 0);
        }
        if (ble < 0 || ended) {
            network_snapshot_t before; network_get_snapshot(&before);
            close_ble();
            if (testing || before.ble_status == 4) { apply_wifi(); retry_at = 0; }
        } else if (ble > 0) {
            network_snapshot_t before; network_get_snapshot(&before);
            if (before.setup_active) open_setup(false);
            if (ble_initialized && ble_is_data) close_ble();
            if (!ble_initialized) {
                esp_err_t result = open_ble(false,false);
                if (result != ESP_OK) ESP_LOGE(TAG, "BLE provisioning failed to start: %s", esp_err_to_name(result));
            }
        }
        if (retry && ble_initialized) {
            network_prov_mgr_reset_wifi_sm_state_on_failure();
            apply_wifi(); retry_at = 0;
        }
        network_snapshot_t snapshot; network_get_snapshot(&snapshot);
        uint64_t now = now_ms();
        if (data_open && ble_initialized) { close_ble(); if (testing) apply_wifi(); network_get_snapshot(&snapshot); }
        take(); bool enrollment_done=state.data_enrollment && ble_enrollment_should_close(&enrollment,now); give();
        if(enrollment_done) { close_ble(); network_get_snapshot(&snapshot); }
        if (!ble_initialized && !snapshot.setup_active && now>=data_block_until) {
            esp_err_t result=open_ble(true,data_open);
            if (result!=ESP_OK) { data_block_until=now+60000; ESP_LOGE(TAG,"BLE data service start failed: %s",esp_err_to_name(result)); }
            network_get_snapshot(&snapshot);
        }
        if (snapshot.data_pairing_open && now>=data_pairing_until) {
            take(); state.data_pairing_open=false; ++state.revision; give();
        }
        if (snapshot.setup_active && now > (uint64_t)setup_until) open_setup(false);
        take(); bool test_timeout = ble_testing && now >= ble_test_until; give();
        if (snapshot.ble_active && ((!ble_is_data && now >= ble_until) || (ble_is_data && !snapshot.data_enrollment && !data_paired && now>=ble_until) || test_timeout)) {
            if (ble_is_data && !data_paired) data_block_until=UINT64_MAX;
            close_ble();
            if (testing || test_timeout) { apply_wifi(); retry_at = 0; }
            if (test_timeout) { take(); state.ble_status = 3; ++state.revision; give(); }
        }
        if(now>=next_ble_probe){
            next_ble_probe=now+1000;
            ble_transport_probe_t transport={0};
            if(ble_initialized)ble_transport_probe(&transport);
            take();
            ble_status_snapshot=(network_ble_status_t){.initialized=ble_initialized,.enabled=transport.enabled,
                .synced=transport.synced,.advertising=transport.advertising,.connections=transport.connections,
                .data_mode=state.data_mode,.enrolling=state.data_enrollment,.authenticated=state.ble_link,
                .recoveries=ble_recovery_count,.recovery_error=ble_recovery_error};
            bool requested=ble_recovery_request;ble_recovery_request=false;
            bool allowed=ble_initialized && ble_is_data && !state.data_enrollment && data_paired &&
                !ble_testing && now>=data_block_until && !(xEventGroupGetBits(shutdown_events)&STOP_REQUEST);
            if(transport.synced && !transport.connections && state.ble_link){state.ble_link=false;ble_status_snapshot.authenticated=false;++state.revision;}
            give();
            bool due=ble_recovery_due(&recovery,now,allowed,transport.advertising,transport.connections);
            if((due || requested) && allowed && !transport.connections){
                // Probe again immediately before recovery. Never interrupt a live client.
                ble_transport_probe(&transport);
                if(!transport.connections){
                    esp_err_t result=close_ble();
                    if(result==ESP_OK)result=open_ble(true,false);
                    ble_recovery_error=result;
                    if(result==ESP_OK)++ble_recovery_count;
                    else data_block_until=now+60000;
                    ESP_LOGI(TAG,"EPAPER_BLE_RECOVERY manual=%u count=%u result=%d",requested,ble_recovery_count,result);
                    network_get_snapshot(&snapshot);
                }
            }
        }
        if (!snapshot.data_mode && snapshot.wifi_configured && !snapshot.connected && (!snapshot.ble_active || snapshot.ble_status != 1) && now >= retry_at) {
            esp_wifi_connect(); retry_at = now + reconnect_delay * 1000;
            reconnect_delay = reconnect_delay < 30 ? reconnect_delay * 2 : 60;
        }
        if (snapshot.connected) {
            reconnect_delay = 2; retry_at = 0;
            bool clock_ready = time(NULL) >= 1735689600;
            take();
            if (state.clock_ready != clock_ready) { state.clock_ready = clock_ready; ++state.revision; }
            give();
            if (clock_ready && !snapshot.data_mode) for (unsigned i = 0; i < PROVIDER_ACCOUNT_COUNT; ++i) {
                if (!snapshot.providers[i].configured) continue;
                if (refresh && last[i] + minimum[i]*1000 < due[i]) due[i] = last[i] + minimum[i]*1000;
                now = now_ms();
                if (now < due[i] || now < blocked[i]) continue;
                unsigned backoff = fetch_usage(i);
                last[i] = now_ms();
                unsigned interval = snapshot.interval_seconds > minimum[i] ? snapshot.interval_seconds : minimum[i];
                if (backoff) {
                    if (failures[i] < 4) ++failures[i];
                    unsigned exponential = 60U << failures[i];
                    if (exponential > backoff) backoff = exponential;
                    blocked[i] = last[i] + (uint64_t)backoff * 1000;
                } else { failures[i] = 0; blocked[i] = 0; }
                due[i] = last[i] + (uint64_t)interval * 1000;
            }
        }
        vTaskDelay(pdMS_TO_TICKS(NETWORK_POLL_MS));
    }
}

esp_err_t network_start(void)
{
    settings = calloc(1, sizeof(*settings));
    lock = xSemaphoreCreateMutex();
    shutdown_events=xEventGroupCreate();
    device_commands=xQueueCreate(4,sizeof(device_command_t));
    if (!settings || !lock || !shutdown_events || !device_commands) return ESP_ERR_NO_MEM;
    esp_err_t err = nvs_flash_init();
    // Preserve the existing system NVS. A failure here must not erase old user data.
    if (err != ESP_OK) return err;
    err = nvs_flash_init_partition("dashboard");
    // Never erase existing bindings or configuration to recover automatically.
    if (err != ESP_OK) return err;
    err = nvs_open_from_partition("dashboard", "settings", NVS_READWRITE, &storage);
    if (err != ESP_OK) return err;
    size_t harness_size=0;bool harness_restored=false;
    if(nvs_get_blob(storage,"harness_stats",NULL,&harness_size)==ESP_OK && harness_size<=sizeof(state.harness)) {
        void *blob=malloc(harness_size);
        if(blob){
            harness_restored=nvs_get_blob(storage,"harness_stats",blob,&harness_size)==ESP_OK && harness_stats_restore(blob,harness_size,&state.harness);
            if(harness_restored && harness_size!=sizeof(state.harness))harness_dirty=true;
            free(blob);
        }
    }
    if(!harness_restored){memset(&state.harness,0,sizeof(state.harness));state.harness.version=2;}
    size_t key_size=sizeof(data_key); if (nvs_get_str(storage,"ble_key",data_key,&key_size)!=ESP_OK) data_key[0]=0;
    uint8_t paired=0; nvs_get_u8(storage,"ble_paired",&paired); data_paired=paired==1;
    size_t size = sizeof(*settings);
    if (nvs_get_blob(storage, "config", settings, &size) != ESP_OK || size != sizeof(*settings) || settings->version != CONFIG_VERSION) {
        memset(settings, 0, sizeof(*settings)); settings->version = CONFIG_VERSION; settings->interval_seconds = 300;
        snprintf(settings->ap_password, sizeof(settings->ap_password), "%08lx%08lx", (unsigned long)esp_random(), (unsigned long)esp_random());
        snprintf(settings->setup_code, sizeof(settings->setup_code), "%08lx", (unsigned long)esp_random());
        err = persist(settings); if (err != ESP_OK) return err;
    }
    err = esp_netif_init(); if (err != ESP_OK) return err;
    err = esp_event_loop_create_default(); if (err != ESP_OK && err != ESP_ERR_INVALID_STATE) return err;
    sta_netif = esp_netif_create_default_wifi_sta();
    if (!sta_netif || !esp_netif_create_default_wifi_ap()) return ESP_ERR_NO_MEM;
    err = esp_event_handler_register(WIFI_EVENT, ESP_EVENT_ANY_ID, wifi_event, NULL); if (err != ESP_OK) return err;
    err = esp_event_handler_register(IP_EVENT, IP_EVENT_STA_GOT_IP, wifi_event, NULL); if (err != ESP_OK) return err;
    err = esp_event_handler_register(PROTOCOMM_SECURITY_SESSION_EVENT, ESP_EVENT_ANY_ID, ble_security_event, NULL); if (err != ESP_OK) return err;
    err = esp_event_handler_register(PROTOCOMM_TRANSPORT_BLE_EVENT, ESP_EVENT_ANY_ID, data_transport_event, NULL); if (err != ESP_OK) return err;
    wifi_init_config_t init = WIFI_INIT_CONFIG_DEFAULT();
    err = esp_wifi_init(&init); if (err != ESP_OK) return err;
    err = esp_wifi_set_storage(WIFI_STORAGE_RAM); if (err != ESP_OK) return err;
    err = esp_wifi_set_mode(WIFI_MODE_STA); if (err != ESP_OK) return err;
    uint8_t mac[6]; esp_read_mac(mac, ESP_MAC_WIFI_STA);
    snprintf(state.ap_ssid, sizeof(state.ap_ssid), "EPaper-%02X%02X%02X", mac[3], mac[4], mac[5]);
    snprintf(state.ble_name, sizeof(state.ble_name), "PROV_%02X%02X%02X", mac[3], mac[4], mac[5]);
    snprintf(state.data_name, sizeof(state.data_name), "EPaper_%02X%02X%02X", mac[3], mac[4], mac[5]);
    memcpy(state.ap_password, settings->ap_password, sizeof(state.ap_password));
    memcpy(state.setup_code, settings->setup_code, sizeof(state.setup_code));
    for (unsigned i = 0; i < PROVIDER_COUNT; ++i) {
        usage_record_init(&state.providers[i]);
        state.providers[i].configured = has_auth(settings, i);
        if (state.providers[i].configured) state.providers[i].status = USAGE_WAITING;
    }
    state.wifi_configured = settings->ssid[0] != 0;
    snprintf(state.ssid, sizeof(state.ssid), "%s", settings->ssid);
    state.interval_seconds = settings->interval_seconds;
    state.started = true;
    // BLE data startup needs Wi-Fi initialized for the optional provisioning API,
    // but leaves its radio stopped. open_ble() stops the manager's scan-only start.
    esp_sntp_config_t sntp = ESP_NETIF_SNTP_DEFAULT_CONFIG_MULTIPLE(2, ESP_SNTP_SERVER_LIST("time.cloudflare.com", "ntp.aliyun.com"));
    sntp.start = false;
    err = esp_netif_sntp_init(&sntp); if (err != ESP_OK) return err;
    state.data_mode=true; apply_pending = true;
    worker_started=xTaskCreate(network_task, "wifi_usage", 24576, NULL, 3, NULL)==pdPASS;
    return worker_started?ESP_OK:ESP_ERR_NO_MEM;
}
