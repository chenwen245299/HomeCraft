#include "power_events.h"

bool power_edge_observe(power_edge_filter_t *f,const power_status_t *current,
                        const power_status_t *observed,uint64_t now_ms)
{
    if(!observed->valid || power_source_equal(current,observed)) {
        f->pending=false;return false;
    }
    if(!f->pending || !power_source_equal(&f->candidate,observed) || now_ms<f->since_ms) {
        f->candidate=*observed;f->since_ms=now_ms;f->pending=true;return false;
    }
    return now_ms-f->since_ms>=100;
}
