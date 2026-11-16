#include "ble_enrollment.h"
#include <stddef.h>
void ble_enrollment_begin(ble_enrollment_t *w, uint64_t now)
{
    if (w) *w=(ble_enrollment_t){.expires_at=now+600000};
}
bool ble_enrollment_claim(ble_enrollment_t *w, uint64_t now)
{
    if (!w || !w->expires_at || w->claimed || now>=w->expires_at) return false;
    w->claimed=true;
    // Leave time for the encrypted GATT response to be read before changing service.
    w->restore_at=now+5000;
    return true;
}
bool ble_enrollment_should_close(const ble_enrollment_t *w, uint64_t now)
{
    return w && w->expires_at && (w->claimed?now>=w->restore_at:now>=w->expires_at);
}
