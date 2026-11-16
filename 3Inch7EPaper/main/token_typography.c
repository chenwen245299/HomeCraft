#include "token_typography.h"
#include <stdio.h>
#include <string.h>

void token_format_compact(char *out, size_t size, uint64_t total)
{
    static const uint64_t divisors[] = {1, 1000, 1000000, 1000000000, 1000000000000ULL, 1000000000000000ULL};
    static const char *units[] = {"", "K", "M", "B", "T", "P"};
    unsigned unit = 0;
    while (unit < 5 && total >= divisors[unit + 1]) ++unit;
    // Do not let rounded 1000.0K take an extra digit instead of becoming 1.0M.
    if (unit && unit < 5) {
        bool two_decimals = unit == 3 && total < 100 * divisors[unit] - divisors[unit] / 200;
        uint64_t rounding = divisors[unit] / (two_decimals ? 200 : 20);
        if (total >= divisors[unit + 1] - rounding) ++unit;
    }
    if (!unit) snprintf(out, size, "%llu", (unsigned long long)total);
    else {
        // Keep at most four numeric digits plus the decimal point. Small B
        // values retain two decimals; 100.00B becomes 100.0B at the same font.
        bool two_decimals = unit == 3 && total < 100 * divisors[unit] - divisors[unit] / 200;
        snprintf(out, size, two_decimals ? "%.2f%s" : "%.1f%s", total / (double)divisors[unit], units[unit]);
    }
}

static token_number_layout_t at_font(int64_t total,bool known,const bitmap_font_t *font)
{
    token_number_layout_t result = {0};
    if (known && total >= 0) token_format_compact(result.number, sizeof(result.number), (uint64_t)total);
    else snprintf(result.number, sizeof(result.number), "--");
    size_t length = strlen(result.number);
    if (length && strchr("KMBTP", result.number[length - 1])) {
        result.unit[0] = result.number[length - 1];
        result.number[length - 1] = '\0';
    }
    result.font=font;
    result.unit_font=font->height>=digits40.height?&font_bold20:&font_bold12;
    result.width=canvas_text_width(result.number,font,1);
    if(result.unit[0])result.width+=3+canvas_text_width(result.unit,result.unit_font,1);
    return result;
}
static token_number_layout_t plan(int64_t total, bool known, int max_width, unsigned max_height)
{
    const bitmap_font_t *fonts[] = {&digits96,&digits80,&digits64,&digits56,&digits48,&digits40,
                                   &font_bold32,&font_bold28,&font_bold24,&font_bold20,&font_bold16,&font_bold12};
    for (unsigned i = 0; i < sizeof(fonts)/sizeof(fonts[0]); ++i) {
        const bitmap_font_t *font = fonts[i];
        if (font->height > max_height) continue;
        token_number_layout_t result=at_font(total,known,font);
        if (result.width > max_width) continue;
        return result;
    }
    // All dashboard slots are at least 120px wide; this guard also bounds future narrow callers.
    return at_font(-1,false,&font_bold12);
}
token_number_layout_t token_fixed_number(int64_t total,bool known,const bitmap_font_t *font){return at_font(total,known,font);}

void token_pair_layout(const int64_t totals[2], const bool known[2], const int widths[2],
                       const unsigned preferred_heights[2], token_number_layout_t result[2])
{
    result[0] = plan(totals[0], known[0], widths[0], preferred_heights[0]);
    unsigned secondary = preferred_heights[1];
    if (secondary >= result[0].font->height) secondary = result[0].font->height - 1;
    result[1] = plan(totals[1], known[1], widths[1], secondary);
}

void token_number_draw(canvas_t *canvas, int right, int center_y, const token_number_layout_t *layout)
{
    int ink_top,ink_bottom;canvas_text_ink_bounds("0123456789",layout->font,&ink_top,&ink_bottom);
    int top = center_y - (ink_top+ink_bottom)/2;
    canvas_text(canvas, right-layout->width, top, layout->number, layout->font, 1, true);
    if (layout->unit[0]) canvas_text(canvas, right-canvas_text_width(layout->unit,layout->unit_font,1),
        top+layout->font->baseline-layout->unit_font->baseline, layout->unit, layout->unit_font, 1, true);
}
void token_triple_layout(const int64_t totals[3],const bool known[3],const int widths[3],
                         const unsigned preferred[3],token_number_layout_t result[3])
{
    unsigned height=preferred[0];int width=widths[0];
    for(unsigned i=1;i<3;++i){if(preferred[i]<height)height=preferred[i];if(widths[i]<width)width=widths[i];}
    // A fixed worst-case compact value makes geometry choose the font once.
    // 0, unknown, K/M/B changes and longer values never resize individual rows.
    const bitmap_font_t *font=plan(999900000,true,width,height).font;
    for(unsigned i=0;i<3;++i)result[i]=at_font(totals[i],known[i],font);
}
