// Minute records on SD, daily files, bounded in-memory current-day working.
#include "environment_log.h"
#include "environment_csv.h"
#include "energy_policy.h"
#include "driver/sdmmc_host.h"
#include "esp_heap_caps.h"
#include "esp_log.h"
#include "esp_vfs_fat.h"
#include "sdmmc_cmd.h"
#include "freertos/FreeRTOS.h"
#include "freertos/event_groups.h"
#include "freertos/queue.h"
#include "freertos/semphr.h"
#include "freertos/task.h"
#include <errno.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

typedef struct {float temperature,humidity;bool valid;int battery;TickType_t captured;} sample_t;
typedef struct {int16_t temperature,battery;uint16_t humidity;bool observed,valid,stored;} minute_t;
static QueueHandle_t samples;
static QueueHandle_t verify_queue;
static SemaphoreHandle_t mutex;
static unsigned storage_readers;
static bool storage_ready,unmount_pending;
static EventGroupHandle_t shutdown_events;
static bool logger_started;
#define STOP_REQUEST BIT0
#define STOP_READY BIT1
static environment_history_t history, working, published_working;
static minute_t *minutes;
static sdmmc_card_t *card;

bool environment_storage_acquire(void)
{
    if(!mutex)return false;
    xSemaphoreTake(mutex,portMAX_DELAY);
    bool ready=storage_ready;if(ready)++storage_readers;xSemaphoreGive(mutex);return ready;
}
void environment_storage_release(void)
{
    if(!mutex)return;
    xSemaphoreTake(mutex,portMAX_DELAY);
    if(storage_readers)--storage_readers;
    xSemaphoreGive(mutex);
}
bool environment_storage_available(void)
{
    if(!mutex)return false;
    xSemaphoreTake(mutex,portMAX_DELAY);
    bool ready=storage_ready;xSemaphoreGive(mutex);return ready;
}
static void storage_set_ready(bool ready)
{
    xSemaphoreTake(mutex,portMAX_DELAY);storage_ready=ready;xSemaphoreGive(mutex);
}
static void storage_unmount(void)
{
    xSemaphoreTake(mutex,portMAX_DELAY);storage_ready=false;
    bool idle=storage_readers==0;xSemaphoreGive(mutex);
    if(idle&&card){esp_vfs_fat_sdcard_unmount("/sdcard",card);card=NULL;unmount_pending=false;}
    else unmount_pending=true;
}
static const char *TAG="environment";
static const char *base="/sdcard/EPaper/env";

static void publish(void)
{
    environment_history_t h=working;
    h.observed=h.valid_samples=h.stored_samples=0;
    memset(h.valid,0,sizeof(h.valid));
    bool first=true;
    for(unsigned b=0;b<ENV_HISTORY_BUCKETS;++b) {
        int temp=0,rh=0,count=0;
        for(unsigned i=b*15;i<(b+1)*15;++i) {
            const minute_t *m=&minutes[i];
            if(m->observed)++h.observed;
            if(m->stored)++h.stored_samples;
            if(!m->observed || !m->valid)continue;
            ++h.valid_samples;temp+=m->temperature;rh+=m->humidity;++count;
            float t=m->temperature/100.0f,r=m->humidity/100.0f;
            if(first){h.temperature_min=h.temperature_max=t;h.humidity_min=h.humidity_max=r;first=false;}
            if(t<h.temperature_min)h.temperature_min=t;
            if(t>h.temperature_max)h.temperature_max=t;
            if(r<h.humidity_min)h.humidity_min=r;
            if(r>h.humidity_max)h.humidity_max=r;
        }
        if(count){h.valid[b]=true;h.temperature[b]=(int16_t)lroundf((float)temp/count);h.humidity[b]=(uint16_t)lroundf((float)rh/count);}
    }
    xSemaphoreTake(mutex,portMAX_DELAY);h.revision=history.revision;if(memcmp(&h,&history,sizeof(h))){h.revision=history.revision+1;history=h;}xSemaphoreGive(mutex);
    published_working=working;
    energy_note_history();
}

void environment_log_snapshot(environment_history_t *out)
{
    memset(out,0,sizeof(*out));
    if(!mutex)return;
    xSemaphoreTake(mutex,portMAX_DELAY);*out=history;xSemaphoreGive(mutex);
}
void environment_log_sample(float temperature,float humidity,bool valid,int battery_percent)
{
    if(!samples)return;
    sample_t s={temperature,humidity,valid,battery_percent,xTaskGetTickCount()};
    xQueueOverwrite(samples,&s);
}
void environment_log_verify_request(void){bool request=true;if(verify_queue)xQueueOverwrite(verify_queue,&request);}
esp_err_t environment_log_prepare_shutdown(unsigned timeout_ms)
{
    if (!logger_started) return ESP_OK;
    xEventGroupClearBits(shutdown_events, STOP_READY);
    xEventGroupSetBits(shutdown_events, STOP_REQUEST);
    EventBits_t bits = xEventGroupWaitBits(shutdown_events, STOP_READY, pdFALSE, pdTRUE, pdMS_TO_TICKS(timeout_ms));
    return bits & STOP_READY ? ESP_OK : ESP_ERR_TIMEOUT;
}
void environment_log_resume(void)
{
    if (shutdown_events) xEventGroupClearBits(shutdown_events, STOP_REQUEST);
}
static esp_err_t mount_card(void)
{
    sdmmc_host_t host=SDMMC_HOST_DEFAULT();host.max_freq_khz=SDMMC_FREQ_DEFAULT;
    sdmmc_slot_config_t slot=SDMMC_SLOT_CONFIG_DEFAULT();slot.width=4;
    slot.clk=16;slot.cmd=17;slot.d0=15;slot.d1=7;slot.d2=8;slot.d3=18;
    esp_vfs_fat_sdmmc_mount_config_t cfg={.format_if_mount_failed=false,.max_files=6,.allocation_unit_size=16*1024};
    esp_err_t err=esp_vfs_fat_sdmmc_mount("/sdcard",&host,&slot,&cfg,&card);
    if(err!=ESP_OK)return err;
    if((mkdir("/sdcard/EPaper",0775) && errno!=EEXIST) || (mkdir(base,0775) && errno!=EEXIST)) {
        esp_vfs_fat_sdcard_unmount("/sdcard",card);card=NULL;return ESP_FAIL;
    }
    ESP_LOGI(TAG,"EPAPER_SD mounted bus=4bit clock=20MHz");return ESP_OK;
}
static void filename(char *path,size_t size,int day)
{
    snprintf(path,size,"%s/%08d.csv",base,day);
}
static void load_day(int day)
{
    char path[96],line[192];filename(path,sizeof(path),day);
    FILE *f=fopen(path,"r");if(!f)return;
    while(fgets(line,sizeof(line),f)) {
        if(!strchr(line,'\n')){int ch;while((ch=fgetc(f))!=EOF&&ch!='\n'){}continue;}
        environment_row_t row;unsigned index;
        if(!environment_csv_parse(line,day,&row,&index))continue;
        minute_t *m=&minutes[index];
        if(!m->observed) {
            m->temperature=lroundf(row.temperature*100);m->humidity=lroundf(row.humidity*100);m->battery=row.battery;m->valid=row.valid;
        }
        m->observed=m->stored=true;
        if(row.timestamp>working.last_saved_at)working.last_saved_at=row.timestamp;
    }
    fclose(f);
}
static bool save_day(int day,time_t midnight)
{
    char path[96],line[192];filename(path,sizeof(path),day);
    FILE *f=fopen(path,"a+");if(!f)return false;
    bool ok=fseek(f,0,SEEK_END)==0;long length=ftell(f);
    if(length==0)ok=ok&&fputs("timestamp_local,unix_time,temperature_c,humidity_pct,sensor_valid,battery_pct\n",f)>=0;
    else if(length>0) {
        // Preserve an interrupted append as an invalid line; do not concatenate a new record onto it.
        if(fseek(f,-1,SEEK_END)==0){int last=fgetc(f);fseek(f,0,SEEK_END);if(last!='\n')ok=ok&&fputc('\n',f)!=EOF;}
    }
    unsigned pending[1440],count=0;
    for(unsigned i=0;ok&&i<1440;++i) {
        minute_t *m=&minutes[i];if(!m->observed || m->stored)continue;
        environment_row_t row={midnight+i*60,m->temperature/100.0f,m->humidity/100.0f,m->valid,m->battery};
        if(!environment_csv_format(line,sizeof(line),&row) || fputs(line,f)<0){ok=false;break;}pending[count++]=i;
    }
    if(fflush(f)!=0)ok=false;
    if(fsync(fileno(f))!=0)ok=false;
    if(fclose(f)!=0)ok=false;
    if(ok)for(unsigned j=0;j<count;++j){minutes[pending[j]].stored=true;working.last_saved_at=midnight+pending[j]*60;}
    if(ok&&count)ESP_LOGI(TAG,"EPAPER_ENV_SAVED day=%d rows=%u latest=%lld",day,count,(long long)working.last_saved_at);
    return ok;
}
static void verify_file(void)
{
    char path[96],line[192];filename(path,sizeof(path),working.day);
    FILE *f=card?fopen(path,"r"):NULL;
    if(!f){printf("EPAPER_SD_VERIFY opened=0\n");return;}
    unsigned rows=0,valid=0;environment_row_t last={0};
    for(unsigned i=0;i<4096 && fgets(line,sizeof(line),f);++i) {
        environment_row_t row;unsigned minute;
        if(environment_csv_parse(line,working.day,&row,&minute)){++rows;if(row.valid)++valid;last=row;}
    }
    fclose(f);printf("EPAPER_SD_VERIFY opened=1 day=%d rows=%u valid=%u latest=%lld temperature=%.2f humidity=%.2f\n",working.day,rows,valid,(long long)last.timestamp,(double)last.temperature,(double)last.humidity);
}
static void logger_task(void *unused)
{
    (void)unused;
    TickType_t retry_at=0;int loaded_day=0,last_attempt_minute=-1;sample_t s={0};
    for(;;) {
        if(unmount_pending)storage_unmount();
        bool minute_changed=false;
        xQueuePeek(samples,&s,0);
        time_t now=time(NULL);struct tm local;localtime_r(&now,&local);
        bool clock=now>=1735689600 && now<=4102444800;
        working.clock_ready=clock;
        if(!card && (int32_t)(xTaskGetTickCount()-retry_at)>=0) {
            esp_err_t err=mount_card();working.mounted=err==ESP_OK;working.last_error=err;retry_at=xTaskGetTickCount()+pdMS_TO_TICKS(60000);
            if(err!=ESP_OK)ESP_LOGW(TAG,"EPAPER_SD unavailable error=%s; no format",esp_err_to_name(err));
            else {loaded_day=0;storage_set_ready(true);}
        }
        if(clock) {
            int day=(local.tm_year+1900)*10000+(local.tm_mon+1)*100+local.tm_mday;
            unsigned minute=local.tm_hour*60+local.tm_min;
            if(working.day!=day) {memset(minutes,0,1440*sizeof(*minutes));working.day=day;working.last_saved_at=0;loaded_day=0;last_attempt_minute=-1;}
            if(card && working.mounted && loaded_day!=day){load_day(day);loaded_day=day;}
            minute_t *m=&minutes[minute];
            if(!m->observed && s.captured && xTaskGetTickCount()-s.captured<=pdMS_TO_TICKS(10000)) {
                minute_changed=true;
                m->observed=true;m->valid=s.valid&&isfinite(s.temperature)&&isfinite(s.humidity)&&s.temperature>=-45&&s.temperature<=130&&s.humidity>=0&&s.humidity<=100;
                if(m->valid){m->temperature=lroundf(s.temperature*100);m->humidity=lroundf(s.humidity*100);}
                m->battery=s.battery>=0&&s.battery<=100?s.battery:-1;
            }
            if(card && working.mounted && m->observed && last_attempt_minute!=(int)minute) {
                local.tm_hour=local.tm_min=local.tm_sec=0;time_t midnight=mktime(&local);
                working.write_ok=save_day(day,midnight);last_attempt_minute=minute;
                if(!working.write_ok) {
                    working.last_error=ESP_FAIL;working.mounted=false;
                    storage_unmount();retry_at=xTaskGetTickCount()+pdMS_TO_TICKS(60000);
                    ESP_LOGW(TAG,"EPAPER_SD write failed; retained unsaved minute samples in RAM");
                }
            }
        }
        bool verify;if(xQueueReceive(verify_queue,&verify,0)==pdTRUE)verify_file();
        if (xEventGroupGetBits(shutdown_events) & STOP_REQUEST) {
            // Retry any pending append before acknowledging; save_day always closes the file.
            if (card && clock) {
                struct tm midnight_local;localtime_r(&now,&midnight_local);
                midnight_local.tm_hour=midnight_local.tm_min=midnight_local.tm_sec=0;
                working.write_ok=save_day(working.day,mktime(&midnight_local));
            }
            publish();
            environment_history_t h;environment_log_snapshot(&h);
            ESP_LOGI(TAG,"EPAPER_SD quiescent stored=%u unsaved=%u",h.stored_samples,h.observed-h.stored_samples);
            xEventGroupSetBits(shutdown_events, STOP_READY);
            while (xEventGroupGetBits(shutdown_events) & STOP_REQUEST) vTaskDelay(pdMS_TO_TICKS(50));
            xEventGroupClearBits(shutdown_events, STOP_READY);
        }
        if(minute_changed || memcmp(&working,&published_working,sizeof(working)))publish();
        vTaskDelay(pdMS_TO_TICKS(1000));
    }
}
esp_err_t environment_log_start(void)
{
    minutes=heap_caps_calloc(1440,sizeof(*minutes),MALLOC_CAP_SPIRAM|MALLOC_CAP_8BIT);
    if(!minutes)minutes=calloc(1440,sizeof(*minutes));
    mutex=xSemaphoreCreateMutex();samples=xQueueCreate(1,sizeof(sample_t));verify_queue=xQueueCreate(1,sizeof(bool));shutdown_events=xEventGroupCreate();
    if(!minutes||!mutex||!samples||!verify_queue||!shutdown_events)return ESP_ERR_NO_MEM;
    logger_started=xTaskCreate(logger_task,"environment_sd",12288,NULL,2,NULL)==pdPASS;
    return logger_started?ESP_OK:ESP_ERR_NO_MEM;
}
