#pragma once
#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
typedef struct {int64_t timestamp;float temperature,humidity;bool valid;int battery;} environment_row_t;
bool environment_csv_parse(const char *line,int day,environment_row_t *row,unsigned *minute);
bool environment_csv_format(char *line,size_t size,const environment_row_t *row);
