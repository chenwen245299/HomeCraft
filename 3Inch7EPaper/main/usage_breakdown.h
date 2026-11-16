#pragma once
#include "usage.h"
typedef struct { char label[33]; int64_t tokens; } usage_rank_t;
typedef struct {
    bool received;
    int day;
    int64_t fetched_at, total;
    unsigned model_count, provider_count;
    uint8_t model_rows, provider_rows;
    usage_rank_t models[3], providers[3];
} usage_breakdown_t;
bool usage_breakdown_decode(const char *text, usage_breakdown_t *out);
bool usage_breakdown_matches(const usage_breakdown_t *detail, const usage_record_t *record);
