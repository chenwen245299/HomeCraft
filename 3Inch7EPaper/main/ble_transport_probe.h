#pragma once
#include <stdbool.h>
typedef struct {bool enabled,synced,advertising;unsigned connections;} ble_transport_probe_t;
void ble_transport_probe(ble_transport_probe_t *out);
