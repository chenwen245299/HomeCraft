#pragma once
#include <stdint.h>

// Sampling, display evaluation and transport have independent schedules.
#define ENV_SAMPLE_MS 30000U
#define IMU_SAMPLE_MS 1000U
#define POWER_SOURCE_POLL_MS 100U
#define INPUT_POLL_MS 10U
#define DASHBOARD_POLL_MS 100U
#define NETWORK_POLL_MS 1000U
#define HARNESS_SAVE_MS 300000U
#define CPU_MAX_MHZ 80
#define CPU_MIN_MHZ 40
void energy_start(void);
void energy_counters(uint64_t *count,uint64_t *slept_us);
void energy_note_telemetry(void);
void energy_note_history(void);
void energy_work_counters(uint64_t *telemetry,uint64_t *history);
