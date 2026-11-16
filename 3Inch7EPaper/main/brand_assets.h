#pragma once
#include "canvas.h"
void brand_icon(canvas_t *canvas, unsigned provider, int x, int y, unsigned size);
void brand_title(canvas_t *c,unsigned provider,const char *name,int x,int center,unsigned size,int text_room,const bitmap_font_t *preferred,bool compact_suffix);
