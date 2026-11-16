#include "dashboard.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
static int left,top,width,height,bar_y,primary_center,primary_font,primary_right;
static unsigned box_count,segments_count;
typedef struct{int x,y,r,b;char s[40];}box_t;static box_t boxes[32];
void codex_audit_text(canvas_t *c,int x,int y,const char *s,const bitmap_font_t *f,int scale,bool black){
 int a,b;if(canvas_text_ink_bounds(s,f,&a,&b) && x>=left+10 && x<left+width-10 && y+a>=top+58 && y+a<top+height-8){
  box_t v={x,y+a,x+canvas_text_width(s,f,scale),y+b+1,{0}};snprintf(v.s,sizeof(v.s),"%s",s);
  if(v.r>left+width-10 || v.b>bar_y-2){fprintf(stderr,"Codex bound %s (%d,%d..%d,%d) bar=%d rotation=%u\n",s,v.x,v.y,v.r,v.b,bar_y,c->rotation);abort();}
  for(unsigned i=0;i<box_count;++i){box_t u=boxes[i];if(v.x<u.r && u.x<v.r && v.y<u.b && u.y<v.b){fprintf(stderr,"Codex overlap %s/%s (%d,%d..%d,%d) / (%d,%d..%d,%d) rotation=%u\n",u.s,v.s,u.x,u.y,u.r,u.b,v.x,v.y,v.r,v.b,c->rotation);abort();}}
  assert(box_count<32);boxes[box_count++]=v;
  if(!strchr(s,':') && strspn(s,"0123456789-")==strlen(s) && v.r>left+width-60 && v.y<primary_center+5){
   assert(!primary_font);primary_font=f->height;primary_right=v.r-left;
  }
 }
 canvas_text(c,x,y,s,f,scale,black);
}
void codex_audit_box(canvas_t *c,int x,int y,int w,int h,int radius,int stroke){
 if(y==bar_y && x>=left+16 && x<left+width-16 && (h==8 || h==12))++segments_count;
 canvas_round_box(c,x,y,w,h,radius,stroke);
}
int main(void){
 setenv("TZ","SGT-8",1);tzset();time_t now=time(NULL);struct tm tm;localtime_r(&now,&tm);int day=(tm.tm_year+1900)*10000+(tm.tm_mon+1)*100+tm.tm_mday;
 const int remaining[]={-1,0,570,999,1000};const int64_t tokens[]={-1,0,100015606,9007199254740991LL};unsigned cases=0;
 for(unsigned rotation=0;rotation<360;rotation+=90)for(unsigned count=1;count<=3;++count){
 int fixed_font=0,fixed_right=0;
 for(unsigned q=0;q<5;++q)for(unsigned h=0;h<5;++h)for(unsigned stale=0;stale<2;++stale)for(unsigned reset=0;reset<4;++reset){
  dashboard_state_t s={.page=PAGE_OVERVIEW,.sensor_valid=true,.temperature=25.8,.humidity=61.2};s.network.started=s.network.data_mode=s.network.ble_link=true;
  for(unsigned i=0;i<PROVIDER_COUNT;++i)usage_record_init(&s.network.providers[i]);
  usage_record_t *r=&s.network.providers[0];r->configured=r->has_data=true;r->status=USAGE_OK;r->fetched_at=now;r->token_day=day;r->daily_tokens=0;
  const int64_t reset_at[]={now+8280,0,now+30,now-1};
  r->windows[0]=(quota_window_t){remaining[q],300,reset_at[reset]};r->windows[1]=(quota_window_t){remaining[q],10080,now+86400};
  r->yesterday_day=usage_previous_day(day);r->yesterday_tokens=tokens[h%4];r->yesterday_week_used_tenths=h==0?-1:(h==1?0:(h==2?280:1000));
  if(h==4)r->yesterday_day=usage_previous_day(r->yesterday_day);
  for(unsigned i=1;i<count;++i){usage_record_t *a=&s.network.providers[i+2];*a=*r;strcpy(a->name,i==1?"Argus":"DeepSeek Harness");a->windows[0].remaining_tenths=a->windows[1].remaining_tenths=-1;a->token_day=day;a->daily_tokens=0;}
  if(stale){s.network.ble_link=false;r->fetched_at=now-7200;r->status=USAGE_NETWORK_ERROR;}
  bool portrait=rotation%180;unsigned selected[3];assert(usage_overview_select(s.network.providers,PROVIDER_COUNT,day,now,selected,3)==count);unsigned slot=0;while(selected[slot]!=0)++slot;
  width=portrait?440:(760-12*((int)count-1))/(int)count;height=portrait?(504-12*((int)count-1))/(int)count:260;left=portrait?20:20+(int)slot*(width+12);top=portrait?72+(int)slot*(height+12):64;bar_y=top+height-(height<210?18:21);
  primary_center=top+(height<210?74:(height>=360?140:(height<250?90:96)));
  uint8_t pixels[SCREEN_BYTES];canvas_t c={.pixels=pixels,.rotation=rotation};box_count=segments_count=0;primary_font=primary_right=0;dashboard_render(&c,&s);
  if(segments_count!=10 || box_count<7 || !primary_font){fprintf(stderr,"Codex missing rotation=%u count=%u quota=%d reset=%u font=%d boxes=%u segments=%u center=%d\n",rotation,count,remaining[q],reset,primary_font,box_count,segments_count,primary_center);for(unsigned b=0;b<box_count;++b)fprintf(stderr,"%s (%d,%d..%d,%d)\n",boxes[b].s,boxes[b].x,boxes[b].y,boxes[b].r,boxes[b].b);abort();}
  if(!fixed_font){fixed_font=primary_font;fixed_right=primary_right;}
  if(primary_font!=fixed_font || primary_right!=fixed_right){fprintf(stderr,"Primary changed rotation=%u count=%u quota=%d reset=%u font=%d/%d right=%d/%d\n",rotation,count,remaining[q],reset,primary_font,fixed_font,primary_right,fixed_right);abort();}
  ++cases;
 }
 }
 printf("Codex history: %u layouts, fixed primary size/right edge, countdown boundaries, yesterday token/quota, 1-3 cards and four directions; progress bar retained.\n",cases);
}
