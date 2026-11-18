#include "dashboard.h"
#include <assert.h>
#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#include <time.h>
int main(int argc,char **argv)
{
    setenv("TZ","SGT-8",1);tzset();time_t now=time(NULL);struct tm tm;localtime_r(&now,&tm);
    int day=(tm.tm_year+1900)*10000+(tm.tm_mon+1)*100+tm.tm_mday;
    unsigned char guarded[SCREEN_BYTES+64];
    for(unsigned count=1;count<=8;++count)for(unsigned rotation=0;rotation<360;rotation+=90) {
        dashboard_state_t s={.page=PAGE_EXTRA1,.auto_rotate=true};s.network.started=true;s.network.harness.version=2;
        for(unsigned i=0;i<PROVIDER_COUNT;++i)usage_record_init(&s.network.providers[i]);
        for(unsigned i=0;i<count;++i) {
            usage_record_t r;usage_record_init(&r);strcpy(r.name,"DeepSeek Harness");snprintf(r.source_id,sizeof(r.source_id),"%016x",i+1);snprintf(r.source_name,sizeof(r.source_name),"Mac-%u",i+1);
            r.status=USAGE_OK;r.has_data=r.usage_available=true;r.fetched_at=now;r.token_day=day;r.token_week=usage_week_start(day);r.daily_tokens=r.input_tokens=9548778+i*10000;r.weekly_tokens=r.daily_tokens+100000;r.token_month=usage_month_start(day);r.monthly_tokens=r.daily_tokens+10000000;
            bool changed;assert(harness_stats_update(&s.network.harness,&r,&changed));
        }
        usage_record_t shared;usage_record_init(&shared);strcpy(shared.name,"DeepSeek Harness");
        shared.status=USAGE_OK;shared.has_data=shared.usage_available=true;shared.fetched_at=now;
        shared.token_day=day;shared.token_week=usage_week_start(day);shared.token_month=usage_month_start(day);
        shared.daily_tokens=shared.input_tokens=9548778;shared.weekly_tokens=19639050;shared.monthly_tokens=30000000;
        bool changed;assert(harness_stats_update(&s.network.harness,&shared,&changed));
        harness_stats_total(&s.network.harness,day,&s.network.providers[4]);
        memset(guarded,0xab,sizeof(guarded));canvas_t canvas={.pixels=guarded,.rotation=rotation};dashboard_render(&canvas,&s);
        for(unsigned i=SCREEN_BYTES;i<sizeof(guarded);++i)assert(guarded[i]==0xab);
        unsigned requested_count=argc>2?(unsigned)atoi(argv[2]):2,requested_rotation=argc>3?(unsigned)atoi(argv[3]):0;
        if(argc>=2 && count==requested_count && rotation==requested_rotation) {FILE *f=fopen(argv[1],"wb");assert(f);fprintf(f,"P4\n800 480\n");for(unsigned i=0;i<SCREEN_BYTES;++i)fputc(~guarded[i]&0xff,f);fclose(f);}
    }
    puts("Harness rendering: 1-8 devices, four orientations, guarded framebuffer OK");
}
