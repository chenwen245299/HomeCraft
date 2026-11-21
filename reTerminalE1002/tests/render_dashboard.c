#include "dashboard.h"
#include "homecraft_wire.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static void todo(dashboard_state_t *s,unsigned q,const char *a,const char *b,const char *c){todo_quadrant_t *t=&s->todo[q];const char *items[]={a,b,c};for(int i=0;i<3;++i)if(items[i])snprintf(t->items[t->count++],sizeof(t->items[0]),"%s",items[i]);}
int main(int argc,char **argv){
 assert(argc==3);const char *scenario=argv[1];dashboard_state_t s={0};s.now=1792999800;s.connected=true;s.sensor_valid=true;s.temperature=24.3;s.humidity=53;s.battery_mv=3950;
 for(int i=0;i<PROVIDER_COUNT;++i)usage_record_init(&s.usage[i]);
 if(strcmp(scenario,"empty"))for(int i=0;i<5;++i){usage_record_t *r=&s.usage[i];r->configured=true;r->has_data=true;r->status=USAGE_OK;r->fetched_at=s.now;if(i!=2){r->token_day=20261026;r->daily_tokens=(i+1)*2600000;r->token_week=20261026;r->weekly_tokens=(i+1)*14300000;r->token_month=20261001;r->monthly_tokens=(i+1)*56900000;}}
 s.usage[0].windows[0]=(quota_window_t){.remaining_tenths=!strcmp(scenario,"critical")?80:1000,.window_mins=300,.reset_at=s.now+9000};
 s.usage[0].windows[1]=(quota_window_t){.remaining_tenths=470,.window_mins=10080,.reset_at=s.now+240000};
 s.usage[0].yesterday_day=20261025;s.usage[0].yesterday_tokens=31200000;s.usage[0].yesterday_week_used_tenths=380;
 s.usage[2].balance_available=true;s.usage[2].balance=128.45;snprintf(s.usage[2].currency,4,"CNY");
 s.weather[0]=(weather_t){.valid=true,.fetched_at=s.now,.temperature_tenths=271,.low_tenths=252,.high_tenths=298,.code=2,.has_tomorrow=true,.tomorrow_code=95,.tomorrow_low=246,.tomorrow_high=304};snprintf(s.weather[0].city,sizeof(s.weather[0].city),"新加坡");
 s.weather[1]=(weather_t){.valid=true,.fetched_at=s.now,.temperature_tenths=158,.low_tenths=139,.high_tenths=190,.code=61,.has_tomorrow=true,.tomorrow_code=3,.tomorrow_low=120,.tomorrow_high=175};snprintf(s.weather[1].city,sizeof(s.weather[1].city),"Tokyo");
 s.todo_known=true;
 todo(&s,0,"交季度报告","回复王老师邮件",NULL);todo(&s,1,"读完《万历十五年》","整理 HomeCraft 待办","周末跑步十公里");
 todo(&s,2,"取快递",NULL,NULL);todo(&s,3,"给绿萝浇水","换床单",NULL);
 // From HomeCraft's bundled list, so the bold subset carries it.
 s.poem=(poem_t){.valid=true,.count=2};snprintf(s.poem.title,sizeof(s.poem.title),"临平道中");snprintf(s.poem.author,sizeof(s.poem.author),"宋 · 道潜");
 snprintf(s.poem.lines[0],sizeof(s.poem.lines[0]),"风蒲猎猎弄轻柔，欲立蜻蜓不自由。");snprintf(s.poem.lines[1],sizeof(s.poem.lines[1]),"五月临平山下路，藕花无数满汀洲。");
 if(!strcmp(scenario,"empty")){s.weather[0].valid=false;s.weather[1].valid=false;s.connected=false;s.todo_known=false;memset(s.todo,0,sizeof(s.todo));s.poem.valid=false;s.battery_mv=-1;s.sensor_valid=false;for(int i=0;i<PROVIDER_COUNT;++i)usage_record_init(&s.usage[i]);}
 if(!strcmp(scenario,"critical")){s.battery_mv=3550;s.connected=false;s.weather[0].code=95;s.weather[1].valid=false;s.usage[3].daily_tokens=123400000;
  // Not in the bundled list: the whole poem falls back to the regular face.
  snprintf(s.poem.title,sizeof(s.poem.title),"登鹳雀楼");snprintf(s.poem.author,sizeof(s.poem.author),"唐 · 王之涣");
  snprintf(s.poem.lines[0],sizeof(s.poem.lines[0]),"白日依山尽，黄河入海流。");snprintf(s.poem.lines[1],sizeof(s.poem.lines[1]),"欲穷千里目，更上一层楼。");}
 if(!strcmp(scenario,"details"))s.page=HOMECRAFT_PAGE_USAGE;
 if(!strcmp(scenario,"privacy"))s.privacy=true;
 if(!strcmp(scenario,"expired")){s.weather[0].fetched_at=s.now-21601;s.weather[1].fetched_at=s.now-7300;
  // A five-character quatrain from the bundled list: both verses of a line share one row.
  snprintf(s.poem.title,sizeof(s.poem.title),"江中对月");snprintf(s.poem.author,sizeof(s.poem.author),"唐 · 刘长卿");
  snprintf(s.poem.lines[0],sizeof(s.poem.lines[0]),"空洲夕烟敛，望月秋江里。");snprintf(s.poem.lines[1],sizeof(s.poem.lines[1]),"历历沙上人，月中孤渡水。");}
 if(!strcmp(scenario,"pairing")){s.pairing=true;snprintf(s.pairing_code,9,"12345678");snprintf(s.device_name,20,"EPaper_A1B2C3");}
 if(!strcmp(scenario,"long")){
  for(int i=0;i<5;++i){s.usage[i].daily_tokens=9007199254740000LL;s.usage[i].weekly_tokens=9007199254740000LL;s.usage[i].monthly_tokens=9007199254740000LL;}
  snprintf(s.weather[0].city,sizeof(s.weather[0].city),"内蒙古呼和浩特市");s.weather[0].code=63;
  memset(&s.todo[0],0,sizeof(s.todo[0]));for(int i=0;i<TODO_ITEMS;++i)snprintf(s.todo[0].items[s.todo[0].count++],sizeof(s.todo[0].items[0]),"这是一条非常非常长的待办事项%d",i+1);s.todo[0].total=9;
  s.poem=(poem_t){.valid=true,.count=4};snprintf(s.poem.title,sizeof(s.poem.title),"山居秋暝");snprintf(s.poem.author,sizeof(s.poem.author),"唐 · 王维");
  const char *lines[]={"空山新雨后，天气晚来秋。","明月松间照，清泉石上流。","竹喧归浣女，莲动下渔舟。","随意春芳歇，王孙自可留。"};for(int i=0;i<4;++i)snprintf(s.poem.lines[i],sizeof(s.poem.lines[0]),"%s",lines[i]);}
 // Only HomeCraft supplies totals. Missing or stale periods never sum provider packets.
 if(!strcmp(scenario,"overview")){int64_t t[3];dashboard_totals(&s,20261026,t);assert(t[0]==-1&&t[1]==-1&&t[2]==-1);
  s.hub_totals=true;s.hub_periods[0]=20261026;s.hub_periods[1]=20261026;s.hub_periods[2]=20261001;s.hub_tokens[0]=5;s.hub_tokens[1]=-1;s.hub_tokens[2]=1323000000;
  dashboard_totals(&s,20261026,t);assert(t[0]==5&&t[1]==-1&&t[2]==1323000000);
  s.hub_periods[0]=20261025;dashboard_totals(&s,20261026,t);assert(t[0]==-1);s.hub_periods[0]=20261026;}
 // Places from HomeCraft (places:1): this display (here, own sensor) and another display's reading.
 if(!strcmp(scenario,"overview")||!strcmp(scenario,"critical")||!strcmp(scenario,"long")){s.place_count=2;
  s.places[0]=(place_t){.known=true,.temperature_tenths=235,.humidity_tenths=481,.at=s.now-300};snprintf(s.places[0].name,sizeof(s.places[0].name),"书房");
  s.places[1]=(place_t){.here=true};snprintf(s.places[1].name,sizeof(s.places[1].name),"客厅");}
 // The user's setup: one city, so tomorrow takes the second weather column.
 if(!strcmp(scenario,"overview"))s.weather[1].valid=false;
 // Two hours without a new reading: shown with its time in red.
 if(!strcmp(scenario,"critical"))s.places[0].at=s.now-7200;
 // A sixteen-character name, and a display that has not reported yet.
 if(!strcmp(scenario,"long")){snprintf(s.places[0].name,sizeof(s.places[0].name),"这是一个很长很长很长的位置名称");s.places[0].known=false;}
 if(!strcmp(scenario,"long"))s.charge=CHARGE_ACTIVE;
 if(!strcmp(scenario,"expired"))s.charge=CHARGE_DONE;
 uint8_t *frame=malloc(FRAME_BYTES);assert(frame);color_canvas_t c={frame};dashboard_render(&c,&s);
 for(size_t i=0;i<FRAME_BYTES;++i){assert((frame[i]&15)!=4&&(frame[i]&15)<7);assert((frame[i]>>4)!=4&&(frame[i]>>4)<7);}
 assert(quota_ink(1000)==INK_GREEN&&quota_ink(0)==INK_RED&&quota_ink(-1)==INK_BLACK);
 assert(battery_level(-1)==-1&&battery_level(4100)==4&&battery_level(3700)==2&&battery_level(3550)==0);
 assert(battery_percent(-1)==-1&&battery_percent(4200)==100&&battery_percent(3800)==50&&battery_percent(3000)==0);
 FILE *f=fopen(argv[2],"wb");assert(f);assert(fwrite(frame,1,FRAME_BYTES,f)==FRAME_BYTES);fclose(f);free(frame);return 0;
}
