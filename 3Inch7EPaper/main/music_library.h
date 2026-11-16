#pragma once
#include <stdbool.h>
#include <stddef.h>
#define MUSIC_MAX_TRACKS 128
#define MUSIC_NAME_MAX 240
#define MUSIC_VISIBLE_ROWS 6
typedef enum {MUSIC_FORMAT_NONE,MUSIC_FORMAT_MP3,MUSIC_FORMAT_WAV,MUSIC_FORMAT_FLAC,MUSIC_FORMAT_AAC,MUSIC_FORMAT_M4A,MUSIC_FORMAT_OGG} music_format_t;
typedef struct {char name[MUSIC_NAME_MAX+1];music_format_t format;} music_track_t;
typedef struct {music_track_t tracks[MUSIC_MAX_TRACKS];unsigned count,skipped;} music_library_t;
music_format_t music_file_format(const char *name);
const char *music_format_label(music_format_t format);
bool music_filename_valid(const char *name);
void music_text_copy(char *out,size_t size,const char *text);
int music_library_scan(music_library_t *library,const char *directory);
