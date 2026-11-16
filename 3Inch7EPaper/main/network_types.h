#pragma once
#include "usage.h"
#include "usage_breakdown.h"
#include "harness_stats.h"
#define EPAPER_PROVISION_USERNAME "wifiprov"
typedef struct {
    bool started, wifi_configured, connected, clock_ready, setup_active, ble_active, wifi_running;
    unsigned revision, interval_seconds;
    int rssi;
    char ssid[33], ip[16], ap_ssid[32], ap_password[17], setup_code[9];
    char ble_name[20], ble_pop[33];
    unsigned ble_status; // 0 ready, 1 testing, 2 saved, 3 failed, 4 storage error
    unsigned wifi_disconnect_reason;
    bool data_mode, ble_link, data_pairing_open, data_enrollment;
    char pairing_code[9];
    int64_t ble_received_at;
    char data_name[20];
    usage_record_t providers[PROVIDER_COUNT];
    usage_totals_t hub_totals;
    usage_breakdown_t argus_breakdown;
    harness_stats_t harness;
} network_snapshot_t;
