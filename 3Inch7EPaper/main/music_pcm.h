#pragma once
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
bool music_pcm_supported(unsigned rate,unsigned bits,unsigned channels);
size_t music_pcm_stereo16(const uint8_t *source,size_t bytes,unsigned bits,unsigned channels,int16_t out[1024]);
