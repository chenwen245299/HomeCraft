#pragma once
#include <stdbool.h>
#include <stdint.h>

typedef enum {
    SAMPLE_PENDING=0, SAMPLE_UPDATED=1, SAMPLE_UNCHANGED=2,
    SAMPLE_SENSOR_ERROR=3, SAMPLE_DISPLAY_ERROR=4
} sample_result_t;
typedef struct {
    uint32_t id;
    sample_result_t result;
    uint64_t requested_ms;
    bool waiting, needs_draw;
} sample_refresh_t;

void sample_refresh_begin(sample_refresh_t *s,uint32_t id,uint64_t now_ms);
// Returns true once a fresh valid sample is ready for immediate rendering.
bool sample_refresh_observe(sample_refresh_t *s,uint32_t observed_id,uint64_t sampled_ms,bool valid,uint64_t now_ms);
void sample_refresh_drawn(sample_refresh_t *s,bool changed,bool success);
bool sample_refresh_pending(const sample_refresh_t *s);
