#pragma once
#include "usage.h"
#include <stddef.h>
enum { HARNESS_MAX_DEVICES=8 };
typedef struct {
    char id[17], name[25];
    int64_t fetched_at;
    int day, week;
    int64_t daily, weekly, input, output, cached, calls;
    int month;
    int64_t monthly;
} harness_device_t;
typedef struct { uint32_t version, count; harness_device_t devices[HARNESS_MAX_DEVICES]; } harness_stats_t;
bool harness_stats_valid(const harness_stats_t *stats);
bool harness_stats_restore(const void *blob,size_t length,harness_stats_t *out);
bool harness_stats_update(harness_stats_t *stats, const usage_record_t *record, bool *changed);
void harness_stats_total(const harness_stats_t *stats, int day, usage_record_t *out);
bool harness_stats_encode(const harness_stats_t *stats, unsigned offset, char out[481]);
