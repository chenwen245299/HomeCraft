#include <assert.h>
#include <stdio.h>
#include "power_events.h"

int main(void)
{
    power_status_t battery={.valid=true,.battery_present=true,.percent=73};
    power_status_t usb=battery;usb.usb_present=true;usb.charging=true;
    power_edge_filter_t f={0};
    assert(!power_edge_observe(&f,&battery,&usb,1000));
    assert(!power_edge_observe(&f,&battery,&usb,1099));
    assert(power_edge_observe(&f,&battery,&usb,1100));
    // A short connection bounce does not replace the stable state.
    f=(power_edge_filter_t){0};
    assert(!power_edge_observe(&f,&battery,&usb,2000));
    assert(!power_edge_observe(&f,&battery,&battery,2050));
    assert(!power_edge_observe(&f,&battery,&usb,2100));
    assert(power_edge_observe(&f,&battery,&usb,2200));
    // A failed PMU read resets stability without publishing an invented source.
    power_status_t failed={0};f=(power_edge_filter_t){0};
    assert(!power_edge_observe(&f,&battery,&usb,3000));
    assert(!power_edge_observe(&f,&battery,&failed,3100));
    assert(!power_edge_observe(&f,&battery,&usb,3200));
    assert(power_edge_observe(&f,&battery,&usb,3300));
    // Unplug requires its own stable observation and preserves the reported SOC.
    f=(power_edge_filter_t){0};
    assert(!power_edge_observe(&f,&usb,&battery,4000));
    assert(power_edge_observe(&f,&usb,&battery,4100));
    assert(battery.percent==73);
    power_status_t full=usb;full.charging=false;full.percent=100;
    assert(!power_source_equal(&usb,&full));
    assert(power_icon_equal(&usb,&full)); // USB is still visibly powered after charge finishes.
    assert(!power_icon_equal(&usb,&battery));
    assert(!power_icon_equal(&battery,&(power_status_t){.valid=true,.battery_present=true,.percent=0}));
    assert(!power_icon_equal(&battery,&(power_status_t){.valid=true,.battery_present=true,.percent=-1}));
    puts("Power edges: debounce, bounce, read failure, unplug, real SOC and USB-full semantics: OK");
}
