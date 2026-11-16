#pragma once
#include <stdbool.h>
#include <stddef.h>
// Lunar month/day text such as "九月十八" for a UTC+8 Gregorian date in 2025–2060.
bool lunar_text(int year,int month,int day,char *out,size_t capacity);
