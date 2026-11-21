#pragma once
#include <stdint.h>
#define BLE_HS_FOREVER INT32_MAX
#define BLE_GAP_CONN_MODE_UND 2
#define BLE_GAP_DISC_MODE_GEN 2
#define BLE_GAP_EVENT_CONNECT 0
#define BLE_GAP_EVENT_DISCONNECT 1
#define BLE_GAP_EVENT_ADV_COMPLETE 9
#define BLE_GAP_EVENT_MTU 15
typedef struct {uint8_t type,val[6];} ble_addr_t;
struct ble_gap_adv_params {
    uint8_t conn_mode,disc_mode;
    uint16_t itvl_min,itvl_max;
    uint8_t channel_map,filter_policy;
    uint8_t high_duty_cycle:1;
};
struct ble_gap_event {uint8_t type;struct {int status;uint16_t conn_handle;} connect;};
typedef int ble_gap_event_fn(struct ble_gap_event *event,void *arg);
int ble_gap_adv_active(void);
