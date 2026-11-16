#include "dashboard.h"
#include <string.h>
#include "token_typography.h"
#include "argus_page.h"
#include "pairing_qr.h"
#include "brand_assets.h"
#include "lunar_calendar.h"
#include "quota_reset.h"
#include <math.h>
#include <stdio.h>
#include <time.h>

uint32_t dashboard_layout_key(const dashboard_state_t *s, const canvas_t *c)
{
    // Content rearrangement is safe with both complete RAM images. Only page or
    // orientation changes require a clean full frame; ordinary data stays differential.
    return 17U + s->page * 397U + c->rotation * 13U;
}

static void icon_line(canvas_t *c,int x,int y,int end_x,int end_y,int stroke,bool black);
static void centered_text(canvas_t *c, int y, const char *text, const bitmap_font_t *font, int scale)
{
    canvas_text(c, (canvas_width(c) - canvas_text_width(text, font, scale)) / 2, y, text, font, scale, true);
}

void dashboard_render_shutdown(canvas_t *c, bool failed)
{
    canvas_clear(c);
    int width = canvas_width(c), height = canvas_height(c);
    bool portrait = width < height;
    int cx = width / 2, cy = portrait ? 253 : 151;
    canvas_round_box(c, 24, 24, width - 48, height - 48, 20, 2);
    centered_text(c, portrait ? 63 : 43, "EPAPER HOME", &font_bold20, 1);

    // A sleeping e-paper terminal with a crescent moon, drawn natively at any rotation.
    canvas_round_box(c, cx - 66, cy - 39, 132, 86, 13, 3);
    canvas_round_box(c, cx - 55, cy - 28, 110, 63, 5, 1);
    icon_line(c, cx - 31, cy, cx - 20, cy + 5, 3, true);
    icon_line(c, cx - 20, cy + 5, cx - 9, cy, 3, true);
    icon_line(c, cx + 9, cy, cx + 20, cy + 5, 3, true);
    icon_line(c, cx + 20, cy + 5, cx + 31, cy, 3, true);
    canvas_fill(c, cx - 2, cy + 48, 4, 15, true);
    canvas_round_box(c, cx - 26, cy + 61, 52, 5, 2, 2);
    for (int y = -25; y <= 25; ++y) for (int x = -25; x <= 25; ++x) {
        bool moon = x*x + y*y <= 25*25 && (x-10)*(x-10) + (y+7)*(y+7) > 24*24;
        if (moon) canvas_pixel(c, cx + 87 + x, cy - 58 + y, true);
    }
    canvas_text(c, cx - 104, cy - 47, "z", &font_bold20, 1, true);
    canvas_text(c, cx - 86, cy - 69, "Z", &font_bold24, 1, true);
    centered_text(c, portrait ? 405 : 250, failed ? "关机未完成" : "已关机", failed ? &font32 : &font_shutdown64, 1);
    centered_text(c, portrait ? 505 : 335, failed ? "设备仍在运行，请松开按键后重试" : "同步与采样已停止", &font20, 1);
    int line_y = portrait ? 622 : 374;
    canvas_fill(c, cx - 110, line_y, 220, 1, true);
    centered_text(c, line_y + 19, failed ? "SD或网络忙碌时不会直接断电" : "长按 PWR 开机", &font20, 1);
    if (!failed) centered_text(c, portrait ? 695 : 430, "USB 供电时也可重新上电", &font16, 1);
}

static void environment_icon(canvas_t *c,int x,int y,bool humidity)
{
    if(!humidity) {
        canvas_round_box(c,x+8,y,9,23,4,2);
        for(int yy=-7;yy<=7;++yy) for(int xx=-7;xx<=7;++xx)
            if(xx*xx+yy*yy<=49 && xx*xx+yy*yy>=20) canvas_pixel(c,x+12+xx,y+24+yy,true);
        canvas_fill(c,x+11,y+8,3,18,true);
        canvas_fill(c,x+9,y+22,7,5,true);
        for(int i=0;i<3;++i) canvas_fill(c,x+20,y+5+i*6,4,2,true);
    } else {
        icon_line(c,x+13,y,x+2,y+17,2,true);icon_line(c,x+13,y,x+24,y+17,2,true);
        for(int yy=0;yy<=13;++yy) for(int xx=-11;xx<=11;++xx)
            if(xx*xx+(yy-2)*(yy-2)<=121 && xx*xx+(yy-2)*(yy-2)>=78) canvas_pixel(c,x+13+xx,y+17+yy,true);
        icon_line(c,x+7,y+20,x+10,y+25,2,true);
    }
}

static void reset_label(char *out, size_t size, int64_t timestamp)
{
    if (!timestamp) { snprintf(out, size, "恢复时间未知"); return; }
    time_t seconds = timestamp;
    struct tm local;
    localtime_r(&seconds, &local);
    char date[32];
    strftime(date, sizeof(date), "%m/%d %H:%M", &local);
    snprintf(out, size, "恢复 %s", date);
}

static void reset_countdown(canvas_t *c, int x, int y, int width,
                            const quota_window_t *w, bool known, bool compact)
{
    char text[80];
    quota_reset_label(text, sizeof(text), known && w->remaining_tenths >= 0 ? w->reset_at : 0, time(NULL));
    const bitmap_font_t *font = compact ? &font16 : &font20;
    if (canvas_text_width(text, font, 1) > width - 22) font = &font16;
    int cx = x + 7, cy = y + (int)font->height / 2;
    for (int yy = -7; yy <= 7; ++yy) for (int xx = -7; xx <= 7; ++xx)
        if (xx*xx + yy*yy <= 49 && xx*xx + yy*yy >= 32)
            canvas_pixel(c, cx + xx, cy + yy, true);
    icon_line(c, cx, cy, cx, cy - 4, 1, true);
    icon_line(c, cx, cy, cx + 3, cy + 2, 1, true);
    canvas_text(c, x + 22, y, text, font, 1, true);
}

// Keep the unit small so the number can use the width beside its label.
static void quota_value(canvas_t *c, int right, int center_y, int max_width,
                        int value, bool available, const bitmap_font_t *preferred)
{
    char text[16];
    bool percent = available && value >= 0;
    if (percent) snprintf(text, sizeof(text), "%.0f", value / 10.0);
    else snprintf(text, sizeof(text), "--");
    const bitmap_font_t *unit = preferred->height<=font_bold32.height?&font_bold16:&font24;
    const bitmap_font_t *options[] = {&digits96, &digits80, &digits64, &digits56, &digits48, &digits40,&font_bold32,&font_bold28,&font_bold24};
    const bitmap_font_t *font = &font_bold24;
    int unit_width = percent ? canvas_text_width("%", unit, 1) + 3 : 0;
    for (unsigned i = 0; i < sizeof(options) / sizeof(options[0]); ++i) {
        if (options[i]->height <= preferred->height &&
            canvas_text_width(text, options[i], 1) + unit_width <= max_width) {
            font = options[i]; break;
        }
    }
    int y = center_y - (int)font->height / 2;
    canvas_text(c, right - unit_width - canvas_text_width(text, font, 1), y, text, font, 1, true);
    if (percent) canvas_text(c, right - unit_width + 3,
                             y + font->baseline - unit->baseline, "%", unit, 1, true);
}

static void quota_card(canvas_t *c, int x, int y, int width, int height,
                       const quota_window_t *w, bool known, bool secondary, const char *title,
                       bool countdown)
{
    canvas_round_box(c, x, y, width, height, 12, 2);
    char label[64];
    if (w->window_mins == 10080)
        snprintf(label, sizeof(label), "周额度");
    else if (w->window_mins >= 1440 && w->window_mins % 1440 == 0)
        snprintf(label, sizeof(label), "%d天额度", w->window_mins / 1440);
    else if (w->window_mins > 0 && w->window_mins % 60 == 0)
        snprintf(label, sizeof(label), "%d小时", w->window_mins / 60);
    else if (w->window_mins > 0)
        snprintf(label, sizeof(label), "%d分钟额度", w->window_mins);
    else snprintf(label, sizeof(label), "%s", secondary ? "周额度" : "5小时");
    const bitmap_font_t *label_font = secondary ? &font28 : &font32;
    int center = height < 240 ? 74 : 110;
    canvas_text(c, x + 20, y + center - 22, title ? title : label, label_font, 1, true);
    canvas_text(c, x + width - 60, y + 10, "剩余", &font20, 1, true);
    quota_value(c, x + width - 20, y + center,
                width - 52 - canvas_text_width(title ? title : label, label_font, 1),
                w->remaining_tenths, known, secondary ? &digits64 : &digits96);
    int bar = width - 40;
    canvas_box(c, x + 20, y + height - 74, bar, 24, 1);
    if (known && w->remaining_tenths >= 0) {
        int filled = (bar - 4) * w->remaining_tenths / 1000;
        canvas_fill(c, x + 22, y + height - 72, filled, 20, true);
        for (int i = 1; i < 10; ++i)
            canvas_fill(c, x + 22 + i * (bar - 4) / 10, y + height - 72, 2, 20, false);
    }
    if (countdown) reset_countdown(c, x + 20, y + height - 38, width - 40, w, known, false);
    else {
        reset_label(label, sizeof(label), known ? w->reset_at : 0);
        canvas_text(c, x + 20, y + height - 38, label, canvas_text_width(label,&font20,1)>width-40?&font16:&font20, 1, true);
    }
}

static bool environment_valid(const dashboard_state_t *s)
{
    return s->sensor_valid && isfinite(s->temperature) && isfinite(s->humidity);
}

typedef struct {
    char numbers[2][24];
    const bitmap_font_t *font, *unit_font;
    int unit_width;
} environment_numbers_t;

static environment_numbers_t environment_numbers(const dashboard_state_t *s,int width,
                                                  const bitmap_font_t *preferred)
{
    environment_numbers_t result={.font=&font_bold16,.unit_font=&font_bold20};
    const char *units[]={"°C","%"};
    for(unsigned i=0;i<2;++i) {
        if(environment_valid(s))snprintf(result.numbers[i],sizeof(result.numbers[i]),"%.1f",(double)(i?s->humidity:s->temperature));
        else snprintf(result.numbers[i],sizeof(result.numbers[i]),"--");
        int unit_width=canvas_text_width(units[i],result.unit_font,1);
        if(unit_width>result.unit_width)result.unit_width=unit_width;
    }
    const bitmap_font_t *fonts[]={&font_bold32,&font_bold28,&font_bold24,&font_bold20,&font_bold16};
    for(unsigned f=0;f<sizeof(fonts)/sizeof(fonts[0]);++f) {
        const bitmap_font_t *font=fonts[f];
        if(font->height<=preferred->height &&
           canvas_text_width(result.numbers[0],font,1)+result.unit_width+3<=width &&
           canvas_text_width(result.numbers[1],font,1)+result.unit_width+3<=width) {
            result.font=font;break;
        }
    }
    return result;
}

static void environment_number_draw(canvas_t *c,int right,int center,
                                    const environment_numbers_t *numbers,unsigned index)
{
    int top,bottom;canvas_text_ink_bounds("0123456789",numbers->font,&top,&bottom);
    int y=center-(top+bottom)/2;
    int number_right=right-numbers->unit_width-3;
    canvas_text(c,number_right-canvas_text_width(numbers->numbers[index],numbers->font,1),
                y,numbers->numbers[index],numbers->font,1,true);
    canvas_text(c,right-numbers->unit_width,
                y+(int)numbers->font->baseline-(int)numbers->unit_font->baseline,
                index?"%":"°C",numbers->unit_font,1,true);
}

static void environment_strip(canvas_t *c, const dashboard_state_t *s, int y, int height)
{
    bool portrait = canvas_width(c) < canvas_height(c);
    int width = canvas_width(c) - 48, half = width / 2;
    canvas_box(c, 24, y, width, height, 1);
    canvas_fill(c, 24 + half, y, 1, height, true);
    environment_numbers_t numbers=environment_numbers(s,half-(portrait?40:112),&font_bold32);
    for (unsigned i = 0; i < 2; ++i) {
        int x = 44 + i * half;
        canvas_text(c, x, y + 10, i ? "湿度" : "温度", &font16, 1, true);
        environment_number_draw(c,24+(int)(i+1)*half-20,y+(portrait?82:49),&numbers,i);
    }
}

static void history_chart(canvas_t *c,const environment_history_t *h,int x,int y,int width,int height,bool humidity)
{
    canvas_round_box(c,x,y,width,height,12,2);
    environment_icon(c,x+12,y+9,humidity);
    canvas_text(c,x+49,y+6,humidity?"湿度 %RH":"温度 °C",&font24,1,true);
    canvas_text(c,x+width-110,y+14,"15分钟均值",&font12,1,true);
    float low=humidity?h->humidity_min:h->temperature_min, high=humidity?h->humidity_max:h->temperature_max;
    if(!h->valid_samples){low=humidity?0:15;high=humidity?100:35;}
    else {float pad=humidity?3:1;low=floorf(low-pad);high=ceilf(high+pad);if(humidity){if(low<0)low=0;if(high>100)high=100;}}
    if(high<=low)high=low+1;
    int left=x+48,right=x+width-20,top=y+53,bottom=y+height-39;
    char text[24];
    for(int i=0;i<=2;++i){int yy=top+i*(bottom-top)/2;canvas_fill(c,left,yy,right-left,1,true);snprintf(text,sizeof(text),"%.0f",high-i*(high-low)/2);canvas_text(c,x+8,yy-8,text,&font_bold16,1,true);}
    for(int i=0;i<=4;++i){int xx=left+i*(right-left)/4;canvas_fill(c,xx,bottom,1,5,true);snprintf(text,sizeof(text),"%02d",i*6);canvas_text(c,xx-9,bottom+8,text,&font_bold16,1,true);}
    int previous_x=0,previous_y=0;bool previous=false;
    for(unsigned i=0;i<ENV_HISTORY_BUCKETS;++i) {
        if(!h->valid[i]){previous=false;continue;}
        float value=(humidity?h->humidity[i]:h->temperature[i])/100.0f;
        int xx=left+(int)i*(right-left)/ENV_HISTORY_BUCKETS, yy=bottom-(int)lroundf((value-low)*(bottom-top)/(high-low));
        if(previous)icon_line(c,previous_x,previous_y,xx,yy,2,true);
        canvas_fill(c,xx-2,yy-2,4,4,true);previous_x=xx;previous_y=yy;previous=true;
    }
    if(!h->valid_samples)canvas_text(c,left+12,top+22,"等待当天采样",&font20,1,true);
}

static void environment_cards(canvas_t *c, const dashboard_state_t *s)
{
    bool portrait=canvas_width(c)<canvas_height(c);
    int width=portrait?210:364;
    environment_numbers_t numbers=environment_numbers(s,width-32,&font_bold32);
    for(unsigned i=0;i<2;++i) {
        int x=24+i*(width+(portrait?12:24));
        canvas_round_box(c,x,94,width,96,12,2);environment_icon(c,x+12,103,i!=0);
        canvas_text(c,x+49,99,i?"湿度":"温度",&font28,1,true);
        environment_number_draw(c,x+width-16,160,&numbers,i);
    }
    history_chart(c,&s->history,24,portrait?210:208,portrait?432:364,portrait?208:184,false);
    history_chart(c,&s->history,portrait?24:412,portrait?434:208,portrait?432:364,portrait?208:184,true);
    char status[96];
    if(!s->history.clock_ready)snprintf(status,sizeof(status),"等待蓝牙校时 · 每分钟记录");
    else if(!s->history.mounted)snprintf(status,sizeof(status),"SD 未就绪 · RAM %u 条",s->history.valid_samples);
    else snprintf(status,sizeof(status),"SD 已存 %u 条 · 每分钟记录",s->history.stored_samples);
    canvas_text(c,24,portrait?665:396,status,&font20,1,true);
}

static void help_page(canvas_t *c)
{
    static const char *keys[] = {"BOOT 单击", "BOOT 双击", "BOOT 三击", "BOOT 长按",
                                "PWR 单击", "PWR 双击", "PWR 三击", "PWR 长按"};
    static const char *actions[] = {"下一页", "同步额度并全刷", "自动旋转 / 网络页热点", "蓝牙配网开关 · 按3秒",
                                   "上一页", "旋转90度并锁定", "总览并恢复自动", "按1.2秒松手暂停 · 按3秒关机"};
    bool portrait = canvas_width(c) < canvas_height(c);
    for (unsigned i = 0; i < 8; ++i) {
        int x = portrait ? 24 : 24 + (i / 4) * 388;
        int y = 100 + (portrait ? i : i % 4) * (portrait ? 72 : 74);
        canvas_text(c, x + 8, y, keys[i], &font20, 1, true);
        canvas_text(c, x + 8, y + 29, actions[i], &font16, 1, true);
        canvas_fill(c, x, y + 61, portrait ? 432 : 364, 1, true);
    }
}

static bool record_fresh(const dashboard_state_t *s, const usage_record_t *r)
{
    int64_t age = (int64_t)time(NULL) - r->fetched_at;
    unsigned timeout = s->network.interval_seconds * 3;
    if (timeout < 900) timeout = 900;
    bool online = s->network.data_mode ? s->network.ble_link : (s->network.connected && s->network.clock_ready);
    return r->has_data && r->status == USAGE_OK && online && age >= 0 && age <= timeout;
}

static const char *record_status(const dashboard_state_t *s, const usage_record_t *r)
{
    if (!r->configured) return "未配置";
    if (r->status != USAGE_OK) return usage_status_text(r->status);
    return record_fresh(s, r) ? "已同步" : "旧数据";
}

static int calendar_day(void)
{
    time_t now=time(NULL);struct tm local;localtime_r(&now,&local);
    return (local.tm_year+1900)*10000+(local.tm_mon+1)*100+local.tm_mday;
}

static void segments(canvas_t *c,int x,int y,int width,int height,int remaining)
{
    const int gap=3, cell=(width-9*gap)/10;
    for(int i=0;i<10;++i) {
        int xx=x+i*(cell+gap);
        canvas_round_box(c,xx,y,cell,height,2,1);
        if(remaining>=0) {
            int amount=remaining-i*100;
            if(amount>100) amount=100;
            if(amount>0) canvas_fill(c,xx+2,y+2,(cell-4)*amount/100,height-4,true);
        }
    }
}

static void token_summary(canvas_t *c,const dashboard_state_t *s,int y,int width,int height)
{
    bool portrait=canvas_width(c)<canvas_height(c);
    int64_t values[3];bool known[3];int day=calendar_day();
    usage_summary_totals(s->network.providers,PROVIDER_COUNT,&s->network.hub_totals,day,values);
    for(unsigned i=0;i<3;++i)known[i]=values[i]>=0;
    int cell_width=portrait?width:width/3,cell_height=portrait?height/3:height;
    int room=cell_width-(portrait?84:24);int widths[]={room,room,room};
    const unsigned preferred[]={digits48.height,digits48.height,digits48.height};
    token_number_layout_t numbers[3];token_triple_layout(values,known,widths,preferred,numbers);
    canvas_round_box(c,20,y,width,height,12,2);
    const char *labels[]={"今日","本周","本月"};
    for(unsigned i=0;i<3;++i){
        int x=20+(portrait?0:(int)i*cell_width),yy=y+(portrait?(int)i*cell_height:0);
        if(i){if(portrait)canvas_fill(c,32,yy,width-24,1,true);else canvas_fill(c,x,y+10,1,height-20,true);}
        int top,bottom;canvas_text_ink_bounds(labels[i],&font20,&top,&bottom);
        canvas_text(c,x+12,portrait?yy+cell_height/2-(top+bottom)/2:yy+4,labels[i],&font20,1,true);
        token_number_draw(c,x+cell_width-12,portrait?yy+cell_height/2:yy+58,&numbers[i]);
    }
}

static void compact_environment(canvas_t *c,const dashboard_state_t *s,int x,int y,int width,int height)
{
    bool portrait=canvas_width(c)<canvas_height(c);
    int row_height=height/2;
    environment_numbers_t numbers=environment_numbers(s,width-(portrait?24:104),&font_bold32);
    canvas_round_box(c,x,y,width,height,12,2);
    canvas_fill(c,x+12,y+row_height,width-24,1,true);
    for(unsigned i=0;i<2;++i){
        int yy=y+(int)i*row_height;
        environment_icon(c,x+10,yy+(portrait?7:5),i!=0);
        canvas_text(c,x+45,yy+(portrait?8:9),i?"湿度":"温度",portrait?&font24:&font20,1,true);
        int center=yy+(portrait?61:22);
        environment_number_draw(c,x+width-12,center,&numbers,i);
    }
}

static void daily_usage_body(canvas_t *c,const usage_record_t *r,int x,int y,int width,int height,bool detailed)
{
    int day=calendar_day();bool columns=detailed && width>500;
    const bool known[]={r->has_data && r->token_day==day && r->daily_tokens>=0,
        r->has_data && r->token_week==usage_week_start(day) && r->weekly_tokens>=0,
        r->has_data && r->token_month==usage_month_start(day) && r->monthly_tokens>=0};
    const int64_t totals[]={r->daily_tokens,r->weekly_tokens,r->monthly_tokens};
    const char *labels[]={"今日","本周","本月"};
    int rights[3],centers[3],rooms[3];
    bool narrow=height<200;
    const bitmap_font_t *label_font=narrow?&font20:&font24;
    unsigned size=narrow?font_bold32.height:(detailed?digits80.height:digits64.height);
    unsigned preferred[]={size,size,size};
    for(unsigned i=0;i<3;++i){
        int left=x+16,right=x+width-16;
        if(columns){int third=width/3;left=x+(int)i*third+16;right=x+(int)(i+1)*third-16;
            centers[i]=y+130;rooms[i]=third-32;canvas_text(c,left,y+50,labels[i],label_font,1,true);
            if(i)canvas_fill(c,x+(int)i*third,y+50,1,124,true);
        }else{
            int center=narrow?78+(int)i*32:96+(int)i*64;
            if(detailed)center=140+(int)i*150;
            int top,bottom;canvas_text_ink_bounds(labels[i],label_font,&top,&bottom);
            canvas_text(c,left,y+center-(top+bottom)/2,labels[i],label_font,1,true);
            rights[i]=right;centers[i]=y+center;rooms[i]=width-36-canvas_text_width(labels[i],label_font,1)-8;
        }
        rights[i]=right;
    }
    token_number_layout_t numbers[3];token_triple_layout(totals,known,rooms,preferred,numbers);
    for(unsigned i=0;i<3;++i)token_number_draw(c,rights[i],centers[i],&numbers[i]);
    if(detailed && known[0] && r->usage_available){
        char text[128],input[24],output[24];token_format_compact(input,sizeof(input),r->input_tokens);token_format_compact(output,sizeof(output),r->output_tokens);
        snprintf(text,sizeof(text),"输入 %s / 输出 %s",input,output);canvas_text(c,x+16,y+height-95,text,&font20,1,true);
        snprintf(text,sizeof(text),"%lld 次调用 · 缓存已含在输入中",(long long)r->calls);canvas_text(c,x+16,y+height-54,text,&font16,1,true);
    }
}

static void codex_yesterday(canvas_t *c,const usage_record_t *r,int x,int center,int width,bool compact)
{
    int day=usage_previous_day(calendar_day());bool valid=r->has_data && r->yesterday_day==day;
    const bitmap_font_t *label=&font20;
    int top,bottom;canvas_text_ink_bounds("昨日",label,&top,&bottom);
    canvas_text(c,x+16,center-(top+bottom)/2,"昨日",label,1,true);
    int available=width-32-canvas_text_width("昨日",label,1)-10;
    const bitmap_font_t *options[]={&digits40,&font_bold32,&font_bold28,&font_bold24,&font_bold20,&font_bold16};
    const bitmap_font_t *font=&font_bold16,*unit=&font_bold12;int percentage_width=0;
    for(unsigned i=0;i<sizeof(options)/sizeof(options[0]);++i){
        if(compact && options[i]->height>font_bold24.height)continue;
        const bitmap_font_t *u=options[i]->height>=digits40.height?&font_bold20:&font_bold12;
        int tokens=canvas_text_width("999.9",options[i],1)+3+canvas_text_width("M",u,1);
        int percent=canvas_text_width("99.9",options[i],1)+3+canvas_text_width("%",u,1);
        if(tokens+10+percent<=available){font=options[i];unit=u;percentage_width=percent;break;}
    }
    if(!percentage_width)percentage_width=canvas_text_width("99.9",font,1)+3+canvas_text_width("%",unit,1);
    token_number_layout_t token=token_fixed_number(r->yesterday_tokens,valid && r->yesterday_tokens>=0,font);
    token_number_layout_t percent={.font=font,.unit_font=unit};
    if(valid && r->yesterday_week_used_tenths>=0){
        int used=r->yesterday_week_used_tenths;
        if(used%10)snprintf(percent.number,sizeof(percent.number),"%.1f",used/10.0);
        else snprintf(percent.number,sizeof(percent.number),"%d",used/10);
        strcpy(percent.unit,"%");
    }else strcpy(percent.number,"--");
    percent.width=canvas_text_width(percent.number,font,1)+(percent.unit[0]?3+canvas_text_width(percent.unit,unit,1):0);
    int right=x+width-16,token_right=right-percentage_width-10;
    token_number_draw(c,token_right,center,&token);token_number_draw(c,right,center,&percent);
    canvas_fill(c,token_right+5,center-7,1,14,true);
}

static void codex_primary(canvas_t *c,const usage_record_t *r,int x,int center,int width,bool compact)
{
    const bitmap_font_t *label=compact?&font20:&font24,*timer=&font_bold16;
    const bitmap_font_t *unit=compact?&font_bold16:&font24;
    char countdown[24],number[16];
    bool known=r->has_data && r->windows[0].remaining_tenths>=0;
    quota_reset_label(countdown,sizeof(countdown),known?r->windows[0].reset_at:0,time(NULL));
    int timer_width=canvas_text_width(countdown,timer,1);
    int label_width=canvas_text_width("5小时",label,1);
    int left=x+(compact?12:16),unit_width=canvas_text_width("%",unit,1)+3;
    int digit_right=x+width-12-unit_width;
    int group_width=compact?label_width+12+18+timer_width:label_width;
    if(!compact && group_width<18+timer_width)group_width=18+timer_width;
    // Reserve 100% at a fixed size, so changing quota never changes the typography.
    const bitmap_font_t *options[]={&digits80,&digits64,&digits56,&digits48,&digits40,&font_bold32,&font_bold28,&font_bold24};
    unsigned max_height=compact?font_bold32.height:(width<300?digits56.height:digits80.height);
    const bitmap_font_t *font=&font_bold24;
    for(unsigned i=0;i<sizeof(options)/sizeof(options[0]);++i){
        if(options[i]->height<=max_height && canvas_text_width("100",options[i],1)<=digit_right-left-group_width-12){font=options[i];break;}
    }
    int label_top,label_bottom,timer_top,timer_bottom;
    canvas_text_ink_bounds("5小时",label,&label_top,&label_bottom);
    canvas_text_ink_bounds("0123456789:-",timer,&timer_top,&timer_bottom);
    int label_height=label_bottom-label_top+1,timer_height=timer_bottom-timer_top+1;
    if(timer_height<13)timer_height=13;
    int label_ink_y=compact?center-label_height/2:center-(label_height+6+timer_height)/2;
    canvas_text(c,left,label_ink_y-label_top,"5小时",label,1,true);
    int timer_x=compact?left+label_width+12:left;
    int timer_center=compact?center:label_ink_y+label_height+6+timer_height/2;
    for(int yy=-6;yy<=6;++yy)for(int xx=-6;xx<=6;++xx)
        if(xx*xx+yy*yy<=36 && xx*xx+yy*yy>=20)canvas_pixel(c,timer_x+6+xx,timer_center+yy,true);
    icon_line(c,timer_x+6,timer_center,timer_x+6,timer_center-3,1,true);
    icon_line(c,timer_x+6,timer_center,timer_x+9,timer_center+2,1,true);
    canvas_text(c,timer_x+18,timer_center-(timer_top+timer_bottom)/2,countdown,timer,1,true);
    if(known)snprintf(number,sizeof(number),"%.0f",r->windows[0].remaining_tenths/10.0);
    else strcpy(number,"--");
    int digit_top,digit_bottom;canvas_text_ink_bounds("0123456789",font,&digit_top,&digit_bottom);
    int number_y=center-(digit_top+digit_bottom)/2;
    canvas_text(c,digit_right-canvas_text_width(number,font,1),number_y,number,font,1,true);
    if(known)canvas_text(c,digit_right+3,number_y+font->baseline-unit->baseline,"%",unit,1,true);
}

static void codex_overview_body(canvas_t *c,const usage_record_t *r,int x,int y,int width,int height)
{
    bool compact=height<210;
    if(compact){
        codex_primary(c,r,x,y+74,width,true);
        int top,bottom;canvas_text_ink_bounds("周额度",&font20,&top,&bottom);
        canvas_text(c,x+12,y+100-(top+bottom)/2,"周额度",&font20,1,true);
        quota_value(c,x+width-12,y+100,width-111,r->windows[1].remaining_tenths,r->has_data,&font_bold32);
    }else{
        int primary=height>=360?140:(height<250?90:96),weekly=height>=360?285:height-100;
        codex_primary(c,r,x,y+primary,width,false);
        canvas_text(c,x+16,y+weekly-17,"周额度",&font24,1,true);
        quota_value(c,x+width-12,y+weekly,width-114,r->windows[1].remaining_tenths,r->has_data,width>300?&digits64:&digits48);
    }
    codex_yesterday(c,r,x,y+(height>=360?420:height-35-(compact?0:10)),width,compact);
    segments(c,x+16,y+height-(compact?18:21),width-32,compact?8:12,r->has_data?r->windows[0].remaining_tenths:-1);
}

static void usage_overview(canvas_t *c, const dashboard_state_t *s)
{
    bool portrait=canvas_width(c)<canvas_height(c);
    unsigned selected[3];unsigned count=usage_overview_select(s->network.providers,PROVIDER_COUNT,calendar_day(),time(NULL),selected,3);
    int card_width=portrait?440:(count?(760-12*((int)count-1))/(int)count:760);
    int card_height=portrait?(count?(504-12*((int)count-1))/(int)count:504):260;
    const bitmap_font_t *title_options[]={&font_bold28,&font_bold24,&font_bold20,&font_bold16,&font_bold12};
    const bitmap_font_t *title_font=&font_bold12;
    for(unsigned f=0;f<sizeof(title_options)/sizeof(title_options[0]);++f){
        bool fits=true;
        for(unsigned slot=0;slot<count;++slot){
            unsigned p=selected[slot];const char *name=p==PROVIDER_EXTRA1?"DeepSeek":usage_record_name(p,&s->network.providers[p]);
            if(canvas_text_width(name,title_options[f],1)>card_width-84){fits=false;break;}
        }
        if(fits){title_font=title_options[f];break;}
    }
    if(!count) {
        canvas_round_box(c,20,portrait?72:64,portrait?440:760,portrait?504:260,14,2);
        canvas_text(c,44,portrait?160:130,"等待已启用服务的数据",&font28,1,true);
        canvas_text(c,44,portrait?210:180,"请在 Mac 助手连接设备并同步",&font20,1,true);
    }
    for(unsigned slot=0;slot<count;++slot) {
        unsigned i=selected[slot];const usage_record_t *r=&s->network.providers[i];
        int x=portrait?20:20+slot*(card_width+12),y=portrait?72+slot*(card_height+12):64;
        canvas_round_box(c,x,y,card_width,card_height,14,2);
        const char *name=usage_record_name(i,r);
        brand_title(c,i,name,x+12,y+33,44,card_width-84,title_font,true);
        if(r->status!=USAGE_OK || !record_fresh(s,r))
            canvas_text(c,x+66,y+(i==4?60:(i==PROVIDER_CODEX?44:52)),record_status(s,r),&font16,1,true);
        bool quota=r->windows[0].remaining_tenths>=0 || r->windows[1].remaining_tenths>=0;
        if(i==PROVIDER_CODEX){
            codex_overview_body(c,r,x,y,card_width,card_height);
        }else if(quota && count==1 && !portrait) {
            int half=card_width/2;
            canvas_text(c,x+16,y+118,"5小时",&font32,1,true);
            quota_value(c,x+half-24,y+140,half-130,r->windows[0].remaining_tenths,r->has_data,&digits96);
            canvas_fill(c,x+half,y+72,1,148,true);
            canvas_text(c,x+half+24,y+121,"周额度",&font28,1,true);
            quota_value(c,x+card_width-20,y+140,half-140,r->windows[1].remaining_tenths,r->has_data,&digits64);
            char reset[64];
            if (i == PROVIDER_CODEX) reset_countdown(c,x+16,y+196,half-40,&r->windows[0],r->has_data,false);
            else {reset_label(reset,sizeof(reset),r->windows[0].reset_at);canvas_text(c,x+16,y+196,reset,&font20,1,true);}
            reset_label(reset,sizeof(reset),r->windows[1].reset_at);canvas_text(c,x+half+24,y+196,reset,&font20,1,true);
            segments(c,x+16,y+239,card_width-32,12,r->windows[0].remaining_tenths);
        } else if(quota) {
            int primary_center=card_height<200?84:112;
            int weekly_center=card_height<200?138:198;
            int label_x=x+12,right=x+card_width-12;
            canvas_text(c,label_x,y+primary_center-22,"5小时",&font32,1,true);
            quota_value(c,right,y+primary_center,card_width-111,r->windows[0].remaining_tenths,r->has_data,&digits96);
            if (i == PROVIDER_CODEX)
                reset_countdown(c,x+16,y+(card_height<200?102:146),
                                card_height<200?180:card_width-32,&r->windows[0],r->has_data,card_height<200);
            canvas_text(c,x+16,y+weekly_center-17,"周额度",&font24,1,true);
            quota_value(c,right,y+weekly_center,card_width-114,r->windows[1].remaining_tenths,r->has_data,card_height<200?&digits40:(card_width>300?&digits64:&digits48));
            if(card_height>=240)segments(c,x+16,y+card_height-21,card_width-32,12,r->has_data?r->windows[0].remaining_tenths:-1);
        } else if((r->token_day==calendar_day() && r->daily_tokens>=0) || (r->token_week==usage_week_start(calendar_day()) && r->weekly_tokens>=0) || (r->token_month==usage_month_start(calendar_day()) && r->monthly_tokens>=0)) {
            daily_usage_body(c,r,x,y,card_width,card_height,false);
        } else if(r->balance_available) {
            char text[64];snprintf(text,sizeof(text),"%.2f",r->balance);
            const bitmap_font_t *font=card_height<210?&digits64:&digits80;
            if(canvas_text_width(text,font,1)>card_width-32){
                char amount[32];token_format_compact(amount,sizeof(amount),(uint64_t)llround(fabs(r->balance)));
                snprintf(text,sizeof(text),"%s%s",r->balance<0?"-":"",amount);font=&digits64;
            }
            if(canvas_text_width(text,font,1)>card_width-32)font=&digits40;
            if(canvas_text_width(text,font,1)>card_width-32)font=&font_bold32;
            canvas_text(c,x+16,y+62,"API 余额",&font28,1,true);
            canvas_text(c,x+card_width-16-canvas_text_width(text,font,1),y+(card_height<210?80:100),text,font,1,true);
            canvas_text(c,x+16,y+card_height-43,r->currency,&font24,1,true);
        }
    }
    token_summary(c,s,portrait?588:340,portrait?278:548,portrait?174:88);
    compact_environment(c,s,portrait?310:580,portrait?588:340,portrait?150:200,portrait?174:88);
}

static void harness_page(canvas_t *c,const dashboard_state_t *s)
{
    const usage_record_t *r=&s->network.providers[PROVIDER_EXTRA1];
    bool portrait=canvas_width(c)<canvas_height(c);int width=canvas_width(c)-48,top=92,day=calendar_day();
    int height=portrait?220:130;canvas_round_box(c,24,top,width,height,12,2);
    const char *labels[]={"今日","本周","本月"};
    int64_t totals[]={r->daily_tokens,r->weekly_tokens,r->monthly_tokens};
    bool known[]={r->token_day==day && r->daily_tokens>=0,r->token_week==usage_week_start(day) && r->weekly_tokens>=0,
        r->token_month==usage_month_start(day) && r->monthly_tokens>=0};
    int room=portrait?width-110:width/3-32;int rooms[]={room,room,room};
    unsigned preferred[]={digits64.height,digits64.height,digits64.height};token_number_layout_t numbers[3];
    token_triple_layout(totals,known,rooms,preferred,numbers);
    for(unsigned i=0;i<3;++i){
        int x=portrait?24:24+(int)i*(width/3),y=portrait?top+(int)i*(height/3):top;
        if(i){if(portrait)canvas_fill(c,36,y,width-24,1,true);else canvas_fill(c,x,y+12,1,height-24,true);}
        int center=portrait?y+height/6:y+82;
        int ink_top,ink_bottom;canvas_text_ink_bounds(labels[i],&font20,&ink_top,&ink_bottom);
        canvas_text(c,x+16,portrait?center-(ink_top+ink_bottom)/2:y+12,labels[i],&font20,1,true);
        token_number_draw(c,portrait?24+width-16:x+width/3-16,center,&numbers[i]);
    }
    unsigned count=s->network.harness.count;int y=top+(portrait?238:149);
    int name_room=portrait?124:316,first=32+name_room,col=(canvas_width(c)-32-first)/3;
    const bitmap_font_t *heading=count<=4?&font20:&font16;
    canvas_text(c,32,y,"设备",heading,1,true);
    for(unsigned i=0;i<3;++i)canvas_text(c,first+(int)i*col+(col-canvas_text_width(labels[i],heading,1))/2,y,labels[i],heading,1,true);
    y+=count<=4?36:28;int row=count?(canvas_height(c)-48-y)/(int)count:40;if(row>100)row=100;
    const bitmap_font_t *name_font=row>=65?&font_bold28:(row>=40?&font_bold24:(row>=28?&font_bold20:&font_bold16));
    unsigned number_height=row>=65?48:(row>=48?40:(row>=36?32:24));
    for(unsigned i=0;i<count;++i){
        const harness_device_t *d=&s->network.harness.devices[i];int center=y+row/2;
        char name[26];snprintf(name,sizeof(name),"%s",d->name);const bitmap_font_t *fitted=name_font;
        if(canvas_text_width(name,fitted,1)>name_room-10)fitted=&font_bold24;
        if(canvas_text_width(name,fitted,1)>name_room-10)fitted=&font_bold20;
        while(strlen(name)>3 && canvas_text_width(name,fitted,1)>name_room-10)name[strlen(name)-1]=0;
        canvas_text(c,32,center-(int)fitted->height/2,name,fitted,1,true);
        int64_t values[]={d->daily,d->weekly,d->monthly};
        bool available[]={d->day==day,d->week==usage_week_start(day),d->month==usage_month_start(day) && d->monthly>=0};
        int widths[]={col-10,col-10,col-10};unsigned sizes[]={number_height,number_height,number_height};token_number_layout_t row_numbers[3];
        token_triple_layout(values,available,widths,sizes,row_numbers);
        for(unsigned j=0;j<3;++j)token_number_draw(c,first+(int)(j+1)*col-5,center,&row_numbers[j]);
        y+=row;
    }
    canvas_text(c,24,canvas_height(c)-28,portrait?"已同步统计 · BOOT 翻页":"已同步统计 · BOOT 翻页 / 双击同步",&font20,1,true);
}

static void provider_page(canvas_t *c, const dashboard_state_t *s, unsigned provider)
{
    bool portrait = canvas_width(c) < canvas_height(c);
    const usage_record_t *r = &s->network.providers[provider];
    char text[128];
    if(provider==PROVIDER_ARGUS) {
        argus_page_render(c,s);return;
    }
    if(provider==PROVIDER_EXTRA1 && s->network.harness.count) {harness_page(c,s);return;}
    if(provider>PROVIDER_ARGUS) {
        int height=portrait?580:300;
        canvas_round_box(c,24,100,canvas_width(c)-48,height,12,2);
        daily_usage_body(c,r,24,100,canvas_width(c)-48,height,true);
    } else if (provider == PROVIDER_DEEPSEEK) {
        canvas_box(c, 24, 100, canvas_width(c) - 48, portrait ? 490 : 290, 1);
        canvas_text(c, 48, 118, "官方 API 可用余额", &font16, 1, true);
        if (r->balance_available) snprintf(text, sizeof(text), "%s %.2f", r->currency, r->balance);
        else snprintf(text, sizeof(text), "--");
        int scale = 3;
        while (scale > 1 && canvas_text_width(text, &font24, scale) > canvas_width(c) - 96) --scale;
        canvas_text(c, 48, portrait ? 190 : 158, text, &font24, scale, true);
        canvas_text(c, 48, portrait ? 350 : 280, "官方 API 余额", &font16, 1, true);
        canvas_text(c, 48, portrait ? 405 : 327, "用量由 Mac 加密同步", &font12, 1, true);
    } else {
        const char *titles[] = {"5小时", "周额度"};
        for (unsigned i = 0; i < 2; ++i) {
            int x = portrait ? 24 : 24 + i * 388;
            int y = portrait ? 100 + i * 302 : 100;
            quota_card(c, x, y, portrait ? 432 : 364, portrait ? 286 : 300,
                       &r->windows[i], r->has_data, i != 0, titles[i], provider == PROVIDER_CODEX && i == 0);
        }
    }
    snprintf(text, sizeof(text), "%s", record_status(s, r));
    canvas_text(c, 24, portrait ? 706 : 400, text, &font20, 1, true);
}

static void network_page(canvas_t *c, const dashboard_state_t *s)
{
    bool portrait = canvas_width(c) < canvas_height(c);
    const network_snapshot_t *n = &s->network;
    char text[100];
    if (n->data_mode) {
        if(n->data_enrollment) {
            canvas_text(c,32,108,"添加一台 Mac",&font32,1,true);
            canvas_text(c,32,164,"在 HomeCraft 中选择配对中的设备",&font16,1,true);
            if(n->data_pairing_open && n->pairing_code[0]) {
                snprintf(text,sizeof(text),"%.4s %.4s",n->pairing_code,n->pairing_code+4);
                canvas_text(c,32,218,text,portrait?&digits64:&digits80,1,true);
                canvas_text(c,32,portrait?322:316,"输入这8位数字 · 10分钟内有效",&font16,1,true);
            } else canvas_text(c,32,236,"认证完成 · 正在恢复蓝牙连接",&font24,1,true);
            canvas_text(c,32,portrait?380:362,"每台电脑只需添加一次 · 无需 USB",&font16,1,true);
            canvas_text(c,32,portrait?430:394,"保留其他 Mac 的绑定 · 不影响已有数据",&font12,1,true);
            return;
        }
        canvas_text(c,32,108,"Mac 蓝牙数据接收",&font24,1,true);
        canvas_text(c,32,156,n->data_name,&font24,1,true);
        canvas_text(c,32,205,n->ble_link?"Mac 已连接 · 加密同步中":"等待 Mac 连接",&font16,1,true);
        canvas_text(c,32,248,"额度由 Mac 获取 · 板子无需 API Key",&font12,1,true);
        canvas_text(c,32,280,"温湿度由板载 SHTC3 独立采集",&font12,1,true);
        canvas_text(c,32,312,"已添加的 Mac 自动重连 · 切换无需再次配对",&font12,1,true);
        if (n->ble_received_at) {
            time_t t=n->ble_received_at;struct tm local;localtime_r(&t,&local);
            strftime(text,sizeof(text),"最近同步 %m/%d %H:%M:%S",&local);
            canvas_text(c,32,350,text,&font12,1,true);
        }
        canvas_text(c,32,portrait?430:388,"双击 BOOT 打开配对 · 长按3秒 Wi-Fi配网",&font12,1,true);
    } else if (n->ble_active) {
        char payload[240];
        snprintf(payload, sizeof(payload), "{\"ver\":\"v1\",\"name\":\"%s\",\"username\":\"%s\",\"pop\":\"%s\",\"transport\":\"ble\",\"security\":2}", n->ble_name, EPAPER_PROVISION_USERNAME, n->ble_pop);
        int x = portrait ? 24 : 342, y = portrait ? 405 : 105;
        canvas_pairing_qr(c, portrait ? 100 : 32, 102, portrait ? 280 : 270, payload);
        canvas_text(c, x, y, "官方应用 · 蓝牙配网", &font16, 1, true);
        canvas_text(c, x, y + 37, "ESP BLE Provisioning", &font16, 1, true);
        canvas_text(c, x, y + 73, n->ble_name, &font24, 1, true);
        canvas_text(c, x, y + 119, "用户名 " EPAPER_PROVISION_USERNAME " · Security 2", &font12, 1, true);
        canvas_text(c, x, y + 148, n->ble_pop, &font12, 1, true);
        const char *phase[] = {"应用扫码后选择 Wi-Fi 并输入密码", "正在试连，请稍候…", "Wi-Fi 已连接并保存，配网即将关闭",
                               "试连失败，旧网络保留；可在应用重试", "保存失败，请重新配网"};
        canvas_text(c, x, y + 184, phase[n->ble_status < 5 ? n->ble_status : 0], &font12, 1, true);
        canvas_text(c, x, y + 217, "加密传输 · 本次配对码10分钟有效", &font12, 1, true);
        canvas_text(c, 32, portrait ? 690 : 376, n->connected ? "Wi-Fi 已连接 · 后台继续查询" : "等待配置 Wi-Fi · 校时将在联网后开始", &font12, 1, true);
    } else if (n->setup_active) {
        canvas_text(c, 32, 105, "1 连接设备热点", &font16, 1, true);
        canvas_text(c, 32, 140, n->ap_ssid, &font24, 1, true);
        canvas_text(c, 32, 190, "热点密码", &font12, 1, true);
        canvas_text(c, 32, 218, n->ap_password, &font16, 1, true);
        int x = portrait ? 32 : 420, y = portrait ? 290 : 105;
        canvas_text(c, x, y, "2 打开配置网页", &font16, 1, true);
        canvas_text(c, x, y + 38, "http://192.168.4.1", &font16, 1, true);
        canvas_text(c, x, y + 86, "网页配网码", &font12, 1, true);
        canvas_text(c, x, y + 115, n->setup_code, &font24, 1, true);
        canvas_text(c, 32, portrait ? 500 : 290, "保存 Wi-Fi 和账户后，设备自动查询", &font12, 1, true);
        canvas_text(c, 32, portrait ? 535 : 320, "仅支持2.4 GHz · 配网窗口10分钟", &font12, 1, true);
    } else {
        canvas_text(c, 32, 112, n->connected ? "Wi-Fi 已连接" : "Wi-Fi 未连接", &font24, 1, true);
        canvas_text(c, 32, 172, n->wifi_configured ? n->ssid : "尚未设置网络", &font16, 1, true);
        snprintf(text, sizeof(text), "IP: %s", n->connected ? n->ip : "--");
        canvas_text(c, 32, 215, text, &font16, 1, true);
        canvas_text(c, 32, 260, n->connected ? (n->clock_ready ? "网络时间已同步" : "已联网，正在校时") : "连接 Wi-Fi 后自动校时", &font16, 1, true);
        canvas_text(c, 32, portrait ? 350 : 305, "长按 BOOT 3秒 · 官方蓝牙配网", &font16, 1, true);
        canvas_text(c, 32, portrait ? 395 : 347, "三击 BOOT · 网页高级设置热点", &font12, 1, true);
        if (n->wifi_disconnect_reason)
            canvas_text(c, 32, portrait ? 440 : 382, "连接失败可重新配网，已保存密码不会显示", &font12, 1, true);
    }
    if (n->connected && n->setup_active) {
        snprintf(text, sizeof(text), "已连接 %s", n->ssid);
        canvas_text(c, 32, portrait ? 600 : 367, text, &font12, 1, true);
    }
}

static void icon_line(canvas_t *c, int x, int y, int end_x, int end_y, int stroke, bool black)
{
    int dx=end_x>x?end_x-x:x-end_x, sx=x<end_x?1:-1;
    int dy=end_y>y?y-end_y:end_y-y, sy=y<end_y?1:-1, err=dx+dy;
    for (;;) {
        canvas_fill(c,x-stroke/2,y-stroke/2,stroke,stroke,black);
        if(x==end_x && y==end_y) break;
        int twice=err*2;
        if(twice>=dy){err+=dy;x+=sx;}
        if(twice<=dx){err+=dx;y+=sy;}
    }
}

static void bluetooth_icon(canvas_t *c, int x, int y, bool connected)
{
    icon_line(c,x+10,y,x+10,y+32,3,true);
    icon_line(c,x+10,y,x+22,y+9,3,true);
    icon_line(c,x+22,y+9,x,y+25,3,true);
    icon_line(c,x+10,y+32,x+22,y+23,3,true);
    icon_line(c,x+22,y+23,x,y+7,3,true);
    if(!connected) {
        icon_line(c,x-3,y+35,x+26,y-3,7,false);
        icon_line(c,x-3,y+35,x+26,y-3,3,true);
    }
}

static void power_indicator(canvas_t *c, const power_status_t *p, int x, int y)
{
    char text[12];
    if(p->valid && p->usb_present) {
        // External power is useful regardless of battery presence or charging completion.
        canvas_round_box(c,x+4,y+5,22,17,3,2);
        canvas_fill(c,x+8,y,3,8,true);canvas_fill(c,x+19,y,3,8,true);
        canvas_fill(c,x+13,y+22,3,6,true);
        snprintf(text,sizeof(text),"USB");
    } else {
        canvas_round_box(c,x,y+5,29,18,3,2);
        canvas_fill(c,x+29,y+10,3,8,true);
        bool known=p->valid && p->battery_present && p->percent>=0 && p->percent<=100;
        if(known) canvas_fill(c,x+3,y+8,23*p->percent/100,12,true);
        if(p->valid && p->charging) {
            icon_line(c,x+18,y+3,x+11,y+15,6,false);
            icon_line(c,x+11,y+15,x+19,y+15,6,false);
            icon_line(c,x+19,y+15,x+12,y+27,6,false);
            icon_line(c,x+18,y+3,x+11,y+15,2,true);
            icon_line(c,x+11,y+15,x+19,y+15,2,true);
            icon_line(c,x+19,y+15,x+12,y+27,2,true);
        }
        if(known) snprintf(text,sizeof(text),"%d%%",p->percent);
        else snprintf(text,sizeof(text),"--");
    }
    canvas_text(c,x+38,y-2,text,&font20,1,true);
}

void dashboard_render_power(canvas_t *c,const power_status_t *power,bool paused)
{
    int x=canvas_width(c)-104,y=canvas_width(c)<canvas_height(c)?40:20;
    canvas_fill(c,x,y-4,104,32,false);
    power_indicator(c,power,x,y);
    if(paused) {
        canvas_fill(c,canvas_width(c)-56,48,3,10,true);
        canvas_fill(c,canvas_width(c)-49,48,3,10,true);
    }
}

static void header_status(canvas_t *c, const dashboard_state_t *s, int provider)
{
    int width=canvas_width(c);bool portrait=width<canvas_height(c);
    bluetooth_icon(c,width-246,portrait?36:16,s->network.data_mode && s->network.ble_link);
    time_t now=time(NULL);struct tm local;localtime_r(&now,&local);
    char clock[16];
    if(local.tm_year>=124) {
        strftime(clock,sizeof(clock),"%H:%M",&local);
    } else snprintf(clock,sizeof(clock),"--:--");
    int clock_top,clock_bottom;canvas_text_ink_bounds(clock,&font28,&clock_top,&clock_bottom);
    canvas_text(c,width-210,(portrait?54:30)-(clock_top+clock_bottom)/2,clock,&font28,1,true);
    dashboard_render_power(c,&s->power,s->paused);
    if(local.tm_year>=124) {
        char date[16],lunar[48];strftime(date,sizeof(date),"%Y-%m-%d",&local);
        lunar_calendar_text((local.tm_year+1900)*10000+(local.tm_mon+1)*100+local.tm_mday,lunar);
        int date_x=portrait?20:width-452;
        if(!portrait && provider>=0) {
            int title_right=88+canvas_text_width(usage_record_name(provider,&s->network.providers[provider]),&font_bold32,1);
            if(date_x<title_right+12)date_x=title_right+12;
        }
        bool show_lunar=!portrait && date_x==width-452;
        int top,bottom;canvas_text_ink_bounds(date,&font24,&top,&bottom);
        canvas_text(c,date_x,(portrait?56:(show_lunar?18:34))-(top+bottom)/2,date,&font24,1,true);
        if(show_lunar) {
            canvas_text_ink_bounds(lunar,&font20,&top,&bottom);
            canvas_text(c,date_x,50-(top+bottom)/2,lunar,&font20,1,true);
        }
    }
}

static int page_provider(unsigned page)
{
    if(page>=PAGE_QUOTA && page<=PAGE_DEEPSEEK) return page-PAGE_QUOTA;
    if(page>=PAGE_ARGUS && page<=PAGE_EXTRA4) return PROVIDER_ARGUS+page-PAGE_ARGUS;
    return -1;
}

static bool page_visible(const dashboard_state_t *s,unsigned page)
{
    int p=page_provider(page);
    return p<0 || s->network.providers[p].configured || s->network.providers[p].has_data;
}
bool dashboard_page_available(const dashboard_state_t *s,unsigned page)
{
    return page<PAGE_COUNT && page_visible(s,page);
}

static const unsigned page_order[]={PAGE_OVERVIEW,PAGE_QUOTA,PAGE_CLAUDE,PAGE_DEEPSEEK,PAGE_ARGUS,PAGE_EXTRA1,PAGE_EXTRA2,PAGE_EXTRA3,PAGE_EXTRA4,PAGE_ENVIRONMENT,PAGE_MUSIC,PAGE_NETWORK,PAGE_HELP};

dashboard_page_t dashboard_step_page(const dashboard_state_t *s,int step)
{
    if(step==0)return page_visible(s,s->page)?s->page:PAGE_OVERVIEW;
    unsigned position=0;
    while(position<PAGE_COUNT && page_order[position]!=s->page)++position;
    if(position==PAGE_COUNT)position=0;
    for(unsigned n=0;n<PAGE_COUNT;++n) {
        position=(position+PAGE_COUNT+(step<0?-1:1))%PAGE_COUNT;
        unsigned page=page_order[position];
        if(page_visible(s,page))return page;
    }
    return PAGE_OVERVIEW;
}

static void footer_page(canvas_t *c,const dashboard_state_t *s,unsigned page)
{
    int width=canvas_width(c), y=canvas_height(c)-28;
    char text[16];
    unsigned total=0,index=0;
    for(unsigned i=0;i<PAGE_COUNT;++i)if(page_visible(s,page_order[i])){++total;if(page_order[i]==page)index=total;}
    snprintf(text,sizeof(text),"%u/%u",index,total);
    canvas_text(c,width-72,y,text,&font20,1,true);
}

void dashboard_render(canvas_t *c, const dashboard_state_t *s)
{
    canvas_clear(c);
    int width = canvas_width(c), height = canvas_height(c);
    bool portrait = width < height;
    static const char *titles[] = {"AI USAGE", "Codex", "Claude Code", "DeepSeek", "温湿度", "网络设置", "按键说明"};
    unsigned page = (unsigned)s->page < PAGE_COUNT ? s->page : PAGE_OVERVIEW;
    int provider=page_provider(page);
    if(provider>=0) {
        const char *title=usage_record_name(provider,&s->network.providers[provider]);
        int title_room=portrait?canvas_width(c)-96:canvas_width(c)-344;
        brand_title(c,provider,title,20,portrait?22:34,portrait?44:56,title_room,portrait?&font_bold24:&font_bold32,false);
    }else {
        canvas_text(c,20,12,page==PAGE_OVERVIEW?"AI USAGE":(page==PAGE_MUSIC?"音乐":titles[page]),&font28,1,true);
    }
    bool fresh = s->has_quota && s->quota_age_sec <= 180 && s->quota.status == 0;
    header_status(c,s,provider);
    if(page!=PAGE_OVERVIEW) canvas_fill(c,20,69,width-40,2,true);
    if(page==PAGE_MUSIC){dashboard_music_page(c,&s->music);footer_page(c,s,page);return;}
    if (page == PAGE_HELP) help_page(c);
    else if (page == PAGE_NETWORK) network_page(c, s);
    else if (page == PAGE_ENVIRONMENT) environment_cards(c, s);
    else if (s->network.started) {
        if (page == PAGE_OVERVIEW) usage_overview(c, s);
        else if(provider>=0) provider_page(c,s,provider);
    } else {
        int card_height = portrait ? (page == PAGE_OVERVIEW ? 208 : 302) : (page == PAGE_OVERVIEW ? 198 : 300);
        quota_card(c, 24, 100, portrait ? 432 : 364, card_height,
                   &s->quota.primary, s->has_quota, false, NULL, true);
        quota_card(c, portrait ? 24 : 412, portrait ? 100 + card_height + 16 : 100,
                   portrait ? 432 : 364, card_height, &s->quota.secondary, s->has_quota, true, NULL, false);
        if (page == PAGE_OVERVIEW) environment_strip(c, s, portrait ? 548 : 314, portrait ? 144 : 104);
    }
    if((provider==PROVIDER_ARGUS || (provider==PROVIDER_EXTRA1 && s->network.harness.count)) && s->network.started) {footer_page(c,s,page);return;}
    if(page==PAGE_OVERVIEW && s->network.started) {
        const char *hint=portrait?"BOOT 翻页 · 双击同步":"BOOT 翻页 / 双击同步    PWR 上一页 / 双击旋转";
        canvas_text(c,20,height-28,hint,&font20,1,true);
        footer_page(c,s,page);
        return;
    }
    char text[160];
    if (page == PAGE_HELP) snprintf(text, sizeof(text), "BOOT长按3秒配网 · PWR长按暂停");
    else if (page == PAGE_NETWORK) snprintf(text, sizeof(text), "BOOT双击配对 · 长按配网 · 三击网页");
    else snprintf(text, sizeof(text), "BOOT 翻页/双击同步 · PWR 旋转");
    canvas_text(c, 24, height - 60, text, canvas_text_width(text,&font20,1)>width-48?&font16:&font20, 1, true);
    const char *mode = s->auto_rotate ? (s->imu_valid ? "自动旋转" : "IMU离线") : "方向锁定";
    if (s->paused) snprintf(text, sizeof(text), "已暂停 · PWR长按恢复");
    else if (s->network.data_mode) snprintf(text,sizeof(text),"Mac同步 · %s",mode);
    else if (s->network.started) snprintf(text, sizeof(text), "Wi-Fi · %s", mode);
    else if (!s->has_quota) snprintf(text, sizeof(text), "等待同步 · %s", mode);
    else if (!fresh) snprintf(text, sizeof(text), "额度数据已过期 · %s", mode);
    else {
        time_t timestamp = s->quota.fetched_at;
        struct tm local;
        localtime_r(&timestamp, &local);
        char clock[32];
        strftime(clock, sizeof(clock), "%H:%M", &local);
        snprintf(text, sizeof(text), "已同步 %s · %s", clock, mode);
    }
    canvas_text(c, 24, height - 30, text, canvas_text_width(text,&font20,1)>width-196?&font16:&font20, 1, true);
    footer_page(c,s,page);
}
