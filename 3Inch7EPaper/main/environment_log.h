#pragma once
#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"

#include "environment_history.h"

esp_err_t environment_log_start(void);
void environment_log_sample(float temperature,float humidity,bool valid,int battery_percent);
void environment_log_snapshot(environment_history_t *out);
void environment_log_verify_request(void);
// Wait for the logger to finish/close its file, then pause all card access.
esp_err_t environment_log_prepare_shutdown(unsigned timeout_ms);
void environment_log_resume(void);
// A lease keeps VFS/card handles alive while a music FILE or directory is open.
bool environment_storage_acquire(void);
void environment_storage_release(void);
bool environment_storage_available(void);
