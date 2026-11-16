#include "canvas.h"
#include <string.h>

void canvas_clear(canvas_t *c) { memset(c->pixels, 0xFF, SCREEN_BYTES); }
int canvas_width(const canvas_t *c) { return c->rotation % 180 ? SCREEN_HEIGHT : SCREEN_WIDTH; }
int canvas_height(const canvas_t *c) { return c->rotation % 180 ? SCREEN_WIDTH : SCREEN_HEIGHT; }

void canvas_pixel(canvas_t *c, int x, int y, bool black)
{
    if (x < 0 || y < 0 || x >= canvas_width(c) || y >= canvas_height(c)) return;
    int old_x = x;
    switch (c->rotation) {
        case 90: x = SCREEN_WIDTH - 1 - y; y = old_x; break;
        case 180: x = SCREEN_WIDTH - 1 - x; y = SCREEN_HEIGHT - 1 - y; break;
        case 270: x = y; y = SCREEN_HEIGHT - 1 - old_x; break;
        default: break;
    }
    uint8_t *p = &c->pixels[y * (SCREEN_WIDTH / 8) + x / 8];
    uint8_t mask = 0x80 >> (x % 8);
    if (black) *p &= ~mask; else *p |= mask;
}

void canvas_fill(canvas_t *c, int x, int y, int width, int height, bool black)
{
    for (int yy = y; yy < y + height; ++yy)
        for (int xx = x; xx < x + width; ++xx) canvas_pixel(c, xx, yy, black);
}

void canvas_box(canvas_t *c, int x, int y, int width, int height, int stroke)
{
    canvas_fill(c, x, y, width, stroke, true);
    canvas_fill(c, x, y + height - stroke, width, stroke, true);
    canvas_fill(c, x, y, stroke, height, true);
    canvas_fill(c, x + width - stroke, y, stroke, height, true);
}

void canvas_round_box(canvas_t *c, int x, int y, int width, int height, int radius, int stroke)
{
    for(int yy=0;yy<height;++yy) for(int xx=0;xx<width;++xx) {
        int dx=xx<radius?radius-xx:(xx>=width-radius?xx-(width-radius-1):0);
        int dy=yy<radius?radius-yy:(yy>=height-radius?yy-(height-radius-1):0);
        if(dx*dx+dy*dy>radius*radius) continue;
        bool edge=xx<stroke||yy<stroke||xx>=width-stroke||yy>=height-stroke;
        if(dx&&dy) edge=dx*dx+dy*dy>=(radius-stroke)*(radius-stroke);
        if(edge) canvas_pixel(c,x+xx,y+yy,true);
    }
}

static uint32_t next_char(const char **s)
{
    const uint8_t *p = (const uint8_t *)*s;
    uint32_t value = *p++;
    unsigned more = 0;
    if (value >= 0xC2 && value <= 0xDF) { value &= 0x1F; more = 1; }
    else if (value >= 0xE0 && value <= 0xEF) { value &= 0x0F; more = 2; }
    else if (value >= 0xF0 && value <= 0xF4) { value &= 7; more = 3; }
    else if (value >= 0x80) value = '?';
    for (unsigned i = 0; i < more; ++i) {
        if ((*p & 0xC0) != 0x80) { value = '?'; break; }
        value = (value << 6) | (*p++ & 0x3F);
    }
    *s = (const char *)p;
    return value;
}

static const glyph_t *find_glyph(const bitmap_font_t *font, uint32_t code)
{
    const glyph_t *fallback = NULL;
    for (unsigned i = 0; i < font->count; ++i) {
        if (font->glyphs[i].codepoint == code) return &font->glyphs[i];
        if (font->glyphs[i].codepoint == '?') fallback = &font->glyphs[i];
    }
    return fallback;
}

int canvas_text_width(const char *text, const bitmap_font_t *font, int scale)
{
    int width = 0;
    while (*text) {
        const glyph_t *g = find_glyph(font, next_char(&text));
        if (g) width += (g->width + 1) * scale;
    }
    return width;
}

bool canvas_text_ink_bounds(const char *text,const bitmap_font_t *font,int *top,int *bottom)
{
    *top=(int)font->height;*bottom=-1;
    while(*text) {
        const glyph_t *g=find_glyph(font,next_char(&text));if(!g)continue;
        const uint8_t *pixels=font->pixels+g->offset;unsigned stride=(g->width+7)/8;
        for(unsigned y=0;y<font->height;++y)for(unsigned x=0;x<g->width;++x)
            if(pixels[y*stride+x/8]&(0x80>>(x%8))){if((int)y<*top)*top=y;if((int)y>*bottom)*bottom=y;}
    }
    return *bottom>=*top;
}

void canvas_text(canvas_t *c, int x, int y, const char *text,
                 const bitmap_font_t *font, int scale, bool black)
{
    if (scale < 1) return;
    while (*text) {
        const glyph_t *g = find_glyph(font, next_char(&text));
        if (!g) continue;
        const uint8_t *pixels = font->pixels + g->offset;
        int stride = (g->width + 7) / 8;
        for (unsigned yy = 0; yy < font->height; ++yy)
            for (unsigned xx = 0; xx < g->width; ++xx)
                if (pixels[yy * stride + xx / 8] & (0x80 >> (xx % 8)))
                    canvas_fill(c, x + xx * scale, y + yy * scale, scale, scale, black);
        x += (g->width + 1) * scale;
    }
}
