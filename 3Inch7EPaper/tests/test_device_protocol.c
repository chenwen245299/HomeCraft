#include "device_protocol.h"
#include "cJSON.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>
int main(void)
{
    device_command_t music_command;
    assert(!strcmp(device_page_name(12),"music"));
    assert(device_command_decode("{\"v\":1,\"op\":\"page\",\"page\":\"music\"}",&music_command));
    assert(music_command.operation==DEVICE_PAGE&&music_command.value==12);
    device_command_t c;
    const char *bad[]={"{}","[]","{\"v\":1,\"op\":\"shutdown\"}","{\"v\":1,\"op\":\"status\",\"password\":true}","{\"v\":1,\"v\":1,\"op\":\"status\"}","{\"v\":2,\"op\":\"status\"}","{\"v\":1,\"op\":\"history\",\"offset\":96}","{\"v\":1,\"op\":\"history\",\"offset\":-1}","{\"v\":1,\"op\":\"history\",\"offset\":1.5}","{\"v\":1,\"op\":\"rotate\",\"rotation\":4}","{\"v\":1,\"op\":\"page\",\"page\":\"unknown\"}","{\"v\":1,\"op\":\"status\"}trailing"};
    for(unsigned i=0;i<sizeof(bad)/sizeof(bad[0]);++i)assert(!device_command_decode(bad[i],&c));
    assert(device_command_decode("{\"v\":1,\"op\":\"status\"}",&c) && c.operation==DEVICE_STATUS);
    assert(device_command_decode("{\"v\":1,\"op\":\"sample\"}",&c) && c.operation==DEVICE_SAMPLE);
    assert(!device_command_decode("{\"v\":1,\"op\":\"sample\",\"id\":1}",&c));
    assert(device_command_decode("{\"v\":1,\"op\":\"page\",\"page\":\"environment\"}",&c) && c.value==4);
    assert(device_command_decode("{\"v\":1,\"op\":\"rotate\",\"rotation\":\"auto\"}",&c) && c.value==4);
    assert(device_command_decode("{\"v\":1,\"op\":\"rotate\",\"rotation\":3}",&c) && c.value==3);
    assert(device_command_decode("{\"v\":1,\"op\":\"history\",\"offset\":95}",&c));
    environment_history_t h={.mounted=true,.day=20261021,.revision=12,.stored_samples=123,.last_saved_at=1792560000,.write_ok=true};
    device_snapshot_t s={.sensor_valid=true,.temperature=31.23,.humidity=43.45,.sampled_at=1792560000,.sampled_ms=5000,.uptime_ms=6000,.pages=4095,.auto_rotate=true,.partial=true,.power={.valid=true,.usb_present=true,.battery_present=true,.percent=99}};
    char reply[481];assert(device_status_encode(&s,&h,reply) && strlen(reply)<=480);
    cJSON *json=cJSON_Parse(reply);assert(json);
    cJSON *d=cJSON_GetObjectItem(json,"device");
    assert(cJSON_GetObjectItem(d,"t")->valuedouble==31.23);cJSON_Delete(json);
    s.frame=(device_frame_t){.sensor_valid=true,.temperature=31.1,.humidity=44,.at=1792560000,.page=4,.rotation=1,.sequence=2};
    assert(device_status_encode(&s,&h,reply));json=cJSON_Parse(reply);
    cJSON *shown=cJSON_GetObjectItem(cJSON_GetObjectItem(json,"device"),"shown");
    assert(cJSON_GetArraySize(shown)==7 && cJSON_GetArrayItem(shown,0)->valuedouble==31.1 && cJSON_GetArrayItem(shown,6)->valueint==2);cJSON_Delete(json);
    s.sample_id=UINT32_MAX;s.sample_result=2;
    assert(device_status_encode(&s,&h,reply));json=cJSON_Parse(reply);
    cJSON *sample=cJSON_GetObjectItem(cJSON_GetObjectItem(json,"device"),"sample");
    assert(cJSON_GetArraySize(sample)==2 && cJSON_GetArrayItem(sample,0)->valuedouble==UINT32_MAX && cJSON_GetArrayItem(sample,1)->valueint==2);cJSON_Delete(json);
    s.uptime_ms=UINT64_C(31536000000);s.sampled_ms=1;s.sampled_at=4102444800;
    s.frame.at=4102444800;s.frame.sequence=UINT32_MAX;h.stored_samples=UINT32_MAX;h.revision=UINT32_MAX;
    assert(device_status_encode(&s,&h,reply) && strlen(reply)<=480);
    s.temperature=NAN;assert(device_status_encode(&s,&h,reply));
    json=cJSON_Parse(reply);assert(cJSON_IsNull(cJSON_GetObjectItem(cJSON_GetObjectItem(json,"device"),"t")));cJSON_Delete(json);
    for(unsigned i=0;i<96;++i){h.valid[i]=i%7!=0;h.temperature[i]=-4500+(int)i*100;h.humidity[i]=10000;}
    for(unsigned i=0;i<96;++i){assert(device_history_encode(&h,i,reply) && strlen(reply)<=480);json=cJSON_Parse(reply);assert(json);assert(cJSON_GetArraySize(cJSON_GetObjectItem(json,"points"))==(i<=88?8:96-(int)i));cJSON_Delete(json);}
    assert(!device_history_encode(&h,96,reply));
    puts("Device protocol: strict commands, missing sensor, power and 96 history boundaries passed");
}
