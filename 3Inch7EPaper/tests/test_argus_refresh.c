#include "dashboard.h"
#include "refresh_policy.h"
#include <assert.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
static uint8_t a[SCREEN_BYTES],b[SCREEN_BYTES];
int main(void) {
    setenv("TZ","SGT-8",1);tzset();time_t now=time(NULL);struct tm tm;localtime_r(&now,&tm);
    int day=(tm.tm_year+1900)*10000+(tm.tm_mon+1)*100+tm.tm_mday;
    for(unsigned rotation=0;rotation<360;rotation+=90)for(unsigned change=0;change<4;++change){
        dashboard_state_t old={.page=PAGE_ARGUS};old.network.started=old.network.data_mode=old.network.ble_link=true;
        for(unsigned i=0;i<PROVIDER_COUNT;++i)usage_record_init(&old.network.providers[i]);
        usage_record_t *r=&old.network.providers[PROVIDER_ARGUS];r->configured=r->has_data=r->usage_available=true;r->status=USAGE_OK;r->fetched_at=now;r->token_day=day;r->daily_tokens=120;r->input_tokens=100;r->output_tokens=20;r->cached_tokens=40;r->calls=3;
        usage_breakdown_t *d=&old.network.argus_breakdown;*d=(usage_breakdown_t){.received=true,.day=day,.fetched_at=now,.total=120,.model_count=2,.provider_count=1,.model_rows=2,.provider_rows=1};
        strcpy(d->models[0].label,"model-a");d->models[0].tokens=100;strcpy(d->models[1].label,"model-b");d->models[1].tokens=20;strcpy(d->providers[0].label,"ID abcdef12");d->providers[0].tokens=120;
        dashboard_state_t next=old;
        if(change==0){next.network.argus_breakdown.models[0].tokens=90;next.network.argus_breakdown.models[1].tokens=30;}
        if(change==1)strcpy(next.network.argus_breakdown.models[0].label,"new-model");
        if(change==2)next.network.providers[PROVIDER_ARGUS].cached_tokens=60;
        if(change==3)next.network.argus_breakdown.received=false;
        canvas_t ca={.pixels=a,.rotation=rotation},cb={.pixels=b,.rotation=rotation};dashboard_render(&ca,&old);dashboard_render(&cb,&next);assert(memcmp(a,b,sizeof(a)));
        uint32_t key=dashboard_layout_key(&old,&ca);assert(key==dashboard_layout_key(&next,&cb));refresh_policy_t p={.enabled=true};refresh_commit(&p,REFRESH_FULL,true,key,1000000);assert(refresh_decide(&p,true,false,key,61000000)==REFRESH_DIFFERENTIAL);
    }
    puts("16 Argus classification/cache/missing updates retain differential refresh: OK");
}
