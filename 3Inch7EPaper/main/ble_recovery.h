#pragma once
#include <stdbool.h>
#include <stdint.h>

typedef struct { bool waiting; uint64_t idle_since, retry_after; } ble_recovery_t;
// Only normal, already-bound data mode may recover an idle transport.
bool ble_recovery_due(ble_recovery_t *state, uint64_t now, bool allowed,
                      bool advertising, unsigned connections);
