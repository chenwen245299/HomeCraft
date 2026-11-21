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
        dashboard_state_t s={.page=PAGE_EXTRA1,.auto_rotate=true};s.network.started=true;
        for(unsigned i=0;i<PROVIDER_COUNT;++i)usage_record_init(&s.network.providers[i]);
        usage_record_t shared;usage_record_init(&shared);strcpy(shared.name,"DeepSeek Harness");
        shared.status=USAGE_OK;shared.configured=shared.has_data=shared.usage_available=true;shared.fetched_at=now;
        shared.token_day=day;shared.token_week=usage_week_start(day);shared.token_month=usage_month_start(day);
        shared.daily_tokens=shared.input_tokens=count==2?0:9548778;shared.weekly_tokens=19639050;shared.monthly_tokens=30000000;
        if(count==3){shared.token_day=usage_previous_day(day);}
        if(count==4){shared.daily_tokens=-1;shared.usage_available=false;}
        if(count==5){shared.weekly_tokens=-1;shared.monthly_tokens=-1;}
        if(count==6){usage_record_init(&shared);strcpy(shared.name,"DeepSeek Harness");shared.configured=true;shared.status=USAGE_BAD_RESPONSE;}
        if(count==7){shared.daily_tokens=shared.input_tokens=shared.weekly_tokens=shared.monthly_tokens=9007199254740991LL;}
        if(count==8){shared.output_tokens=45688;shared.input_tokens=9503090;shared.calls=102;}
        assert(usage_display_update(&s.network.providers[4],&shared));
        memset(guarded,0xab,sizeof(guarded));canvas_t canvas={.pixels=guarded,.rotation=rotation};dashboard_render(&canvas,&s);
        for(unsigned i=SCREEN_BYTES;i<sizeof(guarded);++i)assert(guarded[i]==0xab);
        unsigned requested_count=argc>2?(unsigned)atoi(argv[2]):2,requested_rotation=argc>3?(unsigned)atoi(argv[3]):0;
        if(argc>=2 && count==requested_count && rotation==requested_rotation) {FILE *f=fopen(argv[1],"wb");assert(f);fprintf(f,"P4\n800 480\n");for(unsigned i=0;i<SCREEN_BYTES;++i)fputc(~guarded[i]&0xff,f);fclose(f);}
    }
    puts("Harness display: eight final/unknown snapshots, four orientations, guarded framebuffer OK");
}
