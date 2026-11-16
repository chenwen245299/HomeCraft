#include "ble_recovery.h"

bool ble_recovery_due(ble_recovery_t *s,uint64_t now,bool allowed,bool advertising,unsigned connections)
{
    if(!s)return false;
    if(!allowed || advertising || connections){s->waiting=false;return false;}
    if(!s->waiting){s->waiting=true;s->idle_since=now;return false;}
    if(now<s->idle_since || now-s->idle_since<15000 || now<s->retry_after)return false;
    s->waiting=false;s->retry_after=now+60000;return true;
}
