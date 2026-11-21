// Render the actual three-card overview through success, failure and missing snapshots.
#include "dashboard.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

typedef struct { int x,y,w,h; } rect_t;
typedef struct { unsigned slot; int x,y,r,b; char text[80]; } text_t;
static rect_t cards[3];
static text_t texts[96];
static unsigned card_count,text_count;

void overview_cards_box(canvas_t *c,int x,int y,int w,int h,int radius,int stroke)
{
    if(radius==14 && stroke==2){
        assert(card_count<3);
        cards[card_count++]=(rect_t){x,y,w,h};
    }
    canvas_round_box(c,x,y,w,h,radius,stroke);
}

void overview_cards_text(canvas_t *c,int x,int y,const char *text,const bitmap_font_t *font,int scale,bool black)
{
    int top,bottom;
    if(canvas_text_ink_bounds(text,font,&top,&bottom))for(unsigned slot=0;slot<card_count;++slot){
        rect_t a=cards[slot];
        int ink_y=y+top*scale;
        if(x<a.x || x>=a.x+a.w || ink_y<a.y || ink_y>=a.y+a.h)continue;
        text_t b={slot,x,ink_y,x+canvas_text_width(text,font,scale),y+(bottom+1)*scale,{0}};
        snprintf(b.text,sizeof(b.text),"%s",text);
        if(!(b.x>=a.x+4 && b.r<=a.x+a.w-4 && b.b<=a.y+a.h-4)){
            fprintf(stderr,"Overview bounds rotation=%u slot=%u: %s [%d,%d..%d,%d] card=[%d,%d %dx%d]\n",
                c->rotation,slot,b.text,b.x,b.y,b.r,b.b,a.x,a.y,a.w,a.h);abort();
        }
        for(unsigned i=0;i<text_count;++i){
            text_t old=texts[i];
            if(old.slot==slot && b.x<old.r && old.x<b.r && b.y<old.b && old.y<b.b){
                fprintf(stderr,"Overview overlap rotation=%u slot=%u: %s [%d,%d..%d,%d] / %s [%d,%d..%d,%d]\n",
                    c->rotation,slot,old.text,old.x,old.y,old.r,old.b,b.text,b.x,b.y,b.r,b.b);
                abort();
            }
        }
        assert(text_count<96);texts[text_count++]=b;break;
    }
    canvas_text(c,x,y,text,font,scale,black);
}

static unsigned occurrences(unsigned slot,const char *text)
{
    unsigned n=0;for(unsigned i=0;i<text_count;++i)if(texts[i].slot==slot && !strcmp(texts[i].text,text))++n;
    return n;
}

static void render(const dashboard_state_t *state,unsigned rotation,const char *directory,const char *name)
{
    uint8_t guarded[SCREEN_BYTES+32];memset(guarded,0xA5,sizeof(guarded));
    canvas_t c={.pixels=guarded+16,.rotation=rotation};card_count=text_count=0;
    dashboard_render(&c,state);
    assert(card_count==3);
    bool portrait=rotation%180;
    for(unsigned i=0;i<3;++i){
        rect_t expected={portrait?20:20+(int)i*257,portrait?72+(int)i*172:64,portrait?440:245,portrait?160:260};
        assert(!memcmp(&cards[i],&expected,sizeof(expected)));
    }
    assert(occurrences(0,"Codex")==1 && occurrences(1,"Argus")==1 && occurrences(2,"DeepSeek")==1 && occurrences(2,"Harness")==1);
    const unsigned providers[]={PROVIDER_CODEX,PROVIDER_ARGUS,PROVIDER_EXTRA1};
    for(unsigned i=0;i<3;++i){
        const usage_record_t *r=&state->network.providers[providers[i]];
        if(!r->configured)assert(occurrences(i,"等待同步")==1);
        else if(r->status!=USAGE_OK)assert(occurrences(i,usage_status_text(r->status))==1);
        if(!r->has_data)assert(occurrences(i,"--")>=(i?3:2));
        if(i)assert(occurrences(i,"今日")==1 && occurrences(i,"本周")==1 && occurrences(i,"本月")==1);
    }
    for(unsigned i=0;i<16;++i)assert(guarded[i]==0xA5 && guarded[SCREEN_BYTES+16+i]==0xA5);
    if(directory && name){
        char path[1024];snprintf(path,sizeof(path),"%s/%s-%u.pbm",directory,name,rotation);
        FILE *f=fopen(path,"wb");assert(f);fprintf(f,"P4\n800 480\n");
        for(unsigned i=0;i<SCREEN_BYTES;++i)fputc(~c.pixels[i]&0xff,f);
        assert(!fclose(f));
    }
}

int main(int argc,char **argv)
{
    setenv("TZ","SGT-8",1);tzset();
    time_t now=time(NULL);struct tm date;localtime_r(&now,&date);
    int day=(date.tm_year+1900)*10000+(date.tm_mon+1)*100+date.tm_mday;
    dashboard_state_t s={.page=PAGE_OVERVIEW,.sensor_valid=true,.temperature=25.8f,.humidity=61.2f};
    s.network.started=s.network.data_mode=s.network.ble_link=true;
    for(unsigned p=0;p<PROVIDER_COUNT;++p)usage_record_init(&s.network.providers[p]);
    const char *directory=argc>1?argv[1]:NULL;
    unsigned cases=0;
    for(unsigned rotation=0;rotation<360;rotation+=90){
        render(&s,rotation,directory,"waiting");++cases;
        dashboard_state_t ready=s;
        const unsigned services[]={PROVIDER_CODEX,PROVIDER_ARGUS,PROVIDER_EXTRA1};
        for(unsigned i=0;i<3;++i){
            usage_record_t *r=&ready.network.providers[services[i]];
            r->configured=r->has_data=true;r->status=USAGE_OK;r->fetched_at=now;
            r->token_day=day;r->daily_tokens=14200000;r->token_week=usage_week_start(day);r->weekly_tokens=123456789;
            r->token_month=usage_month_start(day);r->monthly_tokens=2345678901LL;
        }
        ready.network.providers[0].windows[0]=(quota_window_t){570,300,now+3600};
        ready.network.providers[0].windows[1]=(quota_window_t){840,10080,now+86400};
        render(&ready,rotation,directory,"ready");++cases;
        for(unsigned i=0;i<3;++i)for(unsigned status=0;status<=USAGE_ACCESS_DENIED;++status){
            dashboard_state_t failed=ready;
            usage_record_t incoming;usage_record_init(&incoming);
            incoming.configured=status!=USAGE_NOT_CONFIGURED;incoming.has_data=status==USAGE_OK;
            incoming.status=status;incoming.fetched_at=now+1;
            assert(usage_display_update(&failed.network.providers[services[i]],&incoming));
            render(&failed,rotation,directory,i==2 && status==USAGE_NETWORK_ERROR?"failed":NULL);++cases;
            assert(usage_display_update(&failed.network.providers[services[i]],&ready.network.providers[services[i]]));
            // A stale success cannot resurrect data cleared by a newer failure.
            assert(failed.network.providers[services[i]].status==status);
            usage_record_t recovered=ready.network.providers[services[i]];recovered.fetched_at=now+2;
            assert(usage_display_update(&failed.network.providers[services[i]],&recovered));
            render(&failed,rotation,NULL,NULL);++cases;
        }
        dashboard_state_t stale=ready;stale.network.ble_link=false;
        for(unsigned i=0;i<3;++i)stale.network.providers[services[i]].fetched_at=now-172800;
        render(&stale,rotation,directory,"stale");++cases;
        dashboard_state_t zero=ready;
        for(unsigned i=0;i<3;++i)zero.network.providers[services[i]].daily_tokens=0;
        render(&zero,rotation,NULL,NULL);++cases;
    }
    printf("Overview fixed cards: %u four-direction renders; stable slots/titles through every status, missing/zero/old data; placeholders, text bounds/overlaps and buffer guards passed.\n",cases);
}
