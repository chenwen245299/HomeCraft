#pragma once
#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"
typedef struct { int x,y,w,h; } e1002_rect_t;
esp_err_t e1002_display_init(void);
// Native 4-bit packed palette: black0 white1 yellow2 red3 blue5 green6.
// Region is byte aligned; NULL means full update. Return only after BUSY releases.
// HomeCraft passes NULL only: on the device a region took as long as a full update and left the pixels outside it drifting.
esp_err_t e1002_display_present(const uint8_t *frame,const e1002_rect_t *region,uint32_t *elapsed_ms);
