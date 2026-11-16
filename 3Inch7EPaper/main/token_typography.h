#pragma once
#include "canvas.h"
#include <stddef.h>

typedef struct {
    char number[32];
    char unit[2];
    const bitmap_font_t *font;
    const bitmap_font_t *unit_font;
    int width;
} token_number_layout_t;

void token_format_compact(char *out, size_t size, uint64_t total);
token_number_layout_t token_fixed_number(int64_t total,bool known,const bitmap_font_t *font);
// Both rows are planned together: the primary number is always larger.
void token_pair_layout(const int64_t totals[2], const bool known[2], const int widths[2],
                       const unsigned preferred_heights[2], token_number_layout_t result[2]);
void token_triple_layout(const int64_t totals[3],const bool known[3],const int widths[3],
                         const unsigned preferred_heights[3],token_number_layout_t result[3]);
void token_number_draw(canvas_t *canvas, int right, int center_y, const token_number_layout_t *layout);
