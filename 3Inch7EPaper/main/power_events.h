#pragma once
#include <stdint.h>
#include "power_status.h"

typedef struct {
    power_status_t candidate;
    uint64_t since_ms;
    bool pending;
} power_edge_filter_t;

// Observe only source flags; percent is read afresh after accepting a stable edge.
bool power_edge_observe(power_edge_filter_t *filter,const power_status_t *current,
                        const power_status_t *observed,uint64_t now_ms);
