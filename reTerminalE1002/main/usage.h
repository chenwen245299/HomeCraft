#pragma once
#include "protocol.h"

enum { PROVIDER_ACCOUNT_COUNT = 3 };
typedef enum { PROVIDER_CODEX, PROVIDER_CLAUDE, PROVIDER_DEEPSEEK, PROVIDER_ARGUS,
               PROVIDER_EXTRA1, PROVIDER_EXTRA2, PROVIDER_EXTRA3, PROVIDER_EXTRA4, PROVIDER_COUNT } provider_id_t;
typedef enum {
    USAGE_NOT_CONFIGURED, USAGE_WAITING, USAGE_OK, USAGE_AUTH_ERROR,
    USAGE_RATE_LIMITED, USAGE_NETWORK_ERROR, USAGE_BAD_RESPONSE, USAGE_ACCESS_DENIED
} usage_status_t;
typedef struct {
    quota_window_t windows[3]; // short window, week, month; -1 means not supplied.
    bool configured, has_data;
    usage_status_t status;
    int http_status;
    int64_t fetched_at;
    bool balance_available, monthly_spend_available;
    double balance, monthly_used, monthly_limit;
    char currency[4];
    // Official date buckets only. -1 means not supplied; never a context-token counter.
    int token_day, latest_token_day;
    int64_t daily_tokens, latest_daily_tokens;
    int token_week; // Monday in UTC+8; usage, not remaining quota.
    int64_t weekly_tokens;
    int token_month; // First calendar day in UTC+8; known usage, not quota.
    int64_t monthly_tokens;
    int yesterday_day, yesterday_week_used_tenths;
    int64_t yesterday_tokens;
    char name[25]; // Optional ASCII profile label; built-in account names cannot be replaced.
    bool usage_available, daily_cost_available;
    int64_t input_tokens, output_tokens, cached_tokens, calls, missing_cost_calls;
    double daily_cost_usd;
} usage_record_t;

void usage_record_init(usage_record_t *record);
bool usage_decode(provider_id_t provider, const char *json, int64_t now, usage_record_t *out);
const char *usage_provider_name(provider_id_t provider);
const char *usage_record_name(provider_id_t provider, const usage_record_t *record);
unsigned usage_overview_select(const usage_record_t *records, unsigned count, int day,
                               int64_t now, unsigned *selected, unsigned capacity);
const char *usage_status_text(usage_status_t status);
// Credential-free, compact BLE snapshot. Whitelisted fields only, <= 480 bytes.
bool usage_packet_decode(const char *json, provider_id_t *provider, usage_record_t *out);
// Authenticated snapshot times may advance the clock; cached packets never rewind it.
int64_t usage_clock_target(int64_t current, int64_t fetched_at);
int usage_week_start(int day); // YYYYMMDD -> Monday, or 0 for an invalid date.
int usage_month_start(int day);
int usage_previous_day(int day);
// Replace the displayed result with the hub's complete snapshot, including explicit unknowns.
// Out-of-order transport packets are acknowledged without changing the newer display.
bool usage_display_update(usage_record_t *current, const usage_record_t *incoming);
