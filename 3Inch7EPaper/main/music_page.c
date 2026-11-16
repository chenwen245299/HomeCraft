#include "dashboard.h"
#include <stdio.h>
#include <string.h>
extern const bitmap_font_t font_music20;
static unsigned cp(const char **p)
{
    const unsigned char *s=(const unsigned char *)*p;unsigned v=*s++,n=0;
    if(v>=0xc2&&v<=0xdf){v&=31;n=1;}else if(v>=0xe0&&v<=0xef){v&=15;n=2;}else if(v>=0xf0&&v<=0xf4){v&=7;n=3;}
    while(n--&&(*s&0xc0)==0x80)v=(v<<6)|(*s++&63);
    *p=(const char *)s;return v;
}
static int glyph_width(unsigned c)
{
    unsigned lo=0,hi=font_music20.count;
    while(lo<hi){unsigned mid=(lo+hi)/2;if(font_music20.glyphs[mid].codepoint<c)lo=mid+1;else hi=mid;}
    return lo<font_music20.count&&font_music20.glyphs[lo].codepoint==c?font_music20.glyphs[lo].width:12;
}
static void name_text(canvas_t *c,int x,int y,int width,const char *text)
{
    char out[MUSIC_NAME_MAX+1];const char *p=text,*end=text;int used=0;
    while(*p){const char *start=p;unsigned ch=cp(&p);int w=glyph_width(ch);if(used+w>width-18){p=start;break;}used+=w;end=p;}
    size_t n=(size_t)(end-text);if(n>sizeof(out)-4)n=sizeof(out)-4;
    memcpy(out,text,n);out[n]=0;if(*p)strcat(out,"...");
    canvas_text(c,x,y,out,&font_music20,1,true);
}
void dashboard_music_page(canvas_t *c,const music_snapshot_t *s)
{
    int width=canvas_width(c),height=canvas_height(c);bool portrait=width<height;
    const char *labels[]={"未检测到SD卡","正在读取","暂无歌曲","待播放","正在播放","已暂停","无法播放"};
    unsigned state=s->state<=MUSIC_ERROR?s->state:MUSIC_ERROR;
    int list_x=24,list_y=portrait?282:104,list_w=portrait?width-48:width-310;
    int info_x=portrait?24:width-262,info_y=portrait?94:104,info_w=portrait?width-48:238;
    canvas_round_box(c,info_x,info_y,info_w,portrait?164:276,14,2);
    canvas_text(c,info_x+16,info_y+12,labels[state],&font24,1,true);
    if(s->title[0])name_text(c,info_x+16,info_y+49,info_w-32,s->title);
    char text[80];snprintf(text,sizeof(text),"%02u:%02u",(unsigned)s->elapsed_seconds/60,(unsigned)s->elapsed_seconds%60);
    canvas_text(c,info_x+16,info_y+80,text,&font_bold28,1,true);
    snprintf(text,sizeof(text),"音量 %u",s->volume);canvas_text(c,info_x+(portrait?210:16),info_y+(portrait?84:126),text,&font20,1,true);
    snprintf(text,sizeof(text),"%s%s",music_format_label(s->format),s->sample_rate?"":"");
    if(s->sample_rate)snprintf(text,sizeof(text),"%s  %u Hz",music_format_label(s->format),(unsigned)s->sample_rate);
    canvas_text(c,info_x+16,info_y+(portrait?126:168),text,&font16,1,true);
    if(s->state==MUSIC_ERROR)canvas_text(c,info_x+16,info_y+(portrait?141:206),s->error,portrait?&font12:&font16,1,true);
    canvas_text(c,list_x,list_y,"SD / music",&font_bold20,1,true);
    snprintf(text,sizeof(text),"%u 首",s->count);canvas_text(c,list_x+list_w-88,list_y,text,&font20,1,true);
    if(!s->count) {
        canvas_text(c,list_x+12,list_y+70,s->state==MUSIC_NO_SD?"插入SD卡后自动读取":"把歌曲放入 music 文件夹",&font20,1,true);
        canvas_text(c,list_x+12,list_y+108,"MP3 / WAV / FLAC",&font16,1,true);
        canvas_text(c,list_x+12,list_y+136,"AAC / M4A / OGG",&font16,1,true);
    }
    unsigned rows=portrait?MUSIC_VISIBLE_ROWS:5;
    unsigned offset=!portrait&&s->rows>rows&&s->selected==s->first_row+5?1:0;
    for(unsigned row=0;row<rows&&row+offset<s->rows;++row) {
        unsigned i=row+offset;
        int y=list_y+40+(int)row*(portrait?54:46);bool selected=s->first_row+i==s->selected;
        if(selected)canvas_round_box(c,list_x,y,list_w,portrait?48:42,8,2);
        if(selected)canvas_text(c,list_x+10,y+8,">",&font20,1,true);
        name_text(c,list_x+32,y+6,list_w-46,s->names[i]);
    }
    canvas_text(c,24,height-89,"BOOT 选曲 / 双击播放暂停 / 三击离开",&font16,1,true);
    canvas_text(c,24,height-65,"PWR 上一首 / 双击音量+ / 长按音量-",&font16,1,true);
    canvas_text(c,24,height-41,"PWR三击 停止并回主页",&font16,1,true);
}
