#include "sample_refresh.h"
#include "refresh_policy.h"
#include <assert.h>
#include <stdio.h>

int main(void)
{
    sample_refresh_t s={0};
    sample_refresh_begin(&s,7,1000);
    assert(sample_refresh_pending(&s));
    assert(!sample_refresh_observe(&s,6,1010,true,1010)); // A cached/other request is not completion.
    assert(!sample_refresh_observe(&s,7,999,true,1010));
    assert(sample_refresh_observe(&s,7,1025,true,1025));
    assert(s.needs_draw && s.result==SAMPLE_PENDING);
    assert(!sample_refresh_observe(&s,7,1030,true,1030));
    refresh_policy_t policy={.ready=true,.enabled=true,.layout=5,.last_full_us=1};
    assert(refresh_decide(&policy,true,false,5,3000000)==REFRESH_DIFFERENTIAL);
    sample_refresh_drawn(&s,true,true);
    assert(s.result==SAMPLE_UPDATED && !sample_refresh_pending(&s));
    sample_refresh_begin(&s,8,2000);
    assert(sample_refresh_observe(&s,8,2025,true,2025));
    assert(refresh_decide(&policy,false,false,5,3000000)==REFRESH_SKIP);
    sample_refresh_drawn(&s,false,true);
    assert(s.result==SAMPLE_UNCHANGED);
    sample_refresh_begin(&s,9,3000);
    assert(!sample_refresh_observe(&s,9,3025,false,3025));
    assert(s.result==SAMPLE_SENSOR_ERROR && !sample_refresh_pending(&s));
    sample_refresh_begin(&s,10,4000);
    assert(!sample_refresh_observe(&s,9,8999,true,8999));
    assert(sample_refresh_pending(&s));
    assert(!sample_refresh_observe(&s,9,9000,true,9000));
    assert(s.result==SAMPLE_SENSOR_ERROR);
    sample_refresh_begin(&s,11,10000);
    assert(sample_refresh_observe(&s,11,10025,true,10025));
    sample_refresh_drawn(&s,true,false);
    assert(s.result==SAMPLE_DISPLAY_ERROR);
    puts("Manual sample: fresh transaction, unchanged pixels, partial policy, sensor/display errors and timeout passed");
}
