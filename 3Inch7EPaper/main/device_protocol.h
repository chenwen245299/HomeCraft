#pragma once
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include "environment_history.h"
#include "power_status.h"

#define DEVICE_HISTORY_CHUNK 8
typedef enum { DEVICE_STATUS, DEVICE_HISTORY, DEVICE_PAGE, DEVICE_ROTATE, DEVICE_REFRESH, DEVICE_SAMPLE, DEVICE_HARNESS } device_operation_t;
typedef struct {
    bool sensor_valid, auto_rotate;
    float temperature,humidity;
    int64_t at;
    unsigned page,rotation,sequence;
} device_frame_t;
typedef struct {
    device_operation_t operation;
    unsigned value;
} device_command_t;
typedef struct {
    bool sensor_valid, auto_rotate, paused, busy, partial;
    float temperature, humidity;
    int64_t sampled_at;
    uint64_t sampled_ms, uptime_ms;
    unsigned page, rotation, pages;
    power_status_t power;
    device_frame_t frame;
    uint32_t sample_id;
    unsigned sample_result;
} device_snapshot_t;

const char *device_page_name(unsigned page);
bool device_command_decode(const char *text, device_command_t *out);
// Each plaintext response fits a single encrypted ATT value (<=480 bytes).
bool device_status_encode(const device_snapshot_t *s, const environment_history_t *h, char out[481]);
bool device_history_encode(const environment_history_t *h, unsigned offset, char out[481]);
