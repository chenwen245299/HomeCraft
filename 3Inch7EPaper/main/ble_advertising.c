#include "ble_advertising.h"
#include "freertos/FreeRTOS.h"
#include "host/ble_gap.h"

// Board-local link wrapper: ESP-IDF 6.1 protocomm hardcodes 160ms forever and
// exposes no advertising interval option. Do not patch the shared SDK or replace
// its GAP callback/security. Recheck this adapter when upgrading protocomm.
extern int __real_ble_gap_adv_start(uint8_t own_addr_type,const ble_addr_t *direct_addr,
    int32_t duration_ms,const struct ble_gap_adv_params *params,ble_gap_event_fn *cb,void *arg);

static portMUX_TYPE lock=portMUX_INITIALIZER_UNLOCKED;
static ble_advertising_status_t status;

void ble_advertising_configure(bool paired_data_mode)
{
    portENTER_CRITICAL(&lock);
    status.low_power=paired_data_mode;
    status.configured_ms=0;status.last_error=0;
    portEXIT_CRITICAL(&lock);
}

void ble_advertising_get_status(ble_advertising_status_t *out)
{
    if(!out)return;
    portENTER_CRITICAL(&lock);*out=status;portEXIT_CRITICAL(&lock);
}

int __wrap_ble_gap_adv_start(uint8_t own_addr_type,const ble_addr_t *direct_addr,
    int32_t duration_ms,const struct ble_gap_adv_params *params,ble_gap_event_fn *cb,void *arg)
{
    // Affect only the known, unbounded protocomm advertiser. Change the interval
    // from its first start; leave duration, security and callbacks untouched.
    bool eligible=params && cb && !direct_addr && duration_ms==BLE_HS_FOREVER &&
        params->conn_mode==BLE_GAP_CONN_MODE_UND && params->disc_mode==BLE_GAP_DISC_MODE_GEN &&
        params->itvl_min==BLE_ADV_UPSTREAM_UNITS && params->itvl_max==BLE_ADV_UPSTREAM_UNITS;
    portENTER_CRITICAL(&lock);bool enabled=status.low_power;portEXIT_CRITICAL(&lock);
    if(!enabled || !eligible)
        return __real_ble_gap_adv_start(own_addr_type,direct_addr,duration_ms,params,cb,arg);

    struct ble_gap_adv_params adjusted=*params;
    adjusted.itvl_min=adjusted.itvl_max=BLE_ADV_IDLE_UNITS;
    int result=__real_ble_gap_adv_start(own_addr_type,direct_addr,duration_ms,&adjusted,cb,arg);
    portENTER_CRITICAL(&lock);
    status.last_error=result;
    if(!result) {
        status.configured_ms=(unsigned)adjusted.itvl_min*5/8;
        ++status.starts;
    }
    portEXIT_CRITICAL(&lock);
    return result;
}
