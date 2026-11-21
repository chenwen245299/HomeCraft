#pragma once
#include <stdbool.h>
#include <stdint.h>

#define BLE_ADV_UPSTREAM_UNITS 256U // 160ms; BLE advertising intervals use 0.625ms units.
#define BLE_ADV_IDLE_UNITS 3200U // Fixed 2s, still connectable on all default channels.

typedef struct {
    bool low_power;
    unsigned configured_ms, starts;
    int last_error;
} ble_advertising_status_t;

// Set while the transport is stopped, or after its first authenticated binding
// succeeds on an established connection (advertising has already stopped).
// Enrollment and Wi-Fi provisioning keep the upstream advertising behavior.
void ble_advertising_configure(bool paired_data_mode);
void ble_advertising_get_status(ble_advertising_status_t *out);
