#pragma once
#include <stdbool.h>
#include <stdint.h>

typedef struct {
    int remaining_tenths;  // 0..1000, -1 means unknown. Never treat unavailable as zero.
    int window_mins;
    int64_t reset_at;
} quota_window_t;

typedef struct {
    int64_t fetched_at;
    quota_window_t primary;
    quota_window_t secondary;
    int status;
} quota_snapshot_t;

bool quota_parse(const char *line, quota_snapshot_t *out);
