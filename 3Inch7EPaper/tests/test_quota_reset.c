#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "quota_reset.h"

int main(void)
{
    const int64_t now = 1792692000;
    const struct { int64_t reset, clock; const char *expected; } cases[] = {
        {now + 8280, now, "02:18"},
        {now + 18000, now, "05:00"},
        {now + 3599, now, "01:00"},
        {now + 61, now, "00:02"},
        {now + 60, now, "00:01"},
        {now + 59, now, "00:01"},
        {now + 1, now, "00:01"},
        {now, now, "00:00"},
        {now - 3600, now, "00:00"},
        {0, now, "--:--"},
        {-1, now, "--:--"},
        {INT64_MAX, now, "--:--"},
        {now + 8280, 0, "--:--"},
        {now + 8280, INT64_MIN, "--:--"},
        {now + 8280, INT64_MAX, "--:--"},
        {now + 100800, now, "28:00"},
    };
    for (unsigned i = 0; i < sizeof(cases) / sizeof(cases[0]); ++i) {
        char text[80];
        quota_reset_label(text, sizeof(text), cases[i].reset, cases[i].clock);
        assert(!strcmp(text, cases[i].expected));
    }
    char small[4];
    quota_reset_label(small, sizeof(small), now + 8280, now);
    assert(small[3] == '\0');
    quota_reset_label(NULL, 0, now, now);
    puts("Reset countdown: 16 clock/rounding/missing/expired boundaries and bounded output OK");
}
