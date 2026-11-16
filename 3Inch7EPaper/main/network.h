#pragma once
#include <stddef.h>
#include "esp_err.h"
#include "network_types.h"
#include "device_protocol.h"

esp_err_t network_start(void);
typedef struct {
    bool initialized,enabled,synced,advertising,data_mode,enrolling,authenticated;
    unsigned connections,recoveries;
    int recovery_error;
} network_ble_status_t;
void network_get_ble_status(network_ble_status_t *out);
bool network_request_ble_recovery(void);
void network_get_snapshot(network_snapshot_t *out);
void network_request_refresh(void);
void network_toggle_setup(void);
void network_toggle_ble(void);
void network_open_data_pairing(void);
void network_publish_device(const device_snapshot_t *snapshot, const environment_history_t *history);
bool network_take_device_command(device_command_t *command);
esp_err_t network_prepare_shutdown(unsigned timeout_ms);
void network_resume(void);
// Legacy local maintenance enrollment; normal HomeCraft setup uses BLE screen-code authentication.
esp_err_t network_export_ble_pairing(char *out, size_t size);
// Accepted only during the physical setup window. Used by the portal and one-time USB provisioning.
esp_err_t network_apply_config_json(const char *json);
