#pragma once
#include <stdbool.h>

typedef struct {
    bool valid;
    bool battery_present;
    bool usb_present;
    bool charging;
    int percent; // -1: no battery, disabled gauge, or unavailable reading.
} power_status_t;

static inline bool power_source_equal(const power_status_t *a,const power_status_t *b)
{
    return a->valid==b->valid && (!a->valid ||
        (a->usb_present==b->usb_present && a->battery_present==b->battery_present && a->charging==b->charging));
}

static inline bool power_icon_equal(const power_status_t *a,const power_status_t *b)
{
    if(a->valid!=b->valid)return false;
    if(!a->valid)return true;
    if(a->usb_present!=b->usb_present)return false;
    if(a->usb_present)return true; // External power: show USB, irrespective of SOC.
    bool ak=a->battery_present && a->percent>=0 && a->percent<=100;
    bool bk=b->battery_present && b->percent>=0 && b->percent<=100;
    return ak==bk && (!ak || a->percent==b->percent);
}
