#include "color_canvas.h"
#include <stdlib.h>
#include <string.h>
void color_pixel(color_canvas_t *c,int x,int y,uint8_t ink){if(x<0||y<0||x>=800||y>=480)return;uint8_t *p=&c->pixels[y*400+x/2];if(x&1)*p=(*p&0xF0)|ink;else *p=(*p&0x0F)|(ink<<4);}
void color_fill(color_canvas_t *c,int x,int y,int w,int h,uint8_t ink){for(int yy=y;yy<y+h;++yy)for(int xx=x;xx<x+w;++xx)color_pixel(c,xx,yy,ink);}
void color_rect(color_canvas_t *c,int x,int y,int w,int h,uint8_t ink){color_fill(c,x,y,w,1,ink);color_fill(c,x,y+h-1,w,1,ink);color_fill(c,x,y,1,h,ink);color_fill(c,x+w-1,y,1,h,ink);}
void color_line(color_canvas_t *c,int x1,int y1,int x2,int y2,uint8_t ink,int weight){int dx=abs(x2-x1),sx=x1<x2?1:-1,dy=-abs(y2-y1),sy=y1<y2?1:-1,err=dx+dy;for(;;){color_fill(c,x1,y1,weight,weight,ink);if(x1==x2&&y1==y2)break;int e=2*err;if(e>=dy){err+=dy;x1+=sx;}if(e<=dx){err+=dx;y1+=sy;}}}
static uint32_t next(const char **s){const uint8_t *p=(const uint8_t *)*s;uint32_t n=*p++;int more=0;if(n>=0xC2&&n<=0xDF){n&=31;more=1;}else if(n>=0xE0&&n<=0xEF){n&=15;more=2;}else if(n>=0xF0&&n<=0xF4){n&=7;more=3;}else if(n>=128)n='?';for(int i=0;i<more;++i){if((*p&0xC0)!=0x80){n='?';break;}n=(n<<6)|(*p++&63);}*s=(const char *)p;return n;}
// Glyph tables are generated sorted by codepoint; missing characters fall back to '?'.
static const glyph_t *find(const bitmap_font_t *f,uint32_t n){unsigned lo=0,hi=f->count;while(lo<hi){unsigned mid=(lo+hi)/2;if(f->glyphs[mid].codepoint<n)lo=mid+1;else hi=mid;}return lo<f->count&&f->glyphs[lo].codepoint==n?&f->glyphs[lo]:NULL;}
static const glyph_t *glyph(const bitmap_font_t *f,uint32_t n){const glyph_t *g=find(f,n);return g?g:find(f,'?');}
static void draw(color_canvas_t *c,int x,int y,const glyph_t *g,const bitmap_font_t *f,uint8_t ink){unsigned stride=(g->width+7)/8;for(unsigned yy=0;yy<f->height;++yy)for(unsigned xx=0;xx<g->width;++xx)if(f->pixels[g->offset+yy*stride+xx/8]&(0x80>>(xx&7)))color_pixel(c,x+xx,y+yy,ink);}
bool color_font_covers(const char *s,const bitmap_font_t *f){while(*s)if(!find(f,next(&s)))return false;return true;}
int color_text_width(const char *s,const bitmap_font_t *f){int w=0;while(*s){const glyph_t *g=glyph(f,next(&s));if(g)w+=g->width+1;}return w;}
void color_text(color_canvas_t *c,int x,int y,const char *s,const bitmap_font_t *f,uint8_t ink){while(*s){const glyph_t *g=glyph(f,next(&s));if(!g)continue;draw(c,x,y,g,f,ink);x+=g->width+1;}}
void color_text_fit(color_canvas_t *c,int x,int y,int maxw,const char *s,const bitmap_font_t *f,uint8_t ink){while(*s){const glyph_t *g=glyph(f,next(&s));if(!g)continue;if(g->width+1>maxw)break;draw(c,x,y,g,f,ink);x+=g->width+1;maxw-=g->width+1;}}
void color_text_ellipsis(color_canvas_t *c,int x,int y,int maxw,const char *s,const bitmap_font_t *f,uint8_t ink){
 if(color_text_width(s,f)<=maxw){color_text(c,x,y,s,f,ink);return;}
 const glyph_t *dots=glyph(f,0x2026);int reserve=dots?dots->width+1:0;
 while(*s){const glyph_t *g=glyph(f,next(&s));if(!g)continue;if(g->width+1>maxw-reserve)break;draw(c,x,y,g,f,ink);x+=g->width+1;maxw-=g->width+1;}
 if(dots)draw(c,x,y,dots,f,ink);
}
static bool closing(uint32_t n){return n==0xFF0C||n==0x3002||n==0x3001||n==0xFF1B||n==0xFF1A||n==0xFF1F||n==0xFF01||n==0x201D||n==0xFF09||n==0x300B||n==0x2026;}
static bool word(uint32_t n){return (n>='0'&&n<='9')||(n>='A'&&n<='Z')||(n>='a'&&n<='z');}
// Bytes of `s` that fit in one line of maxw: CJK breaks anywhere, Latin words move whole when possible,
// and closing punctuation may hang past the edge.
static const char *line_end(const char *s,int maxw,const bitmap_font_t *f){
 const char *p=s,*last_break=NULL;int w=0;uint32_t prev=0;
 while(*p){const char *at=p;uint32_t n=next(&p);const glyph_t *g=glyph(f,n);if(!g)continue;
  if(at>s&&!(word(prev)&&word(n)))last_break=at;
  if(w+g->width>maxw&&at>s&&!closing(n))return word(n)&&last_break&&last_break>s?last_break:at;
  w+=g->width+1;prev=n;}
 return p;
}
int color_text_wrap(color_canvas_t *c,int x,int y,int maxw,int line_height,int max_lines,const char *s,const bitmap_font_t *f,uint8_t ink){
 int line=0;char buf[256];
 while(*s&&line<max_lines){
  while(*s==' ')++s;
  const char *end=line_end(s,maxw,f);size_t n=(size_t)(end-s);
  if(line==max_lines-1&&*end){color_text_ellipsis(c,x,y+line*line_height,maxw,s,f,ink);return line+1;}
  if(n>=sizeof(buf))n=sizeof(buf)-1;
  memcpy(buf,s,n);buf[n]=0;color_text(c,x,y+line*line_height,buf,f,ink);
  s=end;++line;
 }
 return line;
}
void color_battery(color_canvas_t *c,int x,int y,int level){
 color_rect(c,x,y,24,13,INK_BLACK);color_fill(c,x+24,y+4,2,5,INK_BLACK);
 if(level<0)return;
 uint8_t ink=level>=3?INK_GREEN:level==2?INK_YELLOW:INK_RED;
 for(int i=0;i<level&&i<4;++i)color_fill(c,x+2+i*5,y+2,4,9,ink);
 if(level==0)color_fill(c,x+2,y+2,2,9,INK_RED);
}
void color_bolt(color_canvas_t *c,int x,int y,uint8_t ink){
 // Rows y+2..y+10 as [first,last] columns: down-left stroke, crossbar, down-left stroke.
 static const int8_t rows[][2]={{12,15},{11,14},{10,13},{7,16},{10,13},{9,12},{8,11},{8,10},{8,9}};
 for(unsigned i=0;i<sizeof(rows)/sizeof(rows[0]);++i)color_fill(c,x+rows[i][0],y+2+(int)i,rows[i][1]-rows[i][0]+1,1,ink);
}
