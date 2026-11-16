#include "sample_refresh.h"

void sample_refresh_begin(sample_refresh_t *s,uint32_t id,uint64_t now_ms)
{
    *s=(sample_refresh_t){.id=id,.requested_ms=now_ms,.waiting=true};
}
bool sample_refresh_observe(sample_refresh_t *s,uint32_t observed_id,uint64_t sampled_ms,bool valid,uint64_t now_ms)
{
    if(!s->waiting)return false;
    if(observed_id==s->id && sampled_ms>s->requested_ms) {
        s->waiting=false;
        if(!valid){s->result=SAMPLE_SENSOR_ERROR;return false;}
        s->needs_draw=true;return true;
    }
    if(now_ms>=s->requested_ms && now_ms-s->requested_ms>=5000) {
        s->waiting=false;s->result=SAMPLE_SENSOR_ERROR;
    }
    return false;
}
void sample_refresh_drawn(sample_refresh_t *s,bool changed,bool success)
{
    if(!s->needs_draw)return;
    s->needs_draw=false;
    s->result=!success?SAMPLE_DISPLAY_ERROR:(changed?SAMPLE_UPDATED:SAMPLE_UNCHANGED);
}
bool sample_refresh_pending(const sample_refresh_t *s)
{
    return s->waiting || s->needs_draw;
}
