#include "ble_link.h"
#include "app_state.h"
#include "protocomm.h"
#include "protocomm_ble.h"
#include "protocomm_security2.h"
#include "esp_srp.h"
#include "nvs.h"
#include "esp_mac.h"
#include "esp_random.h"
#include "bootloader_random.h"
#include "esp_timer.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
static protocomm_t *pc;
static protocomm_security2_params_t security;
static char secret[33],device_name[20],pin[9];
static bool pair_mode,pair_claimed;
static volatile bool open_request;
static int64_t expiry,restore_at;
static unsigned failures;
static portMUX_TYPE guard=portMUX_INITIALIZER_UNLOCKED;
static int reply(const char *text,uint8_t **out,ssize_t *len){size_t n=strlen(text);*out=malloc(n);if(!*out)return ESP_ERR_NO_MEM;memcpy(*out,text,n);*len=n;return ESP_OK;}
static esp_err_t endpoint(uint32_t session,const uint8_t *in,ssize_t n,uint8_t **out,ssize_t *len,void *ctx){
 (void)session;(void)ctx;
 if(n<1||n>480||memchr(in,0,n))return ESP_ERR_INVALID_ARG;
 char input[481];memcpy(input,in,n);input[n]=0;
 if(pair_mode){
  int64_t now=esp_timer_get_time()/1000;
  portENTER_CRITICAL(&guard);
  bool allowed=!pair_claimed&&now<expiry&&n==3&&!memcmp(in,"get",3);
  if(allowed){pair_claimed=true;restore_at=now+5000;}
  portEXIT_CRITICAL(&guard);
  if(!allowed)return ESP_ERR_INVALID_STATE;
  char data[320];snprintf(data,sizeof(data),"{\"name\":\"%s\",\"username\":\"wifiprov\",\"password\":\"%s\",\"service_uuid\":\"1775244d-6b43-439b-877c-060f2d9bed07\",\"model\":\"reTerminalE1002\"}",device_name,secret);
  int e=reply(data,out,len);memset(data,0,sizeof(data));return e;
 }
 if(n==1&&in[0]=='?')return reply("{\"ok\":true,\"daily\":true,\"profiles\":true,\"weekly_tokens\":true,\"monthly_tokens\":true,\"yesterday_usage\":true,\"harness_aggregate\":1,\"display_snapshots\":1,\"device_control\":1,\"sample_refresh\":1,\"rotation_control\":0,\"environment_history\":0,\"weather\":2,\"forecast\":1,\"totals\":1,\"places\":1,\"todo\":1,\"poem\":1,\"color\":6,\"model\":\"reTerminalE1002\",\"refresh\":0}",out,len);
 if(strstr(input,"\"op\"")){
  // JSON and operation allowlist are validated in the application, never arbitrary file access.
  extern int app_handle_request(const char *,char *,size_t);
  char response[481];if(!app_handle_request(input,response,sizeof(response)))return ESP_ERR_INVALID_ARG;
  return reply(response,out,len);
 }
 bool ok=app_accept_usage(input);
 return reply(ok?"{\"ok\":true,\"refresh\":0}":"{\"ok\":false}",out,len);
}
static void events(void *ctx,esp_event_base_t base,int32_t id,void *data){
 (void)ctx;(void)data;
 if(base==PROTOCOMM_TRANSPORT_BLE_EVENT&&id==PROTOCOMM_TRANSPORT_BLE_DISCONNECTED)app_set_connected(false);
 if(base==PROTOCOMM_SECURITY_SESSION_EVENT&&id==PROTOCOMM_SECURITY_SESSION_SETUP_OK)app_set_connected(!pair_mode);
 if(base==PROTOCOMM_SECURITY_SESSION_EVENT&&id!=PROTOCOMM_SECURITY_SESSION_SETUP_OK){
  portENTER_CRITICAL(&guard);if(++failures>=5&&pair_mode)expiry=0;portEXIT_CRITICAL(&guard);
 }
}
static esp_err_t start(bool enrolling){
 if(pc){ESP_ERROR_CHECK_WITHOUT_ABORT(protocomm_ble_stop(pc));protocomm_delete(pc);pc=NULL;}
 free((void *)security.salt);free((void *)security.verifier);memset(&security,0,sizeof(security));
 pair_mode=enrolling;pair_claimed=false;failures=0;
 char *salt=NULL,*verifier=NULL;int verifier_len=0;
 const char *pass=enrolling?pin:secret;
 // The BLE stack (and RF) is fully stopped here, so the salt needs the bootloader entropy source.
 bootloader_random_enable();
 esp_err_t err=esp_srp_gen_salt_verifier("wifiprov",8,pass,strlen(pass),&salt,16,&verifier,&verifier_len);
 bootloader_random_disable();
 if(err!=ESP_OK)return err;
 security=(protocomm_security2_params_t){.salt=salt,.salt_len=16,.verifier=verifier,.verifier_len=verifier_len};
 pc=protocomm_new();if(!pc)return ESP_ERR_NO_MEM;
 protocomm_ble_name_uuid_t names[]={{"prov-session",0xFF51},{"proto-ver",0xFF52},{enrolling?"pair-device":"usage-data",0xFF53}};
 protocomm_ble_config_t config={.service_uuid={0x07,0xED,0x9B,0x2D,0x0F,0x06,0x7C,0x87,0x9B,0x43,0x43,0x6B,0x4D,0x24,0x75,0x17},.nu_lookup_count=3,.nu_lookup=names,.keep_ble_on=0};
 // keep_ble_on=0: protocomm_ble_stop deinitialises NimBLE so the next mode can start (IDF keeps its
 // internal state with keep_ble_on=1 and then refuses a restart as "already started").
 if(enrolling)config.service_uuid[0]=0x08;
 snprintf(config.device_name,sizeof(config.device_name),"%s_%s",enrolling?"EPair":"EPaper",device_name+7);
 err=protocomm_ble_start(pc,&config);if(err!=ESP_OK)return err;
 err=protocomm_set_version(pc,"proto-ver","{\"prov\":{\"ver\":\"v1.1\",\"sec_ver\":2,\"sec_patch_ver\":1}}");if(err!=ESP_OK)return err;
 err=protocomm_set_security(pc,"prov-session",&protocomm_security2,&security);if(err!=ESP_OK)return err;
 return protocomm_add_endpoint(pc,enrolling?"pair-device":"usage-data",endpoint,NULL);
}
static void task(void *arg){
 (void)arg;
 for(;;){
  int64_t now=esp_timer_get_time()/1000;
  if(open_request){
   open_request=false;uint32_t value;do{esp_fill_random(&value,sizeof(value));}while(value>=4200000000U);
   snprintf(pin,sizeof(pin),"%08lu",(unsigned long)(value%100000000));
   expiry=now+600000;restore_at=0;
   app_set_pairing(true,pin,device_name);
   // A physical request is required. The code is displayed, never sent to serial.
   esp_err_t e=start(true);if(e!=ESP_OK)ESP_LOGE("ble","pairing start failed:%s",esp_err_to_name(e));
  }
  if(pair_mode&&((pair_claimed&&now>=restore_at)||(!pair_claimed&&now>=expiry))){
   app_set_pairing(false,NULL,NULL);memset(pin,0,sizeof(pin));
   esp_err_t e=start(false);if(e!=ESP_OK)ESP_LOGE("ble","data start failed:%s",esp_err_to_name(e));
  }
  vTaskDelay(pdMS_TO_TICKS(100));
 }
}
void ble_link_open_pairing(void){open_request=true;}
esp_err_t ble_link_init(void){
 uint8_t mac[6];ESP_ERROR_CHECK(esp_read_mac(mac,ESP_MAC_BT));snprintf(device_name,sizeof(device_name),"EPaper_%02X%02X%02X",mac[3],mac[4],mac[5]);
 nvs_handle_t nvs;esp_err_t e=nvs_open("e1002_hc",NVS_READWRITE,&nvs);if(e!=ESP_OK)return e;
 size_t n=sizeof(secret);e=nvs_get_str(nvs,"ble_key",secret,&n);
 if(e==ESP_ERR_NVS_NOT_FOUND){uint8_t bytes[16];bootloader_random_enable();esp_fill_random(bytes,sizeof(bytes));bootloader_random_disable();for(unsigned i=0;i<16;++i)snprintf(secret+i*2,3,"%02x",bytes[i]);memset(bytes,0,sizeof(bytes));e=nvs_set_str(nvs,"ble_key",secret);if(e==ESP_OK)e=nvs_commit(nvs);}
 nvs_close(nvs);if(e!=ESP_OK||strlen(secret)!=32)return e==ESP_OK?ESP_ERR_INVALID_STATE:e;
 ESP_ERROR_CHECK(esp_event_handler_register(PROTOCOMM_TRANSPORT_BLE_EVENT,ESP_EVENT_ANY_ID,events,NULL));
 ESP_ERROR_CHECK(esp_event_handler_register(PROTOCOMM_SECURITY_SESSION_EVENT,ESP_EVENT_ANY_ID,events,NULL));
 e=start(false);if(e!=ESP_OK)return e;
 xTaskCreate(task,"ble_mode",8192,NULL,4,NULL);return ESP_OK;
}
