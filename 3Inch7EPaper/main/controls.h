#pragma once
#include <stdbool.h>
#include <stdint.h>

typedef enum { GESTURE_NONE, GESTURE_SINGLE, GESTURE_DOUBLE, GESTURE_TRIPLE, GESTURE_LONG, GESTURE_SHUTDOWN } gesture_t;
typedef struct {
    bool raw_down, down, long_sent;
    unsigned clicks;
    uint64_t changed_ms, pressed_ms, released_ms, ignore_until_ms;
} button_state_t;
gesture_t button_update(button_state_t *b, bool raw_down, uint64_t now_ms);
gesture_t button_update_with_hold(button_state_t *b, bool raw_down, uint64_t now_ms, unsigned hold_ms);
typedef struct { button_state_t button; bool pause_pending, shutdown_sent; } power_button_t;
// Release after 1.2s to pause; hold 3s for shutdown, without an intermediate pause.
gesture_t power_button_update(power_button_t *b, bool raw_down, uint64_t now_ms);

typedef struct {
    bool initialized;
    float x, y, z;
    unsigned rotation, candidate;
    uint64_t candidate_since_ms, sampled_ms;
} orientation_filter_t;
bool orientation_update(orientation_filter_t *f, float x, float y, float z, uint64_t now_ms);
