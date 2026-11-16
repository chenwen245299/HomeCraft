#include "refresh_policy.h"
refresh_kind_t refresh_decide(const refresh_policy_t *s,bool changed,bool forced,uint32_t layout,int64_t now)
{
    if(!forced && !changed)return REFRESH_SKIP;
    (void)now;
    if(forced || !s->ready || !s->enabled || layout!=s->layout)return REFRESH_FULL;
    return REFRESH_DIFFERENTIAL;
}
void refresh_commit(refresh_policy_t *s,refresh_kind_t kind,bool success,uint32_t layout,int64_t now)
{
    if(!success){s->ready=false;return;}
    if(kind==REFRESH_FULL){s->ready=true;s->layout=layout;s->partial_count=0;s->last_full_us=now;}
    else if(kind==REFRESH_DIFFERENTIAL)++s->partial_count;
}
