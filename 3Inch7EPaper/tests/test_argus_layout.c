// Audit actual glyph boxes emitted by the production Argus renderer, without device access.
#include "argus_page.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

typedef struct {int left,top,right,bottom;char text[160];} text_box_t;
static text_box_t boxes[96];
static unsigned count;

void argus_audit_text(canvas_t *c,int x,int y,const char *text,const bitmap_font_t *font,int scale,bool black)
{
    int top,bottom;
    if(canvas_text_ink_bounds(text,font,&top,&bottom)) {
        text_box_t b={x,y+top*scale,x+canvas_text_width(text,font,scale),y+(bottom+1)*scale,{0}};
        snprintf(b.text,sizeof(b.text),"%s",text);
        assert(b.left>=20 && b.right<=canvas_width(c)-20 && b.top>=84 && b.bottom<canvas_height(c));
        for(unsigned i=0;i<count;++i) {
            const text_box_t *a=&boxes[i];
            if(b.left<a->right && a->left<b.right && b.top<a->bottom && a->top<b.bottom) {
                fprintf(stderr,"Overlapping Argus text: %s / %s\n",a->text,b.text);
                abort();
            }
        }
        assert(count<sizeof(boxes)/sizeof(boxes[0]));boxes[count++]=b;
    }
    canvas_text(c,x,y,text,font,scale,black);
}
int main(void)
{
    setenv("TZ","SGT-8",1);tzset();time_t now=time(NULL);struct tm t;localtime_r(&now,&t);
    int day=(t.tm_year+1900)*10000+(t.tm_mon+1)*100+t.tm_mday;
    const int64_t totals[]={0,1,999,999949,999950,14200000,19000000,999949999,999950000,999950000000LL,9007199254740991LL};
    unsigned cases=0;uint8_t guarded[SCREEN_BYTES+64];
    for(unsigned rotation=0;rotation<360;rotation+=90)for(unsigned n=0;n<sizeof(totals)/sizeof(totals[0]);++n)for(unsigned state=0;state<5;++state) {
        dashboard_state_t s={0};s.network.ble_link=true;
        usage_record_t *r=&s.network.providers[PROVIDER_ARGUS];usage_record_init(r);
        r->configured=r->has_data=r->usage_available=true;r->status=USAGE_OK;r->fetched_at=now;
        r->token_day=day;r->token_week=usage_week_start(day);r->token_month=usage_month_start(day);r->daily_tokens=r->weekly_tokens=r->monthly_tokens=totals[n];
        r->input_tokens=totals[n]-totals[n]/17;r->output_tokens=totals[n]/17;r->cached_tokens=r->input_tokens;
        r->calls=totals[n];r->daily_cost_available=true;r->daily_cost_usd=12345678.9;r->missing_cost_calls=1;
        usage_breakdown_t *b=&s.network.argus_breakdown;
        *b=(usage_breakdown_t){.received=true,.day=day,.fetched_at=now,.total=totals[n],.model_count=999,.provider_count=999,.model_rows=3,.provider_rows=3};
        for(unsigned i=0;i<3;++i) {
            memset(b->models[i].label,'W',32);b->models[i].label[31]=(char)('0'+i);
            snprintf(b->providers[i].label,sizeof(b->providers[i].label),"ID a18b5c9%u",i);
            b->models[i].tokens=b->providers[i].tokens=i==0?totals[n]-totals[n]/17-totals[n]/1000:(i==1?totals[n]/17:totals[n]/1000);
        }
        if(!totals[n])b->model_rows=b->provider_rows=b->model_count=b->provider_count=0;
        if(state==1)b->received=false;
        if(state==2)r->token_day=20250101;
        if(state==3)r->token_week=20250106;
        if(state==4){s.network.ble_link=false;r->status=USAGE_NETWORK_ERROR;}
        memset(guarded,0xA5,sizeof(guarded));canvas_t c={.pixels=guarded+32,.rotation=rotation};canvas_clear(&c);count=0;
        argus_page_render(&c,&s);
        for(unsigned i=0;i<32;++i)assert(guarded[i]==0xA5 && guarded[SCREEN_BYTES+32+i]==0xA5);
        ++cases;
    }
    printf("Argus: %u layouts, long labels/max counts/rounding/zero/missing/stale, no text overlaps or out-of-bounds glyphs.\n",cases);
}
