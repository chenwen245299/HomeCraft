#pragma once
#include <stdint.h>
#include <stdbool.h>
#define FRAME_BYTES (800*480/2)
enum {INK_BLACK=0,INK_WHITE=1,INK_YELLOW=2,INK_RED=3,INK_BLUE=5,INK_GREEN=6};
typedef struct {uint32_t codepoint,offset;uint8_t width;} glyph_t;
typedef struct {const glyph_t *glyphs;const uint8_t *pixels;unsigned count,height,baseline;} bitmap_font_t;
typedef struct {uint8_t *pixels;} color_canvas_t;
extern const bitmap_font_t text14,text16,text18,text20,text24,bold16,bold18,bold20,bold24,bold32,digits28,digits40,digits56,digits80;
// icons:begin
enum {ICON_SUN=0, ICON_PARTLY=1, ICON_CLOUD=2, ICON_FOG=3, ICON_RAIN=4, ICON_SNOW=5, ICON_THUNDER=6, ICON_BT=7, ICON_BT_OFF=8, ICON_THERMO=9, ICON_DROP=10, ICON_HOME=11, ICON_CLOCK=12,ICON_COUNT};
// icons:end
void color_pixel(color_canvas_t *c,int x,int y,uint8_t ink);
void color_fill(color_canvas_t *c,int x,int y,int w,int h,uint8_t ink);
void color_rect(color_canvas_t *c,int x,int y,int w,int h,uint8_t ink);
void color_line(color_canvas_t *c,int x1,int y1,int x2,int y2,uint8_t ink,int weight);
void color_text(color_canvas_t *c,int x,int y,const char *text,const bitmap_font_t *f,uint8_t ink);
int color_text_width(const char *text,const bitmap_font_t *f);
// True when every character has its own glyph (no '?' substitute).
bool color_font_covers(const char *text,const bitmap_font_t *f);
void color_text_fit(color_canvas_t *c,int x,int y,int maxw,const char *text,const bitmap_font_t *f,uint8_t ink);
// Like color_text_fit, but ends a cut string with an ellipsis.
void color_text_ellipsis(color_canvas_t *c,int x,int y,int maxw,const char *text,const bitmap_font_t *f,uint8_t ink);
// Wraps at character boundaries (CJK has no spaces); returns lines drawn.
int color_text_wrap(color_canvas_t *c,int x,int y,int maxw,int line_height,int max_lines,const char *text,const bitmap_font_t *f,uint8_t ink);
void color_logo(color_canvas_t *c,int provider,int x,int y);
void color_icon(color_canvas_t *c,int icon,int x,int y);
// level 0..4 bars, -1 unknown (outline only).
void color_battery(color_canvas_t *c,int x,int y,int level);
// Charging bolt inside a color_battery outline drawn at the same x,y (rows y+2..y+10, centred).
void color_bolt(color_canvas_t *c,int x,int y,uint8_t ink);
