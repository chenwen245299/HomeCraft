#include "ble_transport_probe.h"
#include "host/ble_hs.h"
#include "host/ble_gap.h"

// ESP-IDF 6.1 exports this connection iterator but omits its declaration from
// ble_gap.h. Its callback type is public; the implementation requires the host
// lock. Keep this compatibility shim isolated and recheck it when updating IDF.
extern void ble_hs_lock(void);
extern void ble_hs_unlock(void);
extern void ble_gap_conn_foreach_handle(ble_gap_conn_foreach_handle_fn *cb,void *arg);
static int count_connection(uint16_t handle,void *arg)
{
    (void)handle;++*(unsigned *)arg;return 0;
}
void ble_transport_probe(ble_transport_probe_t *out)
{
    *out=(ble_transport_probe_t){0};
    out->enabled=ble_hs_is_enabled();out->synced=out->enabled && ble_hs_synced();
    if(!out->synced)return;
    ble_hs_lock();ble_gap_conn_foreach_handle(count_connection,&out->connections);ble_hs_unlock();
    out->advertising=ble_gap_adv_active();
}
