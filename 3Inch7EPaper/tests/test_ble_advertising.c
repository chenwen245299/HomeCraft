#include "ble_advertising.h"
#include "host/ble_gap.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>

int __wrap_ble_gap_adv_start(uint8_t own,const ble_addr_t *direct,int32_t duration,
    const struct ble_gap_adv_params *params,ble_gap_event_fn *cb,void *arg);
static bool active;
static int start_result,delivered[16],callback_cookie;
static struct {
    uint8_t own;
    const ble_addr_t *direct;
    int32_t duration;
    struct ble_gap_adv_params params;
    ble_gap_event_fn *callback;
    void *arg;
} last;
static const struct ble_gap_adv_params upstream={
    .conn_mode=BLE_GAP_CONN_MODE_UND,.disc_mode=BLE_GAP_DISC_MODE_GEN,
    .itvl_min=256,.itvl_max=256,.channel_map=7,.filter_policy=0,
};
int ble_gap_adv_active(void){return active;}
int __real_ble_gap_adv_start(uint8_t own,const ble_addr_t *direct,int32_t duration,
    const struct ble_gap_adv_params *params,ble_gap_event_fn *cb,void *arg)
{
    if(active)return 2;
    if(start_result)return start_result;
    last.own=own;last.direct=direct;last.duration=duration;
    last.params=*params;last.callback=cb;last.arg=arg;active=true;
    return 0;
}
static int upstream_event(struct ble_gap_event *event,void *arg);
static int start(void)
{
    return __wrap_ble_gap_adv_start(1,NULL,BLE_HS_FOREVER,&upstream,upstream_event,&callback_cookie);
}
static int upstream_event(struct ble_gap_event *event,void *arg)
{
    assert(arg==&callback_cookie);++delivered[event->type];
    // ESP-IDF protocomm restarts on expiry, failed connection and disconnect.
    if(event->type==BLE_GAP_EVENT_ADV_COMPLETE || event->type==BLE_GAP_EVENT_DISCONNECT ||
       (event->type==BLE_GAP_EVENT_CONNECT && event->connect.status))assert(start()==0);
    return 37;
}
static void send_event(uint8_t type,int connect_status)
{
    if(type!=BLE_GAP_EVENT_MTU)active=false;
    struct ble_gap_event event={.type=type,.connect={.status=connect_status}};
    assert(last.callback(&event,last.arg)==37);
}
static void expect(uint16_t units,int32_t duration)
{
    assert(last.own==1 && !last.direct && last.duration==duration);
    assert(last.params.itvl_min==units && last.params.itvl_max==units);
    assert(last.params.channel_map==upstream.channel_map);
    assert(last.params.filter_policy==upstream.filter_policy);
    assert(last.params.conn_mode==upstream.conn_mode && last.params.disc_mode==upstream.disc_mode);
}
int main(void)
{
    // Enrollment / Wi-Fi: retain the exact duration, callback and parameters.
    ble_advertising_configure(false);assert(start()==0);
    expect(256,BLE_HS_FOREVER);assert(last.callback==upstream_event && last.arg==&callback_cookie);
    active=false;
    // Bound mode uses 2s immediately, with the original infinite duration and
    // exact callback/argument. No timer or fast-to-slow callback adapter remains.
    ble_advertising_configure(true);assert(start()==0);expect(3200,BLE_HS_FOREVER);
    assert(last.callback==upstream_event && last.arg==&callback_cookie);
    send_event(BLE_GAP_EVENT_CONNECT,1);expect(3200,BLE_HS_FOREVER);
    send_event(BLE_GAP_EVENT_ADV_COMPLETE,0);expect(3200,BLE_HS_FOREVER);
    ble_advertising_status_t s;ble_advertising_get_status(&s);
    assert(s.low_power && s.configured_ms==2000 && s.starts==3 && !s.last_error);
    // An erroneous second start cannot replace the live callback or context.
    assert(__wrap_ble_gap_adv_start(1,NULL,BLE_HS_FOREVER,&upstream,upstream_event,NULL)==2);
    send_event(BLE_GAP_EVENT_CONNECT,0);
    send_event(BLE_GAP_EVENT_MTU,0);
    assert(delivered[BLE_GAP_EVENT_MTU]==1);
    // Every subsequent reconnect uses the same fixed interval.
    send_event(BLE_GAP_EVENT_DISCONNECT,0);expect(3200,BLE_HS_FOREVER);
    send_event(BLE_GAP_EVENT_CONNECT,1);expect(3200,BLE_HS_FOREVER);
    // A new provisioning service retains its original speed.
    active=false;ble_advertising_configure(false);assert(start()==0);expect(256,BLE_HS_FOREVER);
    assert(last.callback==upstream_event);
    // First pairing can finish on the same service: enable while connected,
    // then use the new policy on its very next disconnect, without a reboot.
    send_event(BLE_GAP_EVENT_CONNECT,0);ble_advertising_configure(true);
    send_event(BLE_GAP_EVENT_DISCONNECT,0);expect(3200,BLE_HS_FOREVER);
    // Failures return unchanged and don't count a successful start.
    active=false;ble_advertising_configure(true);start_result=12;
    ble_advertising_get_status(&s);unsigned before=s.starts;
    assert(start()==12);ble_advertising_get_status(&s);
    assert(s.last_error==12 && s.starts==before && s.configured_ms==0);
    start_result=0;assert(start()==0);expect(3200,BLE_HS_FOREVER);
    // Finite, directed and different advertisers keep their own contracts.
    active=false;
    assert(__wrap_ble_gap_adv_start(1,NULL,1234,&upstream,upstream_event,&callback_cookie)==0);
    expect(256,1234);assert(last.callback==upstream_event);
    active=false;ble_addr_t peer={0};
    assert(__wrap_ble_gap_adv_start(1,&peer,BLE_HS_FOREVER,&upstream,upstream_event,&callback_cookie)==0);
    assert(last.direct==&peer && last.params.itvl_min==256 && last.callback==upstream_event);
    active=false;struct ble_gap_adv_params other=upstream;other.itvl_min=other.itvl_max=160;
    assert(__wrap_ble_gap_adv_start(1,NULL,BLE_HS_FOREVER,&other,upstream_event,&callback_cookie)==0);
    assert(last.params.itvl_min==160 && last.callback==upstream_event);
    ble_advertising_get_status(NULL);
    puts("BLE advertising: fixed 2s from first start, original callback/MTU, reconnect, failures and provisioning boundaries passed");
}
