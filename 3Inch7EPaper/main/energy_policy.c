#include "energy_policy.h"
#include "esp_pm.h"
#include "esp_attr.h"
#include "freertos/FreeRTOS.h"

static DRAM_ATTR portMUX_TYPE stats_lock=portMUX_INITIALIZER_UNLOCKED;
static DRAM_ATTR uint64_t sleep_count,sleep_us;
static uint64_t telemetry_publishes,history_rebuilds;
static esp_err_t IRAM_ATTR sleep_exit(int64_t us,void *arg)
{
    (void)arg;
    if(us>0) {
        portENTER_CRITICAL(&stats_lock);
        ++sleep_count;sleep_us+=(uint64_t)us;
        portEXIT_CRITICAL(&stats_lock);
    }
    return ESP_OK;
}
void energy_start(void)
{
    const esp_pm_config_t config={.max_freq_mhz=CPU_MAX_MHZ,.min_freq_mhz=CPU_MIN_MHZ,.light_sleep_enable=true};
    ESP_ERROR_CHECK(esp_pm_configure(&config));
    esp_pm_sleep_cbs_register_config_t callbacks={.exit_cb=sleep_exit};
    ESP_ERROR_CHECK(esp_pm_light_sleep_register_cbs(&callbacks));
}
void energy_counters(uint64_t *count,uint64_t *slept_us)
{
    portENTER_CRITICAL(&stats_lock);
    *count=sleep_count;*slept_us=sleep_us;
    portEXIT_CRITICAL(&stats_lock);
}
void energy_note_telemetry(void)
{
    portENTER_CRITICAL(&stats_lock);++telemetry_publishes;portEXIT_CRITICAL(&stats_lock);
}
void energy_note_history(void)
{
    portENTER_CRITICAL(&stats_lock);++history_rebuilds;portEXIT_CRITICAL(&stats_lock);
}
void energy_work_counters(uint64_t *telemetry,uint64_t *history)
{
    portENTER_CRITICAL(&stats_lock);
    *telemetry=telemetry_publishes;*history=history_rebuilds;
    portEXIT_CRITICAL(&stats_lock);
}
