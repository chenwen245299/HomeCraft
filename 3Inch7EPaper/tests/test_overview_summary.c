// Actual glyph bounds for the home summary; no hardware or user data.
#include "dashboard.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

typedef struct {int x,y,right,bottom;char text[64];} text_box_t;
static text_box_t boxes[32];static unsigned count;
static const bitmap_font_t *reading_fonts[2];
static int reading_rights[2],reading_baselines[2],unit_baselines[2];
static unsigned reading_count,unit_count;
void overview_audit_text(canvas_t *c,int x,int y,const char *text,const bitmap_font_t *font,int scale,bool black)
{
    int top,bottom;
    bool portrait=canvas_width(c)<canvas_height(c);
    int start=portrait?588:340,end=portrait?762:428;
    if(canvas_text_ink_bounds(text,font,&top,&bottom) && y+top*scale>=start && y+top*scale<end) {
        int environment_left=portrait?310:580;
        if(x>=environment_left && ((text[0]>='0' && text[0]<='9') || text[0]=='-')) {
            assert(reading_count<2);
            reading_fonts[reading_count]=font;
            reading_rights[reading_count]=x+canvas_text_width(text,font,scale);
            reading_baselines[reading_count]=y+(int)font->baseline*scale;
            ++reading_count;
        } else if(x>=environment_left && (!strcmp(text,"°C") || !strcmp(text,"%"))) {
            assert(unit_count<2);unit_baselines[unit_count++]=y+(int)font->baseline*scale;
        }
        text_box_t b={x,y+top*scale,x+canvas_text_width(text,font,scale),y+(bottom+1)*scale,{0}};
        snprintf(b.text,sizeof(b.text),"%s",text);
        assert(b.x>=20 && b.right<=canvas_width(c)-20 && b.bottom<=end);
        for(unsigned i=0;i<count;++i) {
            text_box_t a=boxes[i];
            if(b.x<a.right && a.x<b.right && b.y<a.bottom && a.y<b.bottom) {
                fprintf(stderr,"Summary overlap (%u): %s / %s [%d,%d..%d,%d] [%d,%d..%d,%d]\n",c->rotation,a.text,b.text,a.x,a.y,a.right,a.bottom,b.x,b.y,b.right,b.bottom);abort();
            }
        }
        assert(count<32);boxes[count++]=b;
    }
    canvas_text(c,x,y,text,font,scale,black);
}
int main(void)
{
    setenv("TZ","SGT-8",1);tzset();time_t now=time(NULL);struct tm local;localtime_r(&now,&local);
    int day=(local.tm_year+1900)*10000+(local.tm_mon+1)*100+local.tm_mday;
    const int64_t totals[]={0,1,999,999950,14200000,999949999,9007199254740991LL};
    unsigned cases=0;
    for(unsigned rotation=0;rotation<360;rotation+=90)for(unsigned n=0;n<sizeof(totals)/sizeof(totals[0]);++n)for(unsigned mode=0;mode<5;++mode) {
        dashboard_state_t s={.page=PAGE_OVERVIEW,.sensor_valid=true,.temperature=-45,.humidity=100};
        s.network.started=s.network.data_mode=s.network.ble_link=true;
        for(unsigned i=0;i<PROVIDER_COUNT;++i)usage_record_init(&s.network.providers[i]);
        usage_record_t *r=&s.network.providers[0];r->configured=r->has_data=true;r->status=USAGE_OK;r->fetched_at=now;
        r->windows[0]=(quota_window_t){1000,300,now+8000};r->windows[1]=(quota_window_t){1000,10080,now+86400};
        r->token_day=day;r->daily_tokens=totals[n];r->token_week=usage_week_start(day);r->weekly_tokens=totals[n];r->token_month=usage_month_start(day);r->monthly_tokens=totals[n];
        s.network.hub_totals=(usage_totals_t){.received=true,.periods={day,r->token_week,r->token_month},.tokens={totals[n],totals[n],totals[n]}};
        if(mode==1){r->daily_tokens=-1;r->token_week=20241230;r->token_month=20250101;}
        if(mode==2){s.sensor_valid=false;s.network.ble_link=false;}
        if(mode==3){s.temperature=130;s.humidity=0;}
        if(mode==4){s.temperature=25.8f;s.humidity=61.2f;usage_record_t *a=&s.network.providers[3];*a=*r;a->windows[0].remaining_tenths=a->windows[1].remaining_tenths=-1;strcpy(a->name,"Argus");}
        uint8_t guarded[SCREEN_BYTES+32];memset(guarded,0xA5,sizeof(guarded));
        canvas_t c={.pixels=guarded+16,.rotation=rotation};count=reading_count=unit_count=0;dashboard_render(&c,&s);
        assert(count>=10);
        assert(reading_count==2 && unit_count==2);
        assert(reading_fonts[0]==reading_fonts[1]);
        assert(reading_rights[0]==reading_rights[1]);
        assert(reading_baselines[1]-reading_baselines[0]==(rotation%180?87:44));
        assert(reading_baselines[0]==unit_baselines[0] && reading_baselines[1]==unit_baselines[1]);
        for(unsigned i=0;i<16;++i)assert(guarded[i]==0xA5 && guarded[SCREEN_BYTES+16+i]==0xA5);
        ++cases;
    }
    // The rendered bottom summary must match the hub's multi-account totals in all directions,
    // even when the individual Codex packet contains a much smaller monthly number.
    for(unsigned rotation=0;rotation<360;rotation+=90){
        dashboard_state_t s={.page=PAGE_OVERVIEW,.sensor_valid=true,.temperature=25,.humidity=60};
        s.network.started=s.network.data_mode=s.network.ble_link=true;
        for(unsigned i=0;i<PROVIDER_COUNT;++i)usage_record_init(&s.network.providers[i]);
        usage_record_t *r=&s.network.providers[0];r->configured=r->has_data=true;r->status=USAGE_OK;r->fetched_at=now;
        r->token_day=day;r->daily_tokens=20200000;r->token_week=usage_week_start(day);r->weekly_tokens=105400000;
        r->token_month=usage_month_start(day);r->monthly_tokens=2760000000LL;
        s.network.hub_totals=(usage_totals_t){.received=true,.periods={day,r->token_week,r->token_month},.tokens={r->daily_tokens,r->weekly_tokens,r->monthly_tokens}};
        uint8_t pixels[SCREEN_BYTES];canvas_t c={.pixels=pixels,.rotation=rotation};
        count=reading_count=unit_count=0;dashboard_render(&c,&s);
        text_box_t expected[32];unsigned expected_count=count;memcpy(expected,boxes,sizeof(expected));
        s.network.hub_totals=(usage_totals_t){.received=true,.periods={day,r->token_week,r->token_month},.tokens={r->daily_tokens,r->weekly_tokens,r->monthly_tokens}};
        r->daily_tokens=1000000;r->weekly_tokens=10000000;r->monthly_tokens=1320000000;
        count=reading_count=unit_count=0;dashboard_render(&c,&s);assert(count==expected_count);
        for(unsigned i=0;i<count;++i){
            assert(!strcmp(boxes[i].text,expected[i].text));
            assert(boxes[i].x==expected[i].x && boxes[i].y==expected[i].y && boxes[i].right==expected[i].right && boxes[i].bottom==expected[i].bottom);
        }
    }
    printf("Overview: %u summaries and 4 multi-account hub totals; equal temperature/humidity fonts, aligned numeric edges and unit baselines; no overlaps or buffer overflow.\n",cases);
}
