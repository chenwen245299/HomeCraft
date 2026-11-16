#include "device_protocol.h"
#include "homecraft_wire.h"
#include "cJSON.h"
#include <math.h>
#include <stdio.h>
#include <string.h>

static const char *const pages[]=HOMECRAFT_PAGE_NAMES;
const char *device_page_name(unsigned page) { return page<HOMECRAFT_PAGE_COUNT?pages[page]:"overview"; }
bool device_command_decode(const char *text, device_command_t *out)
{
    if (!text || !out || strlen(text)>128) return false;
    const char *end=NULL;
    cJSON *root=cJSON_ParseWithOpts(text,&end,true);
    if(!root || !cJSON_IsObject(root)){cJSON_Delete(root);return false;}
    cJSON *v=cJSON_GetObjectItemCaseSensitive(root,"v"), *op=cJSON_GetObjectItemCaseSensitive(root,"op");
    bool ok=cJSON_IsNumber(v) && v->valuedouble==1 && cJSON_IsString(op);
    device_command_t command={0};
    const char *argument=NULL;
    if(ok) {
        if(!strcmp(op->valuestring,"status"))command.operation=DEVICE_STATUS;
        else if(!strcmp(op->valuestring,"history")){command.operation=DEVICE_HISTORY;argument="offset";}
        else if(!strcmp(op->valuestring,"harness")){command.operation=DEVICE_HARNESS;argument="offset";}
        else if(!strcmp(op->valuestring,"page")){command.operation=DEVICE_PAGE;argument="page";}
        else if(!strcmp(op->valuestring,"rotate")){command.operation=DEVICE_ROTATE;argument="rotation";}
        else if(!strcmp(op->valuestring,"refresh"))command.operation=DEVICE_REFRESH;
        else if(!strcmp(op->valuestring,"sample"))command.operation=DEVICE_SAMPLE;
        else ok=false;
    }
    unsigned seen=0;
    for(cJSON *item=root->child;item;item=item->next) {
        unsigned bit=item->string&&!strcmp(item->string,"v")?1:(item->string&&!strcmp(item->string,"op")?2:(argument&&item->string&&!strcmp(item->string,argument)?4:0));
        if(!bit || (seen&bit))ok=false;
        seen|=bit;
    }
    if(ok && argument) {
        cJSON *value=cJSON_GetObjectItemCaseSensitive(root,argument);
        if(command.operation==DEVICE_PAGE) {
            ok=false;
            if(cJSON_IsString(value))for(unsigned i=0;i<HOMECRAFT_PAGE_COUNT;++i)if(!strcmp(value->valuestring,pages[i])){command.value=i;ok=true;break;}
        } else if(command.operation==DEVICE_ROTATE && cJSON_IsString(value) && !strcmp(value->valuestring,"auto"))command.value=4;
        else {
            ok=cJSON_IsNumber(value) && isfinite(value->valuedouble) && value->valuedouble>=0 && value->valuedouble<(command.operation==DEVICE_HISTORY?96:(command.operation==DEVICE_HARNESS?8:4)) && floor(value->valuedouble)==value->valuedouble;
            if(ok)command.value=(unsigned)value->valuedouble;
        }
    }
    cJSON_Delete(root);
    if(ok)*out=command;
    return ok;
}
bool device_status_encode(const device_snapshot_t *s, const environment_history_t *h, char out[481])
{
    if(!s || !h || !out)return false;
    char temperature[24]="null",humidity[24]="null";
    bool valid=s->sensor_valid && isfinite(s->temperature) && isfinite(s->humidity) && s->temperature>=-45 && s->temperature<=130 && s->humidity>=0 && s->humidity<=100;
    if(valid){snprintf(temperature,sizeof(temperature),"%.2f",(double)s->temperature);snprintf(humidity,sizeof(humidity),"%.2f",(double)s->humidity);}
    int percent=s->power.valid && s->power.battery_present && s->power.percent>=0 && s->power.percent<=100?s->power.percent:-1;
    int n=snprintf(out,481,"{\"ok\":true,\"v\":1,\"device\":{\"t\":%s,\"h\":%s,\"at\":%lld,\"age\":%llu,\"page\":\"%s\",\"pages\":%u,\"rotation\":%u,\"auto\":%s,\"paused\":%s,\"busy\":%s,\"partial\":%s,\"power\":[%d,%d,%d,%d],\"sd\":[%d,%d,%u,%d,%lld,%d],\"history\":%u}}",
        temperature,humidity,(long long)s->sampled_at,(unsigned long long)(s->uptime_ms>=s->sampled_ms?s->uptime_ms-s->sampled_ms:0),device_page_name(s->page),s->pages,s->rotation,
        s->auto_rotate?"true":"false",s->paused?"true":"false",s->busy?"true":"false",s->partial?"true":"false",
        s->power.usb_present,s->power.battery_present,s->power.charging,percent,
        h->mounted,h->day,h->stored_samples,h->write_ok,(long long)h->last_saved_at,h->last_error,h->revision);
    if(n<=0 || n>478)return false;
    char shown_t[24]="null",shown_h[24]="null";
    if(s->frame.sequence && s->frame.sensor_valid && isfinite(s->frame.temperature) && isfinite(s->frame.humidity)) {
        snprintf(shown_t,sizeof(shown_t),"%.2f",(double)s->frame.temperature);
        snprintf(shown_h,sizeof(shown_h),"%.2f",(double)s->frame.humidity);
    }
    n-=2;
    n+=snprintf(out+n,481-(size_t)n,",\"shown\":[%s,%s,%lld,%u,%u,%s,%u]",shown_t,shown_h,(long long)s->frame.at,s->frame.page,s->frame.rotation,s->frame.auto_rotate?"true":"false",s->frame.sequence);
    if(n<0 || n>=479)return false;
    if(s->sample_id)n+=snprintf(out+n,481-(size_t)n,",\"sample\":[%lu,%u]",(unsigned long)s->sample_id,s->sample_result);
    if(n<0 || n>=479)return false;
    n+=snprintf(out+n,481-(size_t)n,"}}");
    return n>0 && n<=480;
}
bool device_history_encode(const environment_history_t *h, unsigned offset, char out[481])
{
    if(!h || !out || offset>=96)return false;
    int n=snprintf(out,481,"{\"ok\":true,\"v\":1,\"day\":%d,\"revision\":%u,\"offset\":%u,\"points\":[",h->day,h->revision,offset);
    for(unsigned i=offset;i<offset+DEVICE_HISTORY_CHUNK && i<96;++i) {
        if(n<0 || n>=450)return false;
        n+=h->valid[i]?snprintf(out+n,481-(size_t)n,"%s[%d,%u]",i==offset?"":",",h->temperature[i],h->humidity[i]):snprintf(out+n,481-(size_t)n,"%snull",i==offset?"":",");
    }
    if(n<0 || n>=478)return false;
    n+=snprintf(out+n,481-(size_t)n,"]}");return n>0 && n<=480;
}
