#include "music.h"
#include "music_pcm.h"
#include "environment_log.h"
#include "audio_output.h"
#include "esp_audio_simple_dec.h"
#include "esp_audio_simple_dec_default.h"
#include "esp_heap_caps.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/queue.h"
#include "freertos/semphr.h"
#include "freertos/event_groups.h"
#include "freertos/task.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum {STOP_REQUEST=1,STOP_READY=2};
typedef struct {music_command_t command;unsigned argument;} request_t;
static QueueHandle_t commands;
static SemaphoreHandle_t mutex;
static EventGroupHandle_t shutdown_events;
static TaskHandle_t worker;
static music_snapshot_t published;
static music_library_t *library;

typedef struct {
    FILE *file;bool leased,paused,output_open,eof;
    esp_audio_simple_dec_handle_t decoder;
    esp_audio_simple_dec_raw_t raw;
    uint8_t *input,*output;
    size_t capacity,output_length,output_offset;
    esp_audio_simple_dec_info_t info;
    uint64_t frames;
} playback_t;

static void publish(music_snapshot_t *s)
{
    s->rows=0;s->first_row=(s->selected/MUSIC_VISIBLE_ROWS)*MUSIC_VISIBLE_ROWS;
    for(unsigned i=s->first_row;i<library->count&&s->rows<MUSIC_VISIBLE_ROWS;++i)
        music_text_copy(s->names[s->rows++],sizeof(s->names[0]),library->tracks[i].name);
    s->count=library->count;s->skipped=library->skipped;
    if(s->selected<s->count) {
        music_text_copy(s->title,sizeof(s->title),library->tracks[s->selected].name);
        s->format=library->tracks[s->selected].format;
    } else {s->title[0]=0;s->format=MUSIC_FORMAT_NONE;}
    xSemaphoreTake(mutex,portMAX_DELAY);s->revision=published.revision+1;published=*s;xSemaphoreGive(mutex);
}
void music_snapshot(music_snapshot_t *out)
{
    memset(out,0,sizeof(*out));out->state=MUSIC_NO_SD;out->volume=90;
    if(mutex){xSemaphoreTake(mutex,portMAX_DELAY);*out=published;xSemaphoreGive(mutex);}
}
bool music_command(music_command_t command,unsigned argument)
{
    if(!commands||!shutdown_events||!worker||(xEventGroupGetBits(shutdown_events)&STOP_REQUEST))return false;
    request_t r={command,argument};bool accepted=xQueueSend(commands,&r,0)==pdPASS;
    if(accepted)xTaskNotifyGive(worker);
    return accepted;
}
bool music_prepare_shutdown(unsigned timeout_ms)
{
    if(!shutdown_events||!worker)return true;
    xEventGroupSetBits(shutdown_events,STOP_REQUEST);
    xTaskNotifyGive(worker);
    return (xEventGroupWaitBits(shutdown_events,STOP_READY,pdFALSE,pdFALSE,pdMS_TO_TICKS(timeout_ms))&STOP_READY)!=0;
}
void music_resume(void)
{
    if(shutdown_events)xEventGroupClearBits(shutdown_events,STOP_REQUEST);
    if(worker)xTaskNotifyGive(worker);
}
static void close_track(playback_t *p)
{
    audio_output_close();p->output_open=false;
    if(p->decoder)esp_audio_simple_dec_close(p->decoder);
    if(p->file)fclose(p->file);
    if(p->leased)environment_storage_release();
    free(p->input);free(p->output);memset(p,0,sizeof(*p));
}
static void fail_track(playback_t *p,music_snapshot_t *s,const char *error)
{
    close_track(p);s->state=MUSIC_ERROR;music_text_copy(s->error,sizeof(s->error),error);
    printf("EPAPER_MUSIC_ERROR decode_or_io=1\n");publish(s);
}
static esp_audio_simple_dec_type_t decoder_type(music_format_t type)
{
    const esp_audio_simple_dec_type_t types[]={ESP_AUDIO_SIMPLE_DEC_TYPE_NONE,ESP_AUDIO_SIMPLE_DEC_TYPE_MP3,
        ESP_AUDIO_SIMPLE_DEC_TYPE_WAV,ESP_AUDIO_SIMPLE_DEC_TYPE_FLAC,ESP_AUDIO_SIMPLE_DEC_TYPE_AAC,
        ESP_AUDIO_SIMPLE_DEC_TYPE_M4A,ESP_AUDIO_SIMPLE_DEC_TYPE_OGG};
    return type<=MUSIC_FORMAT_OGG?types[type]:ESP_AUDIO_SIMPLE_DEC_TYPE_NONE;
}
static bool open_track(playback_t *p,music_snapshot_t *s)
{
    close_track(p);s->elapsed_seconds=s->sample_rate=s->source_bits=s->source_channels=0;s->error[0]=0;
    if(s->selected>=library->count||!environment_storage_acquire())return false;
    p->leased=true;
    char path[320];snprintf(path,sizeof(path),"/sdcard/music/%s",library->tracks[s->selected].name);
    p->file=fopen(path,"rb");if(!p->file){close_track(p);return false;}
    p->input=heap_caps_malloc(4096,MALLOC_CAP_INTERNAL|MALLOC_CAP_DMA);
    if(!p->input){close_track(p);return false;}
    esp_audio_simple_dec_cfg_t cfg={.dec_type=decoder_type(library->tracks[s->selected].format)};
    esp_aac_dec_cfg_t aac={.aac_plus_enable=true};esp_m4a_dec_cfg_t m4a={.aac_plus_enable=true};
    if(cfg.dec_type==ESP_AUDIO_SIMPLE_DEC_TYPE_AAC){cfg.dec_cfg=&aac;cfg.cfg_size=sizeof(aac);}
    if(cfg.dec_type==ESP_AUDIO_SIMPLE_DEC_TYPE_M4A){cfg.dec_cfg=&m4a;cfg.cfg_size=sizeof(m4a);}
    if(esp_audio_simple_dec_open(&cfg,&p->decoder)!=ESP_AUDIO_ERR_OK){close_track(p);return false;}
    p->capacity=8192;p->output=heap_caps_malloc(p->capacity,MALLOC_CAP_SPIRAM|MALLOC_CAP_8BIT);
    if(!p->output){close_track(p);return false;}
    s->state=MUSIC_PLAYING;publish(s);return true;
}
// One bounded PCM write per iteration keeps control/stop responsive.
static int playback_step(playback_t *p,music_snapshot_t *s)
{
    if(!environment_storage_available())return -2;
    if(p->output_offset<p->output_length) {
        if(!p->output_open) {
            if(audio_output_open(p->info.sample_rate,s->volume)!=ESP_OK)return -3;
            p->output_open=true;
        }
        int16_t pcm[1024];size_t frames=music_pcm_stereo16(p->output+p->output_offset,
            p->output_length-p->output_offset,p->info.bits_per_sample,p->info.channel,pcm);
        if(!frames||audio_output_write(pcm,frames)!=ESP_OK)return -3;
        p->output_offset+=frames*(p->info.bits_per_sample/8)*p->info.channel;p->frames+=frames;
        unsigned elapsed=(unsigned)(p->frames/p->info.sample_rate);
        if(elapsed/10!=s->elapsed_seconds/10){s->elapsed_seconds=elapsed;publish(s);}
        return 0;
    }
    if(!p->raw.len) {
        if(p->eof)return p->frames?1:-1;
        size_t n=fread(p->input,1,4096,p->file);
        // Mark an exact-sized final block as EOS too, so parsers flush the last frame.
        if(n==4096) {
            int next=fgetc(p->file);
            if(next!=EOF&&ungetc(next,p->file)==EOF)return -2;
        }
        if(ferror(p->file))return -2;
        p->eof=feof(p->file);p->raw=(esp_audio_simple_dec_raw_t){.buffer=p->input,.len=n,.eos=p->eof};
        if(!n)return p->frames?1:-1;
    }
    esp_audio_simple_dec_out_t out={.buffer=p->output,.len=p->capacity};
    esp_audio_err_t err=esp_audio_simple_dec_process(p->decoder,&p->raw,&out);
    if(err==ESP_AUDIO_ERR_BUFF_NOT_ENOUGH) {
        if(out.needed_size<=p->capacity||out.needed_size>524288)return -1;
        void *b=heap_caps_realloc(p->output,out.needed_size,MALLOC_CAP_SPIRAM|MALLOC_CAP_8BIT);
        if(!b)return -1;
        p->output=b;p->capacity=out.needed_size;return 0;
    }
    if(err!=ESP_AUDIO_ERR_OK||p->raw.consumed>p->raw.len||out.decoded_size>p->capacity)return -1;
    if(!p->raw.consumed&&!out.decoded_size)return -1;
    p->raw.buffer+=p->raw.consumed;p->raw.len-=p->raw.consumed;
    p->output_length=out.decoded_size;p->output_offset=0;
    if(out.decoded_size) {
        esp_audio_simple_dec_info_t info;
        if(esp_audio_simple_dec_get_info(p->decoder,&info)!=ESP_AUDIO_ERR_OK||
            !music_pcm_supported(info.sample_rate,info.bits_per_sample,info.channel)||
            out.decoded_size%((info.bits_per_sample/8)*info.channel))return -1;
        if(p->info.sample_rate!=info.sample_rate||p->info.channel!=info.channel||p->info.bits_per_sample!=info.bits_per_sample) {
            audio_output_close();p->output_open=false;p->info=info;
            s->sample_rate=info.sample_rate;s->source_bits=info.bits_per_sample;s->source_channels=info.channel;publish(s);
            printf("EPAPER_MUSIC_DECODE format=%s rate=%u bits=%u channels=%u\n",music_format_label(s->format),(unsigned)s->sample_rate,s->source_bits,s->source_channels);
        }
    }
    return 0;
}
static void scan(music_snapshot_t *s)
{
    s->state=MUSIC_SCANNING;publish(s);
    char selected[MUSIC_NAME_MAX+1];music_text_copy(selected,sizeof(selected),s->title);
    if(!environment_storage_acquire()){s->state=MUSIC_NO_SD;library->count=0;publish(s);return;}
    int err=music_library_scan(library,"/sdcard/music");environment_storage_release();
    s->selected=0;for(unsigned i=0;i<library->count;++i)if(!strcmp(selected,library->tracks[i].name)){s->selected=i;break;}
    s->state=err?MUSIC_ERROR:(library->count?MUSIC_STOPPED:MUSIC_EMPTY);
    if(err)music_text_copy(s->error,sizeof(s->error),"无法读取音乐目录");else s->error[0]=0;
    publish(s);printf("EPAPER_MUSIC_SCAN count=%u skipped=%u ok=%d\n",library->count,library->skipped,err==0);
}
static void music_task(void *unused)
{
    (void)unused;playback_t p={0};music_snapshot_t s={.state=MUSIC_NO_SD,.volume=90};
    bool scan_pending=true,previous_ready=false;TickType_t next_scan=0;
    if(esp_audio_dec_register_default()!=ESP_AUDIO_ERR_OK || esp_audio_simple_dec_register_default()!=ESP_AUDIO_ERR_OK) {
        s.state=MUSIC_ERROR;music_text_copy(s.error,sizeof(s.error),"解码器初始化失败");publish(&s);
        for(;;) {
            if(xEventGroupGetBits(shutdown_events)&STOP_REQUEST)xEventGroupSetBits(shutdown_events,STOP_READY);
            else xEventGroupClearBits(shutdown_events,STOP_READY);
            ulTaskNotifyTake(pdTRUE,pdMS_TO_TICKS(1000));
        }
    }
    publish(&s);
    for(;;) {
        if(xEventGroupGetBits(shutdown_events)&STOP_REQUEST) {
            close_track(&p);s.state=library->count?MUSIC_STOPPED:MUSIC_EMPTY;publish(&s);xQueueReset(commands);
            xEventGroupSetBits(shutdown_events,STOP_READY);
            while(xEventGroupGetBits(shutdown_events)&STOP_REQUEST)vTaskDelay(pdMS_TO_TICKS(20));
            xEventGroupClearBits(shutdown_events,STOP_READY);
        }
        bool ready=environment_storage_available();
        if(ready&&!previous_ready)scan_pending=true;
        previous_ready=ready;
        if(!ready&&p.file){close_track(&p);s.state=MUSIC_NO_SD;publish(&s);}
        request_t r;
        while(xQueueReceive(commands,&r,0)==pdTRUE) {
            if(r.command==MUSIC_CMD_SCAN)scan_pending=true;
            else if(r.command==MUSIC_CMD_VOLUME_UP||r.command==MUSIC_CMD_VOLUME_DOWN) {
                s.volume=r.command==MUSIC_CMD_VOLUME_UP?(s.volume>=95?100:s.volume+5):(s.volume<=5?0:s.volume-5);
                audio_output_volume(s.volume);publish(&s);
            } else if(r.command==MUSIC_CMD_STOP) {
                close_track(&p);s.state=library->count?MUSIC_STOPPED:MUSIC_EMPTY;s.elapsed_seconds=0;publish(&s);
            } else if(r.command==MUSIC_CMD_NEXT||r.command==MUSIC_CMD_PREVIOUS||r.command==MUSIC_CMD_PLAY_INDEX) {
                if(!library->count)continue;
                bool was_playing=s.state==MUSIC_PLAYING;
                if(r.command==MUSIC_CMD_PLAY_INDEX&&r.argument>=library->count)continue;
                close_track(&p);
                if(r.command==MUSIC_CMD_PLAY_INDEX){if(r.argument>=library->count)continue;s.selected=r.argument;was_playing=true;}
                else s.selected=(s.selected+library->count+(r.command==MUSIC_CMD_NEXT?1:-1))%library->count;
                s.state=MUSIC_STOPPED;s.elapsed_seconds=s.sample_rate=s.source_bits=s.source_channels=0;publish(&s);
                if(was_playing&&!open_track(&p,&s))fail_track(&p,&s,"无法打开这首歌曲");
            } else if(r.command==MUSIC_CMD_TOGGLE) {
                if(s.state==MUSIC_PLAYING) {
                    audio_output_close();p.output_open=false;p.paused=true;s.state=MUSIC_PAUSED;
                    if(p.info.sample_rate)s.elapsed_seconds=(unsigned)(p.frames/p.info.sample_rate);
                    publish(&s);
                } else if(s.state==MUSIC_PAUSED&&p.file) {p.paused=false;s.state=MUSIC_PLAYING;publish(&s);}
                else if(library->count&&!open_track(&p,&s))fail_track(&p,&s,"无法打开这首歌曲");
            }
        }
        if(!p.file&&ready&&(scan_pending||(int32_t)(xTaskGetTickCount()-next_scan)>=0)) {
            scan(&s);scan_pending=false;next_scan=xTaskGetTickCount()+pdMS_TO_TICKS(60000);
        }
        if(s.state==MUSIC_PLAYING&&p.file) {
            int result=playback_step(&p,&s);
            if(result>0) {
                printf("EPAPER_MUSIC_DONE frames=%llu\n",(unsigned long long)p.frames);
                vTaskDelay(pdMS_TO_TICKS(100)); // Let the final DMA audio drain before muting.
                close_track(&p);
                if(s.selected+1<library->count){++s.selected;if(!open_track(&p,&s))fail_track(&p,&s,"无法打开下一首");}
                else {s.state=MUSIC_STOPPED;s.elapsed_seconds=0;publish(&s);}
            } else if(result<0)fail_track(&p,&s,result==-2?"SD卡读取失败":(result==-3?"音频输出失败":"文件损坏或编码不支持"));
            else if(!p.output_length)vTaskDelay(1);
        } else ulTaskNotifyTake(pdTRUE,pdMS_TO_TICKS(1000));
    }
}
void music_start(void)
{
    audio_output_init();mutex=xSemaphoreCreateMutex();commands=xQueueCreate(8,sizeof(request_t));shutdown_events=xEventGroupCreate();
    library=heap_caps_calloc(1,sizeof(*library),MALLOC_CAP_SPIRAM|MALLOC_CAP_8BIT);
    if(!mutex||!commands||!shutdown_events||!library){ESP_LOGE("music","Music allocation failed");return;}
    // No Flash/NVS operations run in this task; keep its decode stack out of DMA RAM.
    if(xTaskCreatePinnedToCoreWithCaps(music_task,"sd_music",32768,NULL,5,&worker,1,MALLOC_CAP_SPIRAM|MALLOC_CAP_8BIT)!=pdPASS)ESP_LOGE("music","Music task unavailable");
}
