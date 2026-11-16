#include "ble_recovery.h"
#include <assert.h>
#include <stdio.h>
int main(void)
{
    ble_recovery_t s={0};
    assert(!ble_recovery_due(&s,100,true,false,0));
    assert(!ble_recovery_due(&s,15099,true,false,0));
    assert(ble_recovery_due(&s,15100,true,false,0));
    assert(!ble_recovery_due(&s,15101,true,false,0));
    assert(!ble_recovery_due(&s,75100,true,true,0));
    assert(!ble_recovery_due(&s,75101,true,false,0));
    assert(ble_recovery_due(&s,90101,true,false,0));
    // A new live connection interrupts an idle countdown and must never restart.
    s=(ble_recovery_t){0};assert(!ble_recovery_due(&s,1,true,false,0));
    assert(!ble_recovery_due(&s,16001,true,false,1));
    assert(!ble_recovery_due(&s,32001,true,false,1));
    assert(!ble_recovery_due(&s,32002,true,false,0));
    assert(ble_recovery_due(&s,47002,true,false,0));
    // First-enrollment windows, lockout, Wi-Fi provisioning and shutdown set allowed=false.
    for(unsigned reason=0;reason<4;++reason){
        s=(ble_recovery_t){0};assert(!ble_recovery_due(&s,1,true,false,0));
        assert(!ble_recovery_due(&s,20000,false,false,0));
        assert(!ble_recovery_due(&s,30000,true,false,0));
        assert(ble_recovery_due(&s,45000,true,false,0));
    }
    assert(!ble_recovery_due(NULL,0,true,false,0));
    puts("BLE recovery: idle grace, cooldown, advertising, active client and authorization-window boundaries passed");
}
