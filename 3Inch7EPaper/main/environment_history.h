#pragma once
#include <stdbool.h>
#include <stdint.h>
#define ENV_HISTORY_BUCKETS 96
typedef struct {
    bool mounted, clock_ready, write_ok;
    int day;
    unsigned observed, valid_samples, stored_samples, revision;
    int64_t last_saved_at;
    int last_error;
    bool valid[ENV_HISTORY_BUCKETS];
    int16_t temperature[ENV_HISTORY_BUCKETS]; // 0.01 C, fifteen-minute mean.
    uint16_t humidity[ENV_HISTORY_BUCKETS]; // 0.01 %RH.
    float temperature_min, temperature_max, humidity_min, humidity_max;
} environment_history_t;

