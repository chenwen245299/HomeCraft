#pragma once
#include <stddef.h>
#include <stdint.h>
#include "esp_err.h"
void audio_output_init(void);
esp_err_t audio_output_open(unsigned rate,unsigned volume);
esp_err_t audio_output_write(const int16_t *stereo,size_t frames);
void audio_output_volume(unsigned volume);
void audio_output_close(void);
