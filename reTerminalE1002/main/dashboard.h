#pragma once
#include "usage.h"
#include "color_canvas.h"
#define TODO_QUADRANTS 4
#define TODO_ITEMS 5
// tomorrow_* hold tomorrow's WMO code and range when has_tomorrow (forecast:1).
typedef struct {bool valid,has_tomorrow;char city[49];int temperature_tenths,low_tenths,high_tenths,code,tomorrow_code,tomorrow_low,tomorrow_high;int64_t fetched_at;} weather_t;
typedef struct {unsigned count,total;char items[TODO_ITEMS][49];} todo_quadrant_t; // total: open items in HomeCraft
typedef struct {bool valid;unsigned count;char title[37],author[25],lines[4][61];} poem_t;
#define PLACES 2
// A place named in HomeCraft (places:1). here: this display, shown with its own sensor; otherwise HomeCraft's
// latest reading of that display when known.
typedef struct {bool here,known;char name[49];int temperature_tenths,humidity_tenths;int64_t at;} place_t;
// SY6974B status (REG08), read only: unknown until the charger answers.
enum {CHARGE_UNKNOWN=-1,CHARGE_NO_INPUT,CHARGE_IDLE,CHARGE_ACTIVE,CHARGE_DONE};
typedef struct {
 usage_record_t usage[PROVIDER_COUNT];weather_t weather[2]; // up to two HomeCraft cities
 todo_quadrant_t todo[TODO_QUADRANTS];bool todo_known;poem_t poem;
 place_t places[PLACES];unsigned place_count; // none: the indoor sensor alone
 int64_t now;float temperature,humidity;bool sensor_valid,connected,busy;int battery_mv,charge;
 // HomeCraft's home-page totals (totals:1): today, its Monday, its month; tokens -1 when unknown.
 bool hub_totals;int hub_periods[3];int64_t hub_tokens[3];
 char pairing_code[9],device_name[20];bool pairing;unsigned page,frame_sequence;
 bool privacy; // picture only, toggled by double-clicking the right key
} dashboard_state_t;
void dashboard_render(color_canvas_t *c,const dashboard_state_t *s);
// Today/week/month totals as shown (-1 unknown): HomeCraft's for matching periods, else the sum of packets.
void dashboard_totals(const dashboard_state_t *s,int day,int64_t out[3]);
int quota_ink(int remaining_tenths);
// Voltage-based estimate (percent, bars 0..4); -1 when no battery reading. Not a calibrated state of charge.
int battery_percent(int millivolts);
int battery_level(int millivolts);
