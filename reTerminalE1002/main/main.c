#include "app_state.h"
#include "ble_link.h"
#include "pins.h"
#include "homecraft_wire.h"
#include "cJSON.h"
#include "driver/gpio.h"
#include "driver/i2c_master.h"
#include "esp_adc/adc_oneshot.h"
#include "esp_adc/adc_cali.h"
#include "esp_adc/adc_cali_scheme.h"
#include "esp_event.h"
#include "esp_heap_caps.h"
#include "esp_log.h"
#include "esp_psram.h"
#include "esp_timer.h"
#include "nvs_flash.h"
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"
#include "freertos/task.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <time.h>
static dashboard_state_t state;
static SemaphoreHandle_t mutex;
static bool dirty=true,force_full=true,display_ready,sample_pending,initial_data,urgent;
static SemaphoreHandle_t sensor_mutex;
static uint32_t request_id,sample_id;static uint8_t sample_result;
static int64_t requested_at,last_sample_ms;
static uint32_t last_ms;static dashboard_state_t shown_state;static int64_t shown_at;
static i2c_master_dev_handle_t sht,charger;
static adc_oneshot_unit_handle_t adc;static adc_cali_handle_t adc_cali;
static void battery_init(void){gpio_config_t en={.pin_bit_mask=1ULL<<E1002_BATTERY_ENABLE,.mode=GPIO_MODE_OUTPUT};if(gpio_config(&en)!=ESP_OK)return;gpio_set_level(E1002_BATTERY_ENABLE,0);
 adc_oneshot_unit_init_cfg_t unit={.unit_id=ADC_UNIT_1};adc_oneshot_chan_cfg_t ch={.atten=ADC_ATTEN_DB_12,.bitwidth=ADC_BITWIDTH_DEFAULT};
 adc_cali_curve_fitting_config_t cali={.unit_id=ADC_UNIT_1,.chan=ADC_CHANNEL_0,.atten=ADC_ATTEN_DB_12,.bitwidth=ADC_BITWIDTH_DEFAULT};
 if(adc_oneshot_new_unit(&unit,&adc)!=ESP_OK||adc_oneshot_config_channel(adc,ADC_CHANNEL_0,&ch)!=ESP_OK||adc_cali_create_scheme_curve_fitting(&cali,&adc_cali)!=ESP_OK)adc=NULL;}
// Seeed's sequence: enable the divider, settle, read millivolts, disable; the divider halves the cell voltage.
static int battery_read(void){if(!adc)return -1;gpio_set_level(E1002_BATTERY_ENABLE,1);vTaskDelay(pdMS_TO_TICKS(10));int sum=0,n=0;for(int i=0;i<8;++i){int raw,mv;if(adc_oneshot_read(adc,ADC_CHANNEL_0,&raw)==ESP_OK&&adc_cali_raw_to_voltage(adc_cali,raw,&mv)==ESP_OK){sum+=mv;++n;}}gpio_set_level(E1002_BATTERY_ENABLE,0);return n?2*sum/n:-1;}
static void take(void){xSemaphoreTake(mutex,portMAX_DELAY);}static void give(void){xSemaphoreGive(mutex);}
void app_snapshot(dashboard_state_t *out){take();*out=state;out->now=time(NULL);give();}
bool app_ready_to_pair(void){take();bool ready=display_ready;give();return ready;}
void app_pair_request(void){if(app_ready_to_pair())ble_link_open_pairing();}
void app_set_pairing(bool open,const char *code,const char *name){take();state.pairing=open;snprintf(state.pairing_code,sizeof(state.pairing_code),"%s",code?code:"");if(name)snprintf(state.device_name,sizeof(state.device_name),"%s",name);dirty=true;force_full=true;give();}
void app_set_connected(bool connected){take();if(state.connected!=connected){state.connected=connected;dirty=true;}give();}
static void change_clock(int64_t stamp){int64_t current=time(NULL),target=usage_clock_target(current,stamp);if(target!=current){struct timeval tv={.tv_sec=target};settimeofday(&tv,NULL);}}
bool app_accept_usage(const char *json){usage_record_t record;provider_id_t provider;if(!usage_packet_decode(json,&provider,&record))return false;
 change_clock(record.fetched_at);take();usage_record_t *old=&state.usage[provider];
 if(old->has_data&&record.fetched_at<old->fetched_at){give();return true;}
 if(record.has_data&&!old->has_data)initial_data=true;
 if(provider==0&&record.has_data&&record.windows[0].remaining_tenths>=0&&record.windows[0].remaining_tenths<=200&&old->windows[0].remaining_tenths>200)urgent=true;
 if(!usage_display_update(old,&record)){give();return false;}
 dirty=true;requested_at=esp_timer_get_time()/1000;give();return true;}
static bool integer(const cJSON *v,int64_t low,int64_t high,int64_t *out){if(!cJSON_IsNumber(v)||!isfinite(v->valuedouble)||floor(v->valuedouble)!=v->valuedouble||v->valuedouble<low||v->valuedouble>high)return false;*out=(int64_t)v->valuedouble;return true;}
static bool weather_decode(const cJSON *obj,weather_t *w){
 const cJSON *city=cJSON_GetObjectItemCaseSensitive(obj,"city"),*arr=cJSON_GetObjectItemCaseSensitive(obj,"values");int64_t values[5];
 if(!cJSON_IsString(city)||!city->valuestring||!city->valuestring[0]||strlen(city->valuestring)>48||!cJSON_IsArray(arr)||cJSON_GetArraySize(arr)!=5)return false;
 for(const unsigned char *p=(const unsigned char *)city->valuestring;*p;++p)if(*p<32||*p==127)return false;
 for(int i=0;i<5;++i)if(!integer(cJSON_GetArrayItem(arr,i),i==0?1735689600:i==4?0:-900,i==0?4102444800LL:i==4?99:700,&values[i]))return false;
 if(values[2]>values[3])return false;
 // Optional tomorrow [code,low,high]; a malformed one rejects the message rather than showing half of it.
 const cJSON *next=cJSON_GetObjectItemCaseSensitive(obj,"tomorrow");int64_t t[3]={0};
 if(next){if(!cJSON_IsArray(next)||cJSON_GetArraySize(next)!=3)return false;for(int i=0;i<3;++i)if(!integer(cJSON_GetArrayItem(next,i),i==0?0:-900,i==0?99:700,&t[i]))return false;if(t[1]>t[2])return false;}
 *w=(weather_t){.valid=true,.has_tomorrow=next!=NULL,.fetched_at=values[0],.temperature_tenths=values[1],.low_tenths=values[2],.high_tenths=values[3],.code=values[4],.tomorrow_code=t[0],.tomorrow_low=t[1],.tomorrow_high=t[2]};snprintf(w->city,sizeof(w->city),"%s",city->valuestring);return true;
}
static bool text_field(const cJSON *v,size_t max,bool allow_empty){if(!cJSON_IsString(v)||!v->valuestring)return false;size_t n=strlen(v->valuestring);if(n>max||(!allow_empty&&!n))return false;for(const unsigned char *p=(const unsigned char *)v->valuestring;*p;++p)if(*p<32||*p==127)return false;return true;}
static bool todo_decode(const cJSON *obj,unsigned *q,todo_quadrant_t *out){int64_t quadrant;const cJSON *items=cJSON_GetObjectItemCaseSensitive(obj,"items");
 if(!integer(cJSON_GetObjectItemCaseSensitive(obj,"q"),0,TODO_QUADRANTS-1,&quadrant)||!cJSON_IsArray(items)||cJSON_GetArraySize(items)>TODO_ITEMS)return false;
 todo_quadrant_t t={0};const cJSON *it;cJSON_ArrayForEach(it,items){if(!text_field(it,48,false))return false;snprintf(t.items[t.count++],sizeof(t.items[0]),"%s",it->valuestring);}
 int64_t total=t.count;const cJSON *n=cJSON_GetObjectItemCaseSensitive(obj,"n");if(n&&(!integer(n,t.count,999,&total)))return false;t.total=(unsigned)total;
 *q=(unsigned)quadrant;*out=t;return true;}
static bool poem_decode(const cJSON *obj,poem_t *out){const cJSON *title=cJSON_GetObjectItemCaseSensitive(obj,"title"),*author=cJSON_GetObjectItemCaseSensitive(obj,"author"),*lines=cJSON_GetObjectItemCaseSensitive(obj,"lines");
 if(!text_field(title,36,false)||!text_field(author,24,true)||!cJSON_IsArray(lines)||cJSON_GetArraySize(lines)<1||cJSON_GetArraySize(lines)>4)return false;
 poem_t p={.valid=true};snprintf(p.title,sizeof(p.title),"%s",title->valuestring);snprintf(p.author,sizeof(p.author),"%s",author->valuestring);
 const cJSON *it;cJSON_ArrayForEach(it,lines){if(!text_field(it,60,false))return false;snprintf(p.lines[p.count++],sizeof(p.lines[0]),"%s",it->valuestring);}
 *out=p;return true;}
// Up to two named places; a reading is t, h and at together or none of them (not known yet).
static bool places_decode(const cJSON *obj,place_t out[PLACES],unsigned *count){const cJSON *rows=cJSON_GetObjectItemCaseSensitive(obj,"rows");
 if(!cJSON_IsArray(rows)||cJSON_GetArraySize(rows)>PLACES)return false;
 place_t p[PLACES]={0};unsigned n=0;const cJSON *it;
 cJSON_ArrayForEach(it,rows){const cJSON *name=cJSON_GetObjectItemCaseSensitive(it,"name"),*t=cJSON_GetObjectItemCaseSensitive(it,"t"),*h=cJSON_GetObjectItemCaseSensitive(it,"h"),*at=cJSON_GetObjectItemCaseSensitive(it,"at");
  if(!cJSON_IsObject(it)||!text_field(name,48,false))return false;
  place_t *q=&p[n++];snprintf(q->name,sizeof(q->name),"%s",name->valuestring);q->here=cJSON_IsTrue(cJSON_GetObjectItemCaseSensitive(it,"here"));
  if(t||h||at){int64_t v[3];if(q->here||!integer(t,-400,1250,&v[0])||!integer(h,0,1000,&v[1])||!integer(at,1735689600,4102444800LL,&v[2]))return false;
   q->known=true;q->temperature_tenths=(int)v[0];q->humidity_tenths=(int)v[1];q->at=v[2];}}
 memcpy(out,p,sizeof(p));*count=n;return true;}
bool app_accept_weather(const char *json){cJSON *obj=cJSON_Parse(json);weather_t w;bool ok=obj&&weather_decode(obj,&w);cJSON_Delete(obj);if(ok){take();state.weather[0]=w;dirty=true;give();}return ok;}
bool app_control(const char *op,const char *page,uint32_t *request){
 take();bool ok=true;
 if(!strcmp(op,"page")){if(!page||(strcmp(page,"overview")&&strcmp(page,"usage")))ok=false;else{state.page=!strcmp(page,"usage")?HOMECRAFT_PAGE_USAGE:HOMECRAFT_PAGE_OVERVIEW;force_full=true;dirty=true;}}
 else if(!strcmp(op,"refresh")){force_full=true;dirty=true;}
 else if(!strcmp(op,"sample")){sample_pending=true;sample_id=++request_id;if(!sample_id)sample_id=++request_id;sample_result=0;dirty=true;}
 else ok=false;
 if(ok){if(strcmp(op,"sample"))++request_id;if(request)*request=request_id;requested_at=0;}
 give();return ok;
}
void app_status_json(char *out,size_t capacity){take();int mv=state.battery_mv;int64_t now=time(NULL),age=last_sample_ms?esp_timer_get_time()/1000-last_sample_ms:0;char t[24]="null",h[24]="null",st[24]="null",sh[24]="null";if(state.sensor_valid){snprintf(t,sizeof(t),"%.1f",state.temperature);snprintf(h,sizeof(h),"%.1f",state.humidity);}if(shown_state.sensor_valid){snprintf(st,sizeof(st),"%.1f",shown_state.temperature);snprintf(sh,sizeof(sh),"%.1f",shown_state.humidity);}
 snprintf(out,capacity,"{\"ok\":true,\"v\":1,\"device\":{\"t\":%s,\"h\":%s,\"at\":%lld,\"age\":%lld,\"page\":\"%s\",\"pages\":%u,\"rotation\":0,\"auto\":false,\"paused\":false,\"busy\":%s,\"partial\":false,\"power\":[%d,%d,%d,-1],\"mv\":%d,\"sd\":[0,0,0,0,0,0],\"history\":0,\"shown\":[%s,%s,%lld,%u,0,false,%u],\"sample\":[%u,%u]},\"refresh\":%u}",t,h,(long long)now,(long long)age,state.page==HOMECRAFT_PAGE_USAGE?"usage":"overview",(1u<<HOMECRAFT_PAGE_OVERVIEW)|(1u<<HOMECRAFT_PAGE_USAGE),state.busy?"true":"false",state.charge>=CHARGE_IDLE,mv>=2500?1:0,state.charge==CHARGE_ACTIVE,mv>=2500?mv:-1,st,sh,(long long)shown_at,shown_state.page,state.frame_sequence,(unsigned)(sample_id?sample_id:1),sample_result,state.frame_sequence);
 give();}
int app_handle_request(const char *input,char *out,size_t capacity){
 const char *end=NULL;cJSON *obj=cJSON_ParseWithOpts(input,&end,true);if(!obj)return 0;
 const cJSON *v=cJSON_GetObjectItemCaseSensitive(obj,"v"),*op=cJSON_GetObjectItemCaseSensitive(obj,"op");bool ok=cJSON_IsNumber(v)&&v->valuedouble==1&&cJSON_IsString(op);const char *name=ok?op->valuestring:"";
 if(ok&&!strcmp(name,"status"))app_status_json(out,capacity);
 else if(ok&&!strcmp(name,"weather")){
  // Optional slot 0/1 for HomeCraft's second city; a clear without slot clears both.
  weather_t w={0};int64_t slot=0;const cJSON *field=cJSON_GetObjectItemCaseSensitive(obj,"slot");bool clear=cJSON_IsFalse(cJSON_GetObjectItemCaseSensitive(obj,"enabled"));
  ok=(!field||integer(field,0,1,&slot))&&(clear||weather_decode(obj,&w));
  if(ok){take();if(clear&&!field)memset(state.weather,0,sizeof(state.weather));else state.weather[slot]=w;dirty=true;give();snprintf(out,capacity,"{\"ok\":true,\"v\":1}");}}
 else if(ok&&!strcmp(name,"todo")){unsigned q;todo_quadrant_t t;ok=todo_decode(obj,&q,&t);if(ok){take();state.todo[q]=t;state.todo_known=true;dirty=true;urgent=true;give();snprintf(out,capacity,"{\"ok\":true,\"v\":1}");}}
 else if(ok&&!strcmp(name,"poem")){poem_t p={0};bool clear=cJSON_IsFalse(cJSON_GetObjectItemCaseSensitive(obj,"enabled"));ok=clear||poem_decode(obj,&p);if(ok){take();state.poem=p;dirty=true;give();snprintf(out,capacity,"{\"ok\":true,\"v\":1}");}}
 else if(ok&&!strcmp(name,"places")){place_t p[PLACES];unsigned n;ok=places_decode(obj,p,&n);if(ok){take();memcpy(state.places,p,sizeof(p));state.place_count=n;dirty=true;give();snprintf(out,capacity,"{\"ok\":true,\"v\":1}");}}
 else if(ok&&!strcmp(name,"totals")){
  // HomeCraft's home totals: [day,tokens], [its Monday,tokens], [its month's first day,tokens]; -1 is unknown.
  static const char *keys[]={"d","wt","mt"};int64_t period[3],tokens[3];
  for(int i=0;ok&&i<3;++i){const cJSON *a=cJSON_GetObjectItemCaseSensitive(obj,keys[i]);ok=cJSON_IsArray(a)&&cJSON_GetArraySize(a)==2&&integer(cJSON_GetArrayItem(a,0),20241230,20991231,&period[i])&&integer(cJSON_GetArrayItem(a,1),-1,9007199254740991LL,&tokens[i]);}
  ok=ok&&usage_week_start((int)period[0])==period[1]&&usage_month_start((int)period[0])==period[2];
  if(ok){take();for(int i=0;i<3;++i){state.hub_periods[i]=(int)period[i];state.hub_tokens[i]=tokens[i];}state.hub_totals=true;dirty=true;give();snprintf(out,capacity,"{\"ok\":true,\"v\":1}");}}
 else if(ok&&!strcmp(name,"history")){int64_t offset=0;ok=integer(cJSON_GetObjectItemCaseSensitive(obj,"offset"),0,88,&offset)&&offset%8==0;if(ok)snprintf(out,capacity,"{\"ok\":true,\"v\":1,\"day\":0,\"revision\":0,\"offset\":%lld,\"points\":[null,null,null,null,null,null,null,null]}",(long long)offset);}
 else if(ok){const cJSON *page=cJSON_GetObjectItemCaseSensitive(obj,"page");uint32_t id=0;ok=app_control(name,cJSON_IsString(page)?page->valuestring:NULL,&id);if(ok)snprintf(out,capacity,"{\"ok\":true,\"v\":1,\"sample\":%u,\"refresh\":%u}",(unsigned)id,(unsigned)id);}
 cJSON_Delete(obj);return ok;
}
static uint8_t crc(const uint8_t *b){uint8_t c=0xFF;for(int i=0;i<2;++i){c^=b[i];for(int j=0;j<8;++j)c=c&128?(c<<1)^0x31:c<<1;}return c;}
static bool sensor_sample(void){if(!sht)return false;xSemaphoreTake(sensor_mutex,portMAX_DELAY);uint8_t cmd=0xFD,b[6];bool ok=i2c_master_transmit(sht,&cmd,1,100)==ESP_OK;if(ok){vTaskDelay(pdMS_TO_TICKS(12));ok=i2c_master_receive(sht,b,6,100)==ESP_OK&&crc(b)==b[2]&&crc(b+3)==b[5];}xSemaphoreGive(sensor_mutex);if(!ok)return false;
 take();state.temperature=-45+175.0f*((b[0]<<8)|b[1])/65535.0f;state.humidity=fminf(100,fmaxf(0,-6+125.0f*((b[3]<<8)|b[4])/65535.0f));state.sensor_valid=true;last_sample_ms=esp_timer_get_time()/1000;give();return true;}
// A few mV of noise would move the estimated percent and pull the header into every redraw:
// the shown voltage follows only a change of 20 mV (about 2%) or a reading appearing/disappearing.
static void battery_update(void){int mv=battery_read();take();int old=state.battery_mv;if(mv<0||old<0||abs(mv-old)>=20){if(battery_level(mv)!=battery_level(old))dirty=true;state.battery_mv=mv;}give();}
// SY6974B REG08 only (VBUS_STAT 7:5, CHRG_STAT 4:3); nothing is written, so its charge settings stay as they are.
// A board without it (pre-V1.2 ETA6003) never answers and stays unknown.
static int charger_read(void){if(!charger)return CHARGE_UNKNOWN;uint8_t reg=0x08,v;if(i2c_master_transmit_receive(charger,&reg,1,&v,1,100)!=ESP_OK)return CHARGE_UNKNOWN;
 int bus=v>>5,stat=(v>>3)&3;if(stat==1||stat==2)return CHARGE_ACTIVE;if(bus==0||bus==7)return CHARGE_NO_INPUT;return stat==3?CHARGE_DONE:CHARGE_IDLE;}
// Only charging and charged are drawn. Plugging in or out is something the user just did, so it shows within 30 s.
static void charger_update(void){int now=charger_read();take();int old=state.charge;state.charge=now;if((old>=CHARGE_ACTIVE?old:0)!=(now>=CHARGE_ACTIVE?now:0)){dirty=true;urgent=true;}give();
 if(old!=now)ESP_LOGI("app","charger %d -> %d",old,now);}
static void sensor_task(void *arg){(void)arg;for(unsigned n=1;;++n){vTaskDelay(pdMS_TO_TICKS(30000));sensor_sample();charger_update();if(n%2==0)battery_update();}}
static void sensors_init(void){i2c_master_bus_config_t bus={.i2c_port=0,.sda_io_num=E1002_I2C_SDA,.scl_io_num=E1002_I2C_SCL,.clk_source=I2C_CLK_SRC_DEFAULT,.glitch_ignore_cnt=7,.flags.enable_internal_pullup=true};i2c_master_bus_handle_t handle;if(i2c_new_master_bus(&bus,&handle)!=ESP_OK)return;i2c_device_config_t device={.dev_addr_length=I2C_ADDR_BIT_LEN_7,.device_address=0x44,.scl_speed_hz=100000};if(i2c_master_bus_add_device(handle,&device,&sht)!=ESP_OK)sht=NULL;
 i2c_master_bus_config_t power_bus={.i2c_port=1,.sda_io_num=E1002_CHARGER_I2C_SDA,.scl_io_num=E1002_CHARGER_I2C_SCL,.clk_source=I2C_CLK_SRC_DEFAULT,.glitch_ignore_cnt=7,.flags.enable_internal_pullup=true};
 i2c_device_config_t pmic={.dev_addr_length=I2C_ADDR_BIT_LEN_7,.device_address=0x6B,.scl_speed_hz=100000};
 if(i2c_new_master_bus(&power_bus,&handle)!=ESP_OK||i2c_master_bus_add_device(handle,&pmic,&charger)!=ESP_OK)charger=NULL;}
static void display_task(void *arg){
 (void)arg;uint8_t *frame=heap_caps_malloc(FRAME_BYTES,MALLOC_CAP_SPIRAM|MALLOC_CAP_8BIT),*shown=heap_caps_malloc(FRAME_BYTES,MALLOC_CAP_SPIRAM|MALLOC_CAP_8BIT);if(!frame||!shown){ESP_LOGE("display","frame allocation failed");vTaskDelete(NULL);return;}
 memset(shown,0xFF,FRAME_BYTES);color_canvas_t canvas={frame};int64_t last=0;
 // Every redraw flashes the whole panel for about 27 s, so routine changes wait 10 min; first data (10 s),
 // urgent ones (30 s), the keys and HomeCraft requests come sooner.
 for(;;){int64_t now=esp_timer_get_time()/1000;take();bool automatic=now-requested_at>=2000&&(now-last>=600000||(initial_data&&now-last>=10000)||(urgent&&now-last>=30000));bool sampling=sample_pending,go=dirty&&!state.busy&&(force_full||sampling||(automatic&&!state.privacy));bool forced=force_full;dashboard_state_t snapshot=state;uint32_t target_sample=sample_id;if(go){dirty=false;force_full=false;initial_data=false;urgent=false;state.busy=true;}give();
  if(go){if(sampling||now-last_sample_ms>=30000)sensor_sample();app_snapshot(&snapshot);
   // Sensor noise alone never redraws: the indoor values, and each place HomeCraft reports, keep their shown values
   // until they move 0.3 °C or 2 %RH, and take the latest reading whenever the screen is redrawn anyway or a sample
   // was requested.
   dashboard_state_t fresh=snapshot;bool hold=false;
   if(!forced&&!sampling){
    if(shown_state.sensor_valid&&snapshot.sensor_valid&&fabsf(snapshot.temperature-shown_state.temperature)<0.3f&&fabsf(snapshot.humidity-shown_state.humidity)<2){snapshot.temperature=shown_state.temperature;snapshot.humidity=shown_state.humidity;hold=true;}
    for(unsigned i=0;i<snapshot.place_count&&i<shown_state.place_count;++i){place_t *p=&snapshot.places[i];const place_t *o=&shown_state.places[i];
     if(p->known&&o->known&&!strcmp(p->name,o->name)&&abs(p->temperature_tenths-o->temperature_tenths)<3&&abs(p->humidity_tenths-o->humidity_tenths)<20){p->temperature_tenths=o->temperature_tenths;p->humidity_tenths=o->humidity_tenths;hold=true;}}}
   dashboard_render(&canvas,&snapshot);bool changed=memcmp(frame,shown,FRAME_BYTES)!=0;
   if(changed&&hold){snapshot=fresh;dashboard_render(&canvas,&snapshot);}
   // Always the whole screen: on this Spectra 6 panel a partial window takes as long as a full refresh, and each
   // partial pass left the pixels outside it drifting (whites toward cyan, black text fading) until the next full one.
   if(changed||forced){uint32_t elapsed=0;esp_err_t e=e1002_display_present(frame,NULL,&elapsed);
    take();state.busy=false;last_ms=elapsed;if(e==ESP_OK){memcpy(shown,frame,FRAME_BYTES);shown_state=snapshot;shown_at=snapshot.now;++state.frame_sequence;display_ready=true;if(sample_id==target_sample){sample_result=1;sample_pending=false;}}else{force_full=true;dirty=true;if(sample_id==target_sample){sample_result=4;sample_pending=false;}}give();
    ESP_LOGI("frame","result=%s mode=full region=0,0,800,480 ms=%lu frame=%u",esp_err_to_name(e),(unsigned long)elapsed,state.frame_sequence);last=now;
   }else{take();state.busy=false;if(sample_id==target_sample){sample_result=2;sample_pending=false;}give();}
  }
  vTaskDelay(pdMS_TO_TICKS(100));
 }
}
// Privacy shows only the picture; data keeps arriving but is drawn again only after leaving it.
static void privacy_toggle(void){take();state.privacy=!state.privacy;force_full=true;dirty=true;give();}
// Physical order left, middle, right(green): long-press left pairs, middle pages; right: one click redraws the
// whole screen (as the factory firmware), a double click toggles privacy.
static void keys_task(void *arg){(void)arg;enum {DOUBLE_MS=450};int pins[]={E1002_KEY2,E1002_KEY1,E1002_KEY0};bool down[3]={0};int64_t since[3]={0},click=0;for(;;){int64_t now=esp_timer_get_time()/1000;uint32_t id;
 // A single click waits out the double-click window before refreshing.
 if(click&&now-click>DOUBLE_MS){click=0;app_control("refresh",NULL,&id);}
 for(int i=0;i<3;++i){bool pressed=!gpio_get_level(pins[i]);if(pressed&&!down[i]){since[i]=now;down[i]=true;}if(pressed&&down[i]&&i==0&&since[i]&&now-since[i]>=2000){app_pair_request();since[i]=0;}if(!pressed&&down[i]){if(since[i]&&now-since[i]>=40){if(i==1){take();bool details=state.page==HOMECRAFT_PAGE_USAGE;give();app_control("page",details?"overview":"usage",&id);}if(i==2){if(click){click=0;privacy_toggle();}else click=now;}}down[i]=false;}}
 vTaskDelay(pdMS_TO_TICKS(20));}}
void app_main(void){
 mutex=xSemaphoreCreateMutex();sensor_mutex=xSemaphoreCreateMutex();for(int i=0;i<PROVIDER_COUNT;++i)usage_record_init(&state.usage[i]);state.battery_mv=-1;state.charge=CHARGE_UNKNOWN;
 esp_err_t e=nvs_flash_init();if(e!=ESP_OK){ESP_LOGE("app","NVS init failed; original storage not erased:%s",esp_err_to_name(e));return;}
 ESP_ERROR_CHECK(esp_event_loop_create_default());sensors_init();sensor_sample();charger_update();battery_init();battery_update();
 gpio_config_t keys={.pin_bit_mask=(1ULL<<E1002_KEY0)|(1ULL<<E1002_KEY1)|(1ULL<<E1002_KEY2),.mode=GPIO_MODE_INPUT,.pull_up_en=GPIO_PULLUP_ENABLE};ESP_ERROR_CHECK(gpio_config(&keys));
 ESP_ERROR_CHECK(e1002_display_init());ESP_ERROR_CHECK(ble_link_init());
 ESP_LOGI("app","HomeCraft E1002 0.3.0;PSRAM=%u;free=%u;battery_mv=%d",(unsigned)esp_psram_get_size(),(unsigned)esp_get_free_heap_size(),state.battery_mv);
 xTaskCreate(display_task,"display",16384,NULL,2,NULL);xTaskCreate(keys_task,"keys",4096,NULL,3,NULL);xTaskCreate(sensor_task,"sensor",4096,NULL,2,NULL);
}
