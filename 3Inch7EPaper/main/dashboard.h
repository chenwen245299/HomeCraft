#pragma once
#include "canvas.h"
#include "protocol.h"
#include "network_types.h"
#include "power_status.h"
#include "environment_history.h"
#include "music.h"
typedef enum { PAGE_OVERVIEW, PAGE_QUOTA, PAGE_CLAUDE, PAGE_DEEPSEEK, PAGE_ENVIRONMENT, PAGE_NETWORK, PAGE_HELP,
               PAGE_ARGUS, PAGE_EXTRA1, PAGE_EXTRA2, PAGE_EXTRA3, PAGE_EXTRA4, PAGE_MUSIC, PAGE_COUNT } dashboard_page_t;

typedef struct {
    quota_snapshot_t quota;
    bool has_quota;
    unsigned quota_age_sec;
    bool sensor_valid;
    float temperature;
    float humidity;
    power_status_t power;
    environment_history_t history;
    bool imu_valid;
    bool auto_rotate;
    bool paused;
    dashboard_page_t page;
    network_snapshot_t network;
    music_snapshot_t music;
} dashboard_state_t;

void dashboard_render(canvas_t *canvas, const dashboard_state_t *state);
void dashboard_render_power(canvas_t *canvas,const power_status_t *power,bool paused);
void dashboard_render_shutdown(canvas_t *canvas, bool failed);
// Structural changes need a fresh base frame; sensor values and curve data do not.
uint32_t dashboard_layout_key(const dashboard_state_t *state, const canvas_t *canvas);
dashboard_page_t dashboard_step_page(const dashboard_state_t *state,int step);
bool dashboard_page_available(const dashboard_state_t *state,unsigned page);
void dashboard_music_page(canvas_t *canvas,const music_snapshot_t *music);
