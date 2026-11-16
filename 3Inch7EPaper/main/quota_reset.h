#pragma once
#include <stddef.h>
#include <stdint.h>

// Official reset timestamp only; reaching it does not confirm fresh quota data.
void quota_reset_label(char *out, size_t size, int64_t reset_at, int64_t now);
