#include "protocol.h"
#include <ctype.h>
#include <errno.h>
#include <stdlib.h>
#include <string.h>

static bool valid_window(const quota_window_t *w)
{
    return w->remaining_tenths >= -1 && w->remaining_tenths <= 1000 &&
           w->window_mins >= 0 && w->window_mins <= 525600 &&
           w->reset_at >= 0 && w->reset_at <= INT64_C(4102444800);
}

bool quota_parse(const char *line, quota_snapshot_t *out)
{
    if (!line || !out || strlen(line) > 240) return false;
    if (strncmp(line, "EPAPER1", 7) != 0 || !isspace((unsigned char)line[7])) return false;
    const char *cursor = line + 7;
    int64_t values[8];
    for (unsigned i = 0; i < 8; ++i) {
        while (isspace((unsigned char)*cursor)) ++cursor;
        char *end;
        errno = 0;
        long long value = strtoll(cursor, &end, 10);
        if (end == cursor || errno == ERANGE || (*end && !isspace((unsigned char)*end))) return false;
        values[i] = value;
        cursor = end;
    }
    while (isspace((unsigned char)*cursor)) ++cursor;
    if (*cursor || values[0] < INT64_C(1577836800) || values[0] > INT64_C(4102444800) ||
        values[1] < -1 || values[1] > 1000 || values[2] < 0 || values[2] > 525600 ||
        values[4] < -1 || values[4] > 1000 || values[5] < 0 || values[5] > 525600 ||
        values[7] < 0 || values[7] > 1) return false;
    quota_snapshot_t q = {
        .fetched_at = values[0],
        .primary = {(int)values[1], (int)values[2], values[3]},
        .secondary = {(int)values[4], (int)values[5], values[6]},
        .status = (int)values[7],
    };
    if (!valid_window(&q.primary) || !valid_window(&q.secondary))
        return false;
    *out = q;
    return true;
}
