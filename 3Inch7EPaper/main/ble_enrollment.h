#pragma once
#include <stdbool.h>
#include <stdint.h>

// Authorization window only. Cryptography stays in official Protocomm Security 2.
typedef struct {
    uint64_t expires_at, restore_at;
    bool claimed;
} ble_enrollment_t;
void ble_enrollment_begin(ble_enrollment_t *window, uint64_t now);
bool ble_enrollment_claim(ble_enrollment_t *window, uint64_t now);
bool ble_enrollment_should_close(const ble_enrollment_t *window, uint64_t now);
