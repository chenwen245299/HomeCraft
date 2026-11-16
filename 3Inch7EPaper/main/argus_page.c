#include "argus_page.h"
#include "token_typography.h"
#include <stdio.h>
#include <string.h>
#include <time.h>

static int today(void)
{
    time_t now=time(NULL);struct tm tm;localtime_r(&now,&tm);
    return (tm.tm_year+1900)*10000+(tm.tm_mon+1)*100+tm.tm_mday;
}

// Place visible glyph ink inside separate rows, independent of a font's ascent/leading.
static void text_at(canvas_t *c,int x,int top,const char *text,const bitmap_font_t *font)
{
    int ink_top,ink_bottom;
    if(canvas_text_ink_bounds(text,font,&ink_top,&ink_bottom))
        canvas_text(c,x,top-ink_top,text,font,1,true);
}
static void right_text(canvas_t *c,int right,int top,const char *text,const bitmap_font_t *font)
{
    text_at(c,right-canvas_text_width(text,font,1),top,text,font);
}
static const bitmap_font_t *fit_font(const char *text,int width,bool bold)
{
    const bitmap_font_t *fonts[]={bold?&font_bold28:&font20,bold?&font_bold24:&font16,
                                  bold?&font_bold20:&font12,bold?&font_bold16:&font12};
    for(unsigned i=0;i<4;++i)if(canvas_text_width(text,fonts[i],1)<=width)return fonts[i];
    return bold?&font_bold12:&font12;
}
static void fitted_label(canvas_t *c,int x,int top,int width,const char *text,const bitmap_font_t *font)
{
    char shown[160];snprintf(shown,sizeof(shown),"%s",text);
    if(canvas_text_width(shown,font,1)>width) {
        size_t n=strlen(shown);
        const int dots=canvas_text_width("...",font,1);
        while(n && canvas_text_width(shown,font,1)>width-dots) {
            do {--n;} while(n && ((unsigned char)shown[n]&0xC0)==0x80);
            shown[n]=0;
        }
        snprintf(shown+n,sizeof(shown)-n,"...");
    }
    text_at(c,x,top,shown,font);
}
static void metric(canvas_t *c,int x,int y,int width,int height,const char *title,
                   const char *value,const char *caption)
{
    fitted_label(c,x+12,y+9,width-24,title,&font16);
    text_at(c,x+12,y+31,value,fit_font(value,width-24,true));
    if(caption)fitted_label(c,x+12,y+height-18,width-24,caption,&font12);
}
static void rank_panel(canvas_t *c,int x,int y,int width,int height,const char *title,
                       const usage_rank_t *rows,unsigned length,unsigned count,int64_t total,bool known)
{
    canvas_round_box(c,x,y,width,height,12,2);
    text_at(c,x+12,y+10,title,&font20);
    if(known){char text[24];snprintf(text,sizeof(text),"%u 个",count);right_text(c,x+width-12,y+12,text,&font16);}
    if(!known || !length) {
        text_at(c,x+12,y+62,known?"当天暂无调用":"等待分类统计",&font20);return;
    }
    char tokens[3][32],percents[3][16];int token_width=0,percent_width=canvas_text_width("100%",&font16,1);
    if(length>3)length=3;
    for(unsigned i=0;i<length;++i) {
        token_format_compact(tokens[i],sizeof(tokens[i]),(uint64_t)rows[i].tokens);
        int w=canvas_text_width(tokens[i],&font_bold20,1);if(w>token_width)token_width=w;
        if(total>0 && rows[i].tokens>0 && rows[i].tokens*100<total)snprintf(percents[i],sizeof(percents[i]),"<1%%");
        else snprintf(percents[i],sizeof(percents[i]),"%.0f%%",total>0?rows[i].tokens*100.0/total:0.0);
    }
    int token_right=x+width-12-percent_width-14;
    int name_width=token_right-token_width-14-(x+12);
    for(unsigned i=0;i<length;++i) {
        int row_y=y+39+(int)i*32;
        const char *name=!strcmp(rows[i].label,"Other")?"其他":rows[i].label;
        const bitmap_font_t *name_font=canvas_text_width(name,&font20,1)<=name_width?&font20:&font16;
        fitted_label(c,x+12,row_y,name_width,name,name_font);
        right_text(c,token_right,row_y,tokens[i],&font_bold20);
        right_text(c,x+width-12,row_y+2,percents[i],&font16);
        int bar=width-24,filled=total>0?(int)((int64_t)bar*rows[i].tokens/total):0;
        if(rows[i].tokens>0 && !filled)filled=1;
        if(filled>bar)filled=bar;
        for(int tick=0;tick<bar;tick+=4)canvas_fill(c,x+12+tick,row_y+25,1,1,true);
        if(filled>0)canvas_fill(c,x+12,row_y+25,filled,3,true);
    }
}
static void number_at_bottom(canvas_t *c,int right,int bottom,const token_number_layout_t *n)
{
    int top,ink_bottom;canvas_text_ink_bounds("0123456789",n->font,&top,&ink_bottom);
    if(n->unit[0]) {
        int unit_top,unit_bottom;
        canvas_text_ink_bounds(n->unit,n->unit_font,&unit_top,&unit_bottom);
        int offset=(int)n->font->baseline-(int)n->unit_font->baseline;
        if(offset+unit_bottom>ink_bottom)ink_bottom=offset+unit_bottom;
    }
    int y=bottom-ink_bottom;
    canvas_text(c,right-n->width,y,n->number,n->font,1,true);
    if(n->unit[0])canvas_text(c,right-canvas_text_width(n->unit,n->unit_font,1),
        y+(int)n->font->baseline-(int)n->unit_font->baseline,n->unit,n->unit_font,1,true);
}
void argus_page_render(canvas_t *c,const dashboard_state_t *s)
{
    int width=canvas_width(c),height=canvas_height(c);bool portrait=width<height;
    const usage_record_t *r=&s->network.providers[PROVIDER_ARGUS];
    const usage_breakdown_t *b=&s->network.argus_breakdown;
    int day=today();bool daily=r->has_data && r->token_day==day && r->daily_tokens>=0;
    bool weekly=r->has_data && r->token_week==usage_week_start(day) && r->weekly_tokens>=0;
    bool detail=daily && usage_breakdown_matches(b,r);
    bool monthly=r->has_data && r->token_month==usage_month_start(day) && r->monthly_tokens>=0;
    const int64_t totals[]={r->daily_tokens,r->weekly_tokens,r->monthly_tokens};const bool available[]={daily,weekly,monthly};
    unsigned size=portrait?digits48.height:digits64.height;const unsigned sizes[]={size,size,size};
    int card_x[]={20,portrait?20:278,portrait?20:536};
    int card_y[]={84,portrait?160:84,portrait?236:84};
    int card_w[]={portrait?440:244,portrait?440:244,portrait?440:244};
    int card_h[]={portrait?68:116,portrait?68:116,portrait?68:116};
    int rooms[]={card_w[0]-(portrait?104:32),card_w[1]-(portrait?104:32),card_w[2]-(portrait?104:32)};
    token_number_layout_t numbers[3];token_triple_layout(totals,available,rooms,sizes,numbers);
    const char *labels[]={"今日","本周","本月"};
    for(unsigned i=0;i<3;++i){
        canvas_round_box(c,card_x[i],card_y[i],card_w[i],card_h[i],12,2);
        text_at(c,card_x[i]+16,card_y[i]+(portrait?card_h[i]/2-10:12),labels[i],&font20);
        number_at_bottom(c,card_x[i]+card_w[i]-16,card_y[i]+card_h[i]-12,&numbers[i]);
    }
    int my=portrait?312:212,mw=width-40,row_h=portrait?74:82,cols=portrait?2:4,mh=row_h*(portrait?2:1);
    canvas_round_box(c,20,my,mw,mh,12,2);
    for(int col=1;col<cols;++col)canvas_fill(c,20+col*mw/cols,my+10,1,mh-20,true);
    if(portrait)canvas_fill(c,32,my+row_h,mw-24,1,true);
    char values[4][32]={{"--"},{"--"},{"--"},{"--"}},cache[64]="";
    bool usage=daily && r->usage_available;
    if(usage){
        token_format_compact(values[0],sizeof(values[0]),(uint64_t)r->input_tokens);
        token_format_compact(values[1],sizeof(values[1]),(uint64_t)r->output_tokens);
        if(r->input_tokens>0)snprintf(values[2],sizeof(values[2]),"%.1f%%",r->cached_tokens*100.0/r->input_tokens);
        snprintf(values[3],sizeof(values[3]),"%lld",(long long)r->calls);
        char compact[24];token_format_compact(compact,sizeof(compact),(uint64_t)r->cached_tokens);
        snprintf(cache,sizeof(cache),"命中 %s",compact);
    }
    const char *titles[]={"输入 Token","输出 Token","缓存命中率","今日调用"};
    for(int i=0;i<4;++i)metric(c,20+(i%cols)*mw/cols,my+(i/cols)*row_h,mw/cols,row_h,titles[i],values[i],i==2 && usage?cache:NULL);
    rank_panel(c,20,portrait?472:306,portrait?440:374,138,"模型用量",b->models,b->model_rows,b->model_count,r->daily_tokens,detail && (b->model_count>0 || r->daily_tokens==0));
    rank_panel(c,portrait?20:406,portrait?622:306,portrait?440:374,138,"供应商用量",b->providers,b->provider_rows,b->provider_count,r->daily_tokens,detail && (b->provider_count>0 || r->daily_tokens==0));
    char footer[160],cost[56]="费用未记录";
    if(usage && r->daily_cost_available)snprintf(cost,sizeof(cost),"记录 $%.2f%s",r->daily_cost_usd,r->missing_cost_calls?"(部分)":"");
    unsigned timeout=s->network.interval_seconds*3;if(timeout<900)timeout=900;
    int64_t age=(int64_t)time(NULL)-r->fetched_at;
    const char *status=!r->configured?"未配置":(!s->network.ble_link?"已断开":(r->status!=USAGE_OK || age<0 || age>timeout?"旧数据":"已同步"));
    snprintf(footer,sizeof(footer),"%s · 当前库 / 今日 · %s",status,cost);
    const bitmap_font_t *footer_font=fit_font(footer,width-112,false);
    int ink_top,ink_bottom;canvas_text_ink_bounds(footer,footer_font,&ink_top,&ink_bottom);
    int footer_y=height-28+(int)font20.baseline-(int)footer_font->baseline;
    fitted_label(c,20,footer_y+ink_top,width-112,footer,footer_font);
}
