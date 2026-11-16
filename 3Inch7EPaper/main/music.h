#pragma once
#include <stdbool.h>
#include <stdint.h>
#include "music_library.h"
typedef enum {MUSIC_NO_SD,MUSIC_SCANNING,MUSIC_EMPTY,MUSIC_STOPPED,MUSIC_PLAYING,MUSIC_PAUSED,MUSIC_ERROR} music_state_t;
typedef struct {
    uint32_t revision;
    music_state_t state;
    unsigned count,selected,first_row,rows,volume,skipped;
    uint32_t elapsed_seconds,sample_rate;
    unsigned source_bits,source_channels;
    char title[MUSIC_NAME_MAX+1],names[MUSIC_VISIBLE_ROWS][MUSIC_NAME_MAX+1];
    music_format_t format;
    char error[80];
} music_snapshot_t;
typedef enum {MUSIC_CMD_SCAN,MUSIC_CMD_TOGGLE,MUSIC_CMD_NEXT,MUSIC_CMD_PREVIOUS,MUSIC_CMD_STOP,MUSIC_CMD_VOLUME_UP,MUSIC_CMD_VOLUME_DOWN,MUSIC_CMD_PLAY_INDEX} music_command_t;
void music_start(void);
bool music_command(music_command_t command,unsigned argument);
void music_snapshot(music_snapshot_t *out);
bool music_prepare_shutdown(unsigned timeout_ms);
void music_resume(void);
