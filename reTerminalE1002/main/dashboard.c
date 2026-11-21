#include "dashboard.h"
#include "homecraft_wire.h"
#include "lunar.h"
#include "privacy.h"
#include <math.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
int quota_ink(int n){return n<0?INK_BLACK:n<=200?INK_RED:n<=400?INK_YELLOW:INK_GREEN;}
// Typical Li-ion resting voltage curve; an estimate, not a calibrated state of charge.
int battery_percent(int mv){
 static const int curve[][2]={{3300,0},{3500,5},{3600,10},{3650,20},{3700,30},{3750,40},{3800,50},{3900,65},{4000,80},{4100,90},{4200,100}};
 if(mv<2500)return -1;
 if(mv<=curve[0][0])return 0;
 for(unsigned i=1;i<sizeof(curve)/sizeof(curve[0]);++i)if(mv<=curve[i][0])return curve[i-1][1]+(mv-curve[i-1][0])*(curve[i][1]-curve[i-1][1])/(curve[i][0]-curve[i-1][0]);
 return 100;
}
int battery_level(int mv){int p=battery_percent(mv);return p<0?-1:p>=75?4:p>=50?3:p>=25?2:p>=10?1:0;}
static void text(color_canvas_t *c,int x,int y,const char *s,const bitmap_font_t *f,int ink){color_text(c,x,y,s,f,ink);}
static void num(char *b,size_t n,int64_t v){if(v<0)snprintf(b,n,"--");else if(v>=1000000000000000LL)snprintf(b,n,"%.2fP",v/1e15);else if(v>=1000000000000LL)snprintf(b,n,"%.2fT",v/1e12);else if(v>=1000000000)snprintf(b,n,"%.2fB",v/1e9);else if(v>=1000000)snprintf(b,n,"%.1fM",v/1e6);else if(v>=1000)snprintf(b,n,"%.1fK",v/1e3);else snprintf(b,n,"%lld",(long long)v);}
static void right(color_canvas_t *c,int x,int y,const char *s,const bitmap_font_t *f,int ink){text(c,x-color_text_width(s,f),y,s,f,ink);}
static const char *condition(int code){if(code==0)return "晴";if(code<=2)return "少云";if(code==3)return "多云";if(code<=48)return "雾";if(code<=67)return "雨";if(code<=77)return "雪";if(code<=82)return "阵雨";if(code<=86)return "阵雪";return "雷雨";}
static int weather_icon(int code){if(code==0)return ICON_SUN;if(code<=2)return ICON_PARTLY;if(code==3)return ICON_CLOUD;if(code<=48)return ICON_FOG;if(code<=67||(code>=80&&code<=82))return ICON_RAIN;if(code<=77||code==85||code==86)return ICON_SNOW;return ICON_THUNDER;}
static void rule(color_canvas_t *c,int x,int y,int w){color_fill(c,x,y,w,1,INK_BLACK);}
static void dot(color_canvas_t *c,int cx,int cy,int r,uint8_t ink){for(int y=-r;y<=r;++y)for(int x=-r;x<=r;++x)if(x*x+y*y<=r*r+r)color_pixel(c,cx+x,cy+y,ink);}
static int today(int64_t now,struct tm *out){if(now<=1735689600)return 0;time_t t=now+8*3600;gmtime_r(&t,out);return (out->tm_year+1900)*10000+(out->tm_mon+1)*100+out->tm_mday;}
static void bar(color_canvas_t *c,int x,int y,int w,int h,int tenths){color_rect(c,x,y,w,h,INK_BLACK);if(tenths>0)color_fill(c,x+1,y+1,(w-2)*(tenths>1000?1000:tenths)/1000,h-2,quota_ink(tenths));}
static void header(color_canvas_t *c,const dashboard_state_t *s){
 text(c,16,4,"HomeCraft",&bold20,INK_BLACK);
 struct tm d;if(today(s->now,&d)){static const char *week[]={"周日","周一","周二","周三","周四","周五","周六"};char b[64],lunar[24];
  int n=snprintf(b,sizeof(b),"%d月%d日 %s",d.tm_mon+1,d.tm_mday,week[d.tm_wday]);
  if(lunar_text(d.tm_year+1900,d.tm_mon+1,d.tm_mday,lunar,sizeof(lunar))&&n>0&&(size_t)n<sizeof(b))snprintf(b+n,sizeof(b)-(size_t)n," · 农历%s",lunar);
  text(c,16+color_text_width("HomeCraft",&bold20)+14,7,b,&text16,INK_BLACK);}
 // Right cluster: Bluetooth, battery, then the percent in a slot sized for "100%" so nothing shifts.
 // On the charger the voltage says nothing about the level: a bolt and 充电中/已充满 replace bars and percent.
 enum {BATTERY_Y=13};bool power=s->charge==CHARGE_ACTIVE||s->charge==CHARGE_DONE;int pct=battery_percent(s->battery_mv);char b[16]="";
 if(power)snprintf(b,sizeof(b),"%s",s->charge==CHARGE_ACTIVE?"充电中":"已充满");else if(pct>=0)snprintf(b,sizeof(b),"%d%%",pct);
 int slot=color_text_width("100%",&text16),width=color_text_width(b,&text16);if(width>slot)slot=width;
 int battery_x=784-(b[0]?slot+5:0)-26;
 color_battery(c,battery_x,BATTERY_Y,power?-1:battery_level(s->battery_mv));
 // Charging: green bolt in an empty battery; charged: white bolt on a full green battery.
 if(s->charge==CHARGE_ACTIVE)color_bolt(c,battery_x,BATTERY_Y,INK_GREEN);
 else if(power){color_fill(c,battery_x+2,BATTERY_Y+2,20,9,INK_GREEN);color_bolt(c,battery_x,BATTERY_Y,INK_WHITE);}
 if(b[0])text(c,battery_x+26+5,7,b,&text16,INK_BLACK);
 color_icon(c,s->connected?ICON_BT:ICON_BT_OFF,battery_x-24,BATTERY_Y-4);
 rule(c,16,34,768);
}
static bool stale(const usage_record_t *r,int64_t now){return r->has_data&&(r->status!=USAGE_OK||now-r->fetched_at>900);}
// One grid for the three services and the totals below: three rows per card, values right-aligned.
// Three equal cards at DeepSeek's width (its title needs it); the todo/poem column on the right gives up the room.
static const int CX[]={16,204,392},CW[]={174,174,174};
// The first labels sit between the service title and the big value, not on the value's top edge.
enum {GUTTER=7,ROW1_Y=84,LABEL1_Y=ROW1_Y-12,ROW2_Y=168,ROW3_Y=218,CARD_BOTTOM=246,TOTALS_Y=CARD_BOTTOM+18,CLIMATE_TOP=TOTALS_Y+70,NOTE_DY=24,CLOCK_DY=3,
 // One vertical split for both rows: cards and weather on the left, todos and the poem on the right.
 SPLIT=574,CLIMATE_EDGE=SPLIT-8,RIGHT_X=SPLIT+10};
static void row(color_canvas_t *c,int x,int w,int y,const char *label,const char *value,const bitmap_font_t *f,int dy,int ink){
 // The big value sits below its label; one too wide for the card (e.g. 123.4M) steps down to digits28.
 if(f==&digits40&&color_text_width(value,f)>w-6){f=&digits28;dy+=10;}
 text(c,x,y,label,&text18,INK_BLACK);right(c,x+w,y+dy,value,f,ink);}
static void codex_card(color_canvas_t *c,int x,int w,const dashboard_state_t *s,int day){
 usage_record_t r=s->usage[0];char b[48];
 for(int i=0;i<2;++i)if(r.windows[i].reset_at>0&&r.windows[i].reset_at<=s->now)r.windows[i].remaining_tenths=-1;
 int five=r.windows[0].remaining_tenths,week=r.windows[1].remaining_tenths;
 if(five<0)snprintf(b,sizeof(b),"--");else snprintf(b,sizeof(b),"%d%%",(five+5)/10);
 // A smaller % keeps even 100% at full size beside the reset time.
 text(c,x,LABEL1_Y,"5小时",&text18,INK_BLACK);
 if(five<0)right(c,x+w,ROW1_Y+8,b,&digits40,INK_BLACK);
 else{int pw=color_text_width("%",&digits28);right(c,x+w,ROW1_Y+8+(int)digits40.baseline-(int)digits28.baseline,"%",&digits28,quota_ink(five));
  snprintf(b,sizeof(b),"%d",(five+5)/10);right(c,x+w-pw,ROW1_Y+8,b,&digits40,quota_ink(five));}
 // The reset time sits left of the big value, after a clock icon, on the value's centre line.
 if(stale(&r,s->now))text(c,x,ROW1_Y+NOTE_DY,"旧数据",&text14,INK_RED);
 else if(five>=0&&r.windows[0].reset_at>s->now){time_t t=r.windows[0].reset_at+8*3600;struct tm d;gmtime_r(&t,&d);char at[8];snprintf(at,sizeof(at),"%02d:%02d",d.tm_hour,d.tm_min);
  color_icon(c,ICON_CLOCK,x,ROW1_Y+NOTE_DY+CLOCK_DY);text(c,x+16,ROW1_Y+NOTE_DY,at,&text14,INK_BLACK);}
 bar(c,x,ROW1_Y+58,w,8,five);
 if(week<0)snprintf(b,sizeof(b),"--");else snprintf(b,sizeof(b),"%d%%",(week+5)/10);
 row(c,x,w,ROW2_Y,"本周",b,&bold24,-4,quota_ink(week));
 bar(c,x,ROW2_Y+31,w,5,week);
 num(b,sizeof(b),r.yesterday_day==usage_previous_day(day)?r.yesterday_tokens:-1);row(c,x,w,ROW3_Y,"昨日",b,&bold24,-4,INK_BLACK);
}
static void token_card(color_canvas_t *c,int x,int w,const dashboard_state_t *s,int provider,int day,int accent){
 const usage_record_t *r=&s->usage[provider];char b[48];
 num(b,sizeof(b),r->token_day==day?r->daily_tokens:-1);row(c,x,w,LABEL1_Y,"今日",b,&digits40,ROW1_Y+8-LABEL1_Y,INK_BLACK);
 if(stale(r,s->now))text(c,x+color_text_width("今日",&text18)+8,LABEL1_Y+3,"旧数据",&text14,INK_RED);
 color_fill(c,x,ROW1_Y+61,w,2,accent);
 num(b,sizeof(b),r->token_week==usage_week_start(day)?r->weekly_tokens:-1);row(c,x,w,ROW2_Y,"本周",b,&bold24,-4,INK_BLACK);
 num(b,sizeof(b),r->token_month==usage_month_start(day)?r->monthly_tokens:-1);row(c,x,w,ROW3_Y,"本月",b,&bold24,-4,INK_BLACK);
}
// The display copies matching hub periods; it never derives totals from provider packets.
void dashboard_totals(const dashboard_state_t *s,int day,int64_t out[3]){
 int periods[]={day,usage_week_start(day),usage_month_start(day)};
 for(int i=0;i<3;++i)out[i]=s->hub_totals&&periods[i]&&s->hub_periods[i]==periods[i]?s->hub_tokens[i]:-1;
}
static void totals(color_canvas_t *c,const dashboard_state_t *s,int day){
 static const char *labels[]={"今日合计","本周合计","本月合计"};int64_t values[3];char b[24];dashboard_totals(s,day,values);
 for(int i=0;i<3;++i){text(c,CX[i],TOTALS_Y,labels[i],&bold16,INK_BLUE);num(b,sizeof(b),values[i]);right(c,CX[i]+CW[i],TOTALS_Y+20,b,&digits28,INK_BLACK);}
}
// Weather and places side by side, one column each: today, then tomorrow (or the second city), then up to two
// places named in HomeCraft, or the indoor sensor. A column is icon and name, a big value, one detail line.
typedef struct {int icon,name_ink,detail_ink;char name[49],value[16],detail[48];} strip_column_t;
static void strip_column(color_canvas_t *c,int x,int w,const strip_column_t *col){
 enum {TOP=CLIMATE_TOP};
 color_icon(c,col->icon,x,col->icon==ICON_HOME?TOP+5:TOP+2);
 color_text_ellipsis(c,x+38,TOP+9,w-38,col->name,&text18,col->name_ink);
 // A value too wide for the column (a forecast range) steps down to digits28 on the same baseline.
 const bitmap_font_t *f=color_text_width(col->value,&digits40)<=w?&digits40:&digits28;
 text(c,x,TOP+46+(int)digits40.baseline-(int)f->baseline,col->value,f,INK_BLACK);
 color_text_ellipsis(c,x,TOP+100,w,col->detail,&text18,col->detail_ink);
}
static void place_column(strip_column_t *col,const char *name,bool known,double temperature,double humidity){
 *col=(strip_column_t){.icon=ICON_HOME,.name_ink=INK_BLACK,.detail_ink=INK_BLACK};snprintf(col->name,sizeof(col->name),"%s",name);
 if(known){snprintf(col->value,sizeof(col->value),"%.1f°",temperature);snprintf(col->detail,sizeof(col->detail),"湿度 %.0f%%",humidity);}
 else{snprintf(col->value,sizeof(col->value),"--");snprintf(col->detail,sizeof(col->detail),"湿度 --");}
}
static void climate(color_canvas_t *c,const dashboard_state_t *s){
 enum {LEFT=16,EDGE=CLIMATE_EDGE,MAX=4};strip_column_t cols[MAX];int n=0;
 const weather_t *cities[2];unsigned cn=0;
 for(int i=0;i<2;++i){const weather_t *w=&s->weather[i];int64_t age=s->now>1735689600?s->now-w->fetched_at:0;
  // HomeCraft stops sending after 6h; an expired summary is hidden, never shown as current.
  if(w->valid&&age<=21600)cities[cn++]=w;}
 if(!cn){cols[n++]=(strip_column_t){.icon=ICON_CLOUD,.name_ink=INK_BLUE,.detail_ink=INK_BLUE,.name="天气",.value="--",.detail="在 HomeCraft 设置城市"};}
 for(unsigned i=0;i<cn;++i){const weather_t *w=cities[i];bool late=s->now-w->fetched_at>7200;strip_column_t *col=&cols[n++];
  *col=(strip_column_t){.icon=weather_icon(w->code),.name_ink=INK_BLUE,.detail_ink=late?INK_RED:INK_BLACK};snprintf(col->name,sizeof(col->name),"%s",w->city);
  snprintf(col->value,sizeof(col->value),"%.0f°",w->temperature_tenths/10.0);
  snprintf(col->detail,sizeof(col->detail),"%s %.0f°/%.0f°",late?"待更新":condition(w->code),w->low_tenths/10.0,w->high_tenths/10.0);}
 // With one city the second weather column is tomorrow; tomorrow has no current reading, so its value is the range.
 if(cn==1&&cities[0]->has_tomorrow){const weather_t *w=cities[0];strip_column_t *col=&cols[n++];
  *col=(strip_column_t){.icon=weather_icon(w->tomorrow_code),.name_ink=INK_BLACK,.detail_ink=INK_BLACK,.name="明天"};
  snprintf(col->value,sizeof(col->value),"%.0f°/%.0f°",w->tomorrow_low/10.0,w->tomorrow_high/10.0);
  snprintf(col->detail,sizeof(col->detail),"%s",condition(w->tomorrow_code));}
 // Places: this display uses its own sensor; another one HomeCraft's reading, marked with its time after an hour
 // and unknown after six. A free slot without this display shows the indoor sensor.
 bool here=false;
 for(unsigned i=0;i<s->place_count&&i<PLACES&&n<MAX;++i){const place_t *p=&s->places[i];strip_column_t *col=&cols[n++];
  if(p->here){here=true;place_column(col,p->name,s->sensor_valid,s->temperature,s->humidity);continue;}
  int64_t age=s->now>1735689600&&p->known?s->now-p->at:0;
  place_column(col,p->name,p->known&&age<=21600,p->temperature_tenths/10.0,p->humidity_tenths/10.0);
  if(p->known&&age>3600&&age<=21600){time_t t=p->at+8*3600;struct tm d;gmtime_r(&t,&d);
   snprintf(col->detail,sizeof(col->detail),"%.0f%% · %02d:%02d",p->humidity_tenths/10.0,d.tm_hour,d.tm_min);col->detail_ink=INK_RED;}}
 if(!here&&s->place_count<PLACES&&n<MAX)place_column(&cols[n++],"室内",s->sensor_valid,s->temperature,s->humidity);
 int w=(EDGE-LEFT-(n-1)*2*GUTTER)/n;
 for(int i=0;i<n;++i){int x=LEFT+i*(w+2*GUTTER);strip_column(c,x,w,&cols[i]);
  if(i<n-1)color_fill(c,x+w+GUTTER,CLIMATE_TOP,1,120,INK_BLACK);}
}
// Todos as one list; the dot colour shows the Eisenhower quadrant (grid view lives in HomeCraft).
static void todos(color_canvas_t *c,const dashboard_state_t *s){
 static const uint8_t inks[]={INK_RED,INK_BLUE,INK_YELLOW,INK_GREEN};
 const int x=RIGHT_X,top=44,bottom=CLIMATE_TOP-14,line=26;
 text(c,x,top,"待办",&bold18,INK_BLACK);
 if(!s->todo_known){text(c,x,top+34,"在 HomeCraft 添加待办",&text16,INK_BLACK);return;}
 unsigned total=0,shown=0;int y=top+32;
 for(int q=0;q<TODO_QUADRANTS;++q){const todo_quadrant_t *t=&s->todo[q];total+=t->total>t->count?t->total:t->count;
  for(unsigned i=0;i<t->count&&i<TODO_ITEMS;++i){if(y+line>bottom)continue;
   // Yellow is faint on white, so its dot gets a black ring.
   if(inks[q]==INK_YELLOW)dot(c,x+6,y+13,6,INK_BLACK);
   dot(c,x+6,y+13,5,inks[q]);
   color_text_ellipsis(c,x+18,y+1,784-x-18,t->items[i],&text18,INK_BLACK);y+=line;++shown;}}
 if(!total)text(c,x,top+34,"没有待办",&text16,INK_BLACK);
 else{char b[24];if(total>shown)snprintf(b,sizeof(b),"%u 项 · 另 %u",total,total-shown);else snprintf(b,sizeof(b),"%u 项",total);right(c,784,top+2,b,&text16,INK_BLACK);}
}
static void centered(color_canvas_t *c,int cx,int y,int maxw,const char *s,const bitmap_font_t *f,int ink){
 int w=color_text_width(s,f);if(w>maxw)color_text_ellipsis(c,cx-maxw/2,y,maxw,s,f,ink);else text(c,cx-w/2,y,s,f,ink);}
// The quatrain box under the todos, as wide as them: title and author on one line, verses centred and bold.
// A short (five-character) poem keeps both verses of a line on one row, so it can use the larger rows.
static void poem(color_canvas_t *c,const dashboard_state_t *s){
 const int width=784-RIGHT_X,cx=RIGHT_X+width/2,top=CLIMATE_TOP-2,bottom=474;const poem_t *p=&s->poem;
 if(!p->valid){color_text_wrap(c,RIGHT_X,top+6,width,24,4,"每日一首古诗，由 HomeCraft 同步",&text18,INK_BLACK);return;}
 unsigned lines=p->count<2?p->count:2;bool heavy=color_font_covers(p->title,&bold18);
 for(unsigned i=0;i<lines;++i)heavy=heavy&&color_font_covers(p->lines[i],&bold20);
 const bitmap_font_t *big=heavy?&bold20:&text20;
 bool paired=true;for(unsigned i=0;i<lines;++i)paired=paired&&color_text_width(p->lines[i],big)<=width;
 char rows[4][61];int n=0;
 for(unsigned i=0;i<lines;++i){
  if(paired){snprintf(rows[n++],sizeof(rows[0]),"%s",p->lines[i]);continue;}
  // Split a line into its two verses at the first full-width ，？！or；.
  const char *verse=p->lines[i],*comma=NULL;
  for(const char *mark=verse;*mark&&!comma;++mark)if((unsigned char)mark[0]==0xEF&&(unsigned char)mark[1]==0xBC&&(mark[2]=='\x8c'||mark[2]=='\x9f'||mark[2]=='\x81'||mark[2]=='\x9b'))comma=mark;
  if(comma){size_t k=(size_t)(comma-verse)+3;memcpy(rows[n],verse,k);rows[n++][k]=0;verse=comma+3;}
  if(n<4)snprintf(rows[n++],sizeof(rows[0]),"%s",verse);
 }
 // Title and author share the first line, centred as one group.
 const bitmap_font_t *head=heavy?&bold18:&text18;int tw=color_text_width(p->title,head),aw=color_text_width(p->author,&text16),gap=10;
 int body=top+30;
 if(tw+gap+aw<=width){int x=cx-(tw+gap+aw)/2;text(c,x,top+2,p->title,head,INK_RED);text(c,x+tw+gap,top+4,p->author,&text16,INK_BLUE);}
 else{centered(c,cx,top+2,width,p->title,head,INK_RED);centered(c,cx,top+24,width,p->author,&text16,INK_BLUE);body=top+46;}
 // Verses fill the space below, centred vertically; two rows get more air than four.
 int pitch=n<=2?34:(bottom-body)>=4*26?26:(bottom-body)/4,block=(n-1)*pitch+(int)big->height,y=body+((bottom-body)-block)/2;
 for(int i=0;i<n;++i)centered(c,cx,y+i*pitch,width,rows[i],big,INK_BLACK);
}
// Privacy: the picture fills the screen; only a small caption names the work.
static void privacy(color_canvas_t *c){
 memcpy(c->pixels,privacy_frame,FRAME_BYTES);
 int w=color_text_width(privacy_caption,&text14)+12;
 color_fill(c,784-w,454,w,20,INK_WHITE);text(c,784-w+6,456,privacy_caption,&text14,INK_BLACK);
}
static void details(color_canvas_t *c,const dashboard_state_t *s,int day){
 static const char *labels[]={"今日","本周","本月"};char b[100];
 text(c,16,44,"用量明细",&bold20,INK_BLACK);
 for(int j=0;j<3;++j)right(c,474+j*145,48,labels[j],&text16,INK_BLUE);
 for(int p=0;p<5;++p){const usage_record_t *r=&s->usage[p];int y=84+p*62;color_logo(c,p,16,y+2);
  text(c,48,y,p==4?"DeepSeek Harness":p==2?"DeepSeek API":usage_record_name(p,r),&bold20,INK_BLACK);
  int64_t values[]={r->token_day==day?r->daily_tokens:-1,r->token_week==usage_week_start(day)?r->weekly_tokens:-1,r->token_month==usage_month_start(day)?r->monthly_tokens:-1};
  for(int j=0;j<3;++j){num(b,sizeof(b),values[j]);right(c,474+j*145,y,b,&text20,INK_BLACK);}
  if(p==0){char tokens[24],used[20];bool yesterday=r->yesterday_day==usage_previous_day(day);num(tokens,sizeof(tokens),yesterday?r->yesterday_tokens:-1);if(yesterday&&r->yesterday_week_used_tenths>=0)snprintf(used,sizeof(used),"%d%%",(r->yesterday_week_used_tenths+5)/10);else snprintf(used,sizeof(used),"--");snprintf(b,sizeof(b),"昨日 %s · 周已用 %s",tokens,used);text(c,48,y+30,b,&text16,INK_BLACK);}
  rule(c,48,y+56,736);
 }
}
void dashboard_render(color_canvas_t *c,const dashboard_state_t *s){
 if(s->privacy&&!s->pairing){privacy(c);return;}
 memset(c->pixels,0x11,FRAME_BYTES);
 header(c,s);
 if(s->pairing){
  text(c,34,60,"安全连接",&bold32,INK_BLUE);
  text(c,34,116,"在 HomeCraft 添加设备，输入屏幕配对码",&text24,INK_BLACK);
  text(c,34,172,s->pairing_code,&digits80,INK_BLUE);
  text(c,36,292,s->device_name,&bold24,INK_BLACK);
  text(c,36,342,"10分钟内有效 · 每台电脑只需认证一次",&text20,INK_BLACK);
  rule(c,16,440,768);text(c,16,448,"长按左键重新打开配对窗口",&text16,INK_BLACK);return;
 }
 struct tm d;int day=today(s->now,&d);
 if(s->page==HOMECRAFT_PAGE_USAGE){details(c,s,day);return;}
 // Three primary services on one grid; HomeCraft stays the data hub.
 static const char *names[]={"Codex","Argus","DeepSeek"};static const int providers[]={0,3,4};
 for(int i=0;i<3;++i){int x=CX[i];color_logo(c,providers[i],x,44);text(c,x+28,46,names[i],&bold16,INK_BLACK);
  if(i==2)text(c,x+28+color_text_width(names[i],&bold16)+4,48,"Harness",&text14,INK_BLACK);
  if(i==0)codex_card(c,x,CW[i],s,day);else token_card(c,x,CW[i],s,providers[i],day,i==1?INK_RED:INK_BLUE);
  if(i<2)color_fill(c,x+CW[i]+GUTTER,46,1,CARD_BOTTOM-46,INK_BLACK);}
 rule(c,16,CARD_BOTTOM+8,CLIMATE_EDGE-16);totals(c,s,day);
 // One rule across both columns separates the rows.
 rule(c,16,CLIMATE_TOP-10,768);climate(c,s);
 color_fill(c,SPLIT,42,1,432,INK_BLACK);todos(c,s);poem(c,s);
}
