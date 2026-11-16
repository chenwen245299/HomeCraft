#include "lunar_calendar.h"
#include "brand_assets.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "lunar_hko_fixtures.h"
static unsigned char pixels[SCREEN_BYTES];
static bool ink(int x,int y){return !(pixels[y*(SCREEN_WIDTH/8)+x/8]&(0x80>>(x%8)));}
static int center(int x0,int x1)
{
    int top=SCREEN_HEIGHT,bottom=-1;
    for(int y=0;y<90;++y)for(int x=x0;x<x1;++x)if(ink(x,y)){if(y<top)top=y;if(y>bottom)bottom=y;}
    assert(bottom>=top);return top+bottom;
}
int main(void)
{
    unsigned cases=sizeof(hko_dates)/sizeof(hko_dates[0]);
    for(unsigned i=0;i<cases;++i) {
        lunar_date_t lunar;
        if(!lunar_calendar_date(hko_dates[i][0],&lunar) || lunar.month!=hko_dates[i][1] || lunar.day!=hko_dates[i][2] || lunar.leap!=hko_dates[i][3]) {
            printf("Mismatch %d got %d/%d leap=%d expected %d/%d leap=%d\n",hko_dates[i][0],lunar.month,lunar.day,lunar.leap,hko_dates[i][1],hko_dates[i][2],hko_dates[i][3]);return 1;
        }
    }
    lunar_date_t lunar;assert(!lunar_calendar_date(20260230,&lunar));assert(!lunar_calendar_date(21010101,&lunar));
    const char *names[]={"Codex","Argus","DeepSeek Harness"};unsigned providers[]={0,3,4};
    canvas_t c={.pixels=pixels};
    static unsigned char wide_header[SCREEN_BYTES];
    for(unsigned i=0;i<3;++i) {
        canvas_clear(&c);brand_title(&c,providers[i],names[i],20,34,56,460,&font_bold32,false);
        memcpy(wide_header,pixels,sizeof(pixels));
        canvas_clear(&c);brand_title(&c,providers[i],names[i],20,34,56,120,&font_bold32,false);
        assert(memcmp(wide_header,pixels,sizeof(pixels))==0);
    }
    for(unsigned i=0;i<3;++i)for(unsigned room=120;room<=260;room+=140) {
        canvas_clear(&c);brand_title(&c,providers[i],names[i],20,34,56,room,&font_bold32,false);
        int a=center(20,76),b=center(88,350);assert(abs(a-b)<=2);
    }
    char text[48];assert(lunar_calendar_text(20261021,text));printf("HKO %u dates, leap months, invalid dates, 6 actual ink-center alignment cases OK; 2026-10-21 %s\n",cases,text);
}
