#include "shutdown.h"
shutdown_result_t shutdown_run(const shutdown_ops_t *ops, void *context)
{
    shutdown_result_t result=SHUTDOWN_STORAGE_BUSY;
    if(!ops->storage_stop(context))goto cancel;
    result=SHUTDOWN_NETWORK_BUSY;
    if(!ops->network_stop(context))goto cancel;
    result=SHUTDOWN_DISPLAY_FAILED;
    if(!ops->draw_off_frame(context))goto cancel;
    ops->display_sleep(context);
    ops->power_off(context);
    ops->standby(context); // Real implementation does not return if power remains via USB.
    return SHUTDOWN_STANDBY;
cancel:
    ops->resume(context);
    return result;
}
