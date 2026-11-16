#pragma once
#include <stdbool.h>
typedef struct {int month,day;bool leap;} lunar_date_t;
bool lunar_calendar_date(int date,lunar_date_t *out);
bool lunar_calendar_text(int date,char out[48]);
