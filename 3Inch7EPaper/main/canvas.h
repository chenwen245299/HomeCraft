#pragma once
#include <stdbool.h>
#include <stdint.h>
#define SCREEN_WIDTH 800
#define SCREEN_HEIGHT 480
#define SCREEN_BYTES (SCREEN_WIDTH * SCREEN_HEIGHT / 8)

typedef struct {
    uint32_t codepoint;
    uint32_t offset;
    uint8_t width;
} glyph_t;
typedef struct {
    const glyph_t *glyphs;
    const uint8_t *pixels;
    unsigned count;
    unsigned height;
    unsigned baseline;
} bitmap_font_t;
typedef struct { uint8_t *pixels; unsigned rotation; } canvas_t;
extern const bitmap_font_t font12, font16, font24;
extern const bitmap_font_t font20, font28, font32, digits40, digits48, digits56, digits64, digits80, digits96;
extern const bitmap_font_t font_bold12, font_bold16, font_bold20, font_bold24, font_bold28, font_bold32;
extern const bitmap_font_t font_shutdown64;

int canvas_width(const canvas_t *c);
int canvas_height(const canvas_t *c);
void canvas_clear(canvas_t *c);
void canvas_pixel(canvas_t *c, int x, int y, bool black);
void canvas_fill(canvas_t *c, int x, int y, int width, int height, bool black);
void canvas_box(canvas_t *c, int x, int y, int width, int height, int stroke);
void canvas_round_box(canvas_t *c, int x, int y, int width, int height, int radius, int stroke);
int canvas_text_width(const char *text, const bitmap_font_t *font, int scale);
bool canvas_text_ink_bounds(const char *text,const bitmap_font_t *font,int *top,int *bottom);
void canvas_text(canvas_t *c, int x, int y, const char *text,
                 const bitmap_font_t *font, int scale, bool black);
