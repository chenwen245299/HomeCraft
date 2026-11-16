#include "ble_enrollment.h"
#include <assert.h>
#include <stdio.h>
int main(void)
{
    ble_enrollment_t w={0};
    assert(!ble_enrollment_claim(&w,1));
    ble_enrollment_begin(&w,1000);
    assert(!ble_enrollment_should_close(&w,600999));
    assert(!ble_enrollment_claim(&w,601000));
    assert(ble_enrollment_should_close(&w,601000));
    ble_enrollment_begin(&w,700000);
    assert(ble_enrollment_claim(&w,700001));
    assert(!ble_enrollment_claim(&w,700002)); // A second client cannot claim the same window.
    assert(!ble_enrollment_should_close(&w,705000));
    assert(ble_enrollment_should_close(&w,705001));
    ble_enrollment_begin(&w,800000); // Adding another Mac requires a fresh physical window.
    assert(ble_enrollment_claim(&w,800001));
    ble_enrollment_begin(&w,1000000);
    assert(ble_enrollment_claim(&w,1599999));
    assert(!ble_enrollment_should_close(&w,1600000));
    assert(!ble_enrollment_should_close(&w,1604998));
    assert(ble_enrollment_should_close(&w,1604999));
    assert(!ble_enrollment_claim(&w,1600000));
    puts("BLE enrollment expiry, single claim, response grace and fresh-window checks passed");
}
