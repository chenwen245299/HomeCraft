#pragma once
#include <stdbool.h>
typedef enum { SHUTDOWN_STANDBY, SHUTDOWN_STORAGE_BUSY, SHUTDOWN_NETWORK_BUSY, SHUTDOWN_DISPLAY_FAILED } shutdown_result_t;
typedef struct {
    bool (*storage_stop)(void *);
    bool (*network_stop)(void *);
    bool (*draw_off_frame)(void *);
    void (*display_sleep)(void *);
    void (*power_off)(void *);
    void (*standby)(void *);
    void (*resume)(void *);
} shutdown_ops_t;
shutdown_result_t shutdown_run(const shutdown_ops_t *ops, void *context);
