#pragma once
#include <stdbool.h>
#include <stdint.h>
typedef enum { REFRESH_SKIP, REFRESH_FULL, REFRESH_DIFFERENTIAL } refresh_kind_t;
typedef struct {bool ready,enabled;unsigned partial_count;uint32_t layout;int64_t last_full_us;} refresh_policy_t;
refresh_kind_t refresh_decide(const refresh_policy_t *state,bool changed,bool forced,uint32_t layout,int64_t now);
void refresh_commit(refresh_policy_t *state,refresh_kind_t kind,bool success,uint32_t layout,int64_t now);
