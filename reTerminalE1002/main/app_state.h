#pragma once
#include "dashboard.h"
#include "e1002_display.h"
void app_snapshot(dashboard_state_t *out);
bool app_accept_usage(const char *json);
bool app_accept_weather(const char *json);
void app_pair_request(void);
void app_set_pairing(bool open,const char *code,const char *name);
void app_set_connected(bool connected);
bool app_control(const char *op,const char *page,uint32_t *request);
void app_status_json(char *out,size_t capacity);
bool app_ready_to_pair(void);
