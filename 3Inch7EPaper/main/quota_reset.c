#include "quota_reset.h"
#include <stdio.h>

void quota_reset_label(char *out, size_t size, int64_t reset_at, int64_t now)
{
    if (reset_at <= 0 || reset_at > INT64_C(4102444800) ||
        now < INT64_C(1735689600) || now > INT64_C(4102444800)) {
        snprintf(out, size, "--:--");
    } else {
        int64_t minutes = reset_at > now ? (reset_at - now + 59) / 60 : 0;
        snprintf(out, size, "%02lld:%02lld", (long long)(minutes / 60), (long long)(minutes % 60));
    }
}
