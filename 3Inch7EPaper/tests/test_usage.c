#include <assert.h>
#include <stdio.h>
#include <string.h>
#include "usage.h"

int main(void)
{
    usage_record_t r;
    usage_record_init(&r);
    assert(r.windows[0].remaining_tenths == -1 && r.windows[2].remaining_tenths == -1);
    const char *codex = "{\"rate_limit\":{\"primary_window\":{\"used_percent\":26,\"limit_window_seconds\":18000,\"reset_at\":1792514682},\"secondary_window\":{\"used_percent\":48,\"limit_window_seconds\":604800,\"reset_at\":1792890838}}}";
    assert(usage_decode(PROVIDER_CODEX, codex, 1792497600, &r));
    assert(r.windows[0].remaining_tenths == 740 && r.windows[1].remaining_tenths == 520);
    assert(r.windows[0].window_mins == 300 && r.windows[1].window_mins == 10080);
    assert(r.windows[2].remaining_tenths == -1 && !r.monthly_spend_available);
    assert(usage_decode(PROVIDER_CODEX, "{\"rate_limit\":{\"primary_window\":{\"used_percent\":null,\"limit_window_seconds\":18000},\"secondary_window\":{\"used_percent\":0,\"limit_window_seconds\":2592000}}}",1792497600,&r));
    assert(r.windows[0].remaining_tenths == -1 && r.windows[1].remaining_tenths == -1 && r.windows[2].remaining_tenths == 1000);
    const char *claude = "{\"five_hour\":{\"utilization\":23.5,\"resets_at\":\"2026-10-20T12:00:00Z\"},\"seven_day\":{\"utilization\":41.2,\"resets_at\":\"2026-10-20T20:00:00+08:00\"},\"extra_usage\":{\"is_enabled\":true,\"used_credits\":479,\"monthly_limit\":5000}}";
    assert(usage_decode(PROVIDER_CLAUDE, claude,1792497600,&r));
    assert(r.windows[0].remaining_tenths == 765 && r.windows[1].remaining_tenths == 588);
    assert(r.windows[0].reset_at == 1792497600 && r.windows[1].reset_at == 1792497600);
    assert(r.monthly_spend_available && r.monthly_used == 4.79 && r.monthly_limit == 50);
    assert(r.windows[2].remaining_tenths == -1); // Extra spend is never a subscription month allowance.
    assert(usage_decode(PROVIDER_CLAUDE, "{\"five_hour\":null,\"seven_day\":null}",1792497600,&r));
    assert(r.windows[0].remaining_tenths == -1 && r.windows[1].remaining_tenths == -1);
    assert(usage_decode(PROVIDER_CLAUDE, "{\"five_hour\":{\"utilization\":true},\"seven_day\":{\"utilization\":1000}}",1792497600,&r));
    assert(r.windows[0].remaining_tenths == -1 && r.windows[1].remaining_tenths == 0);
    const char *deepseek = "{\"is_available\":true,\"balance_infos\":[{\"currency\":\"USD\",\"total_balance\":\"20.50\"},{\"currency\":\"CNY\",\"total_balance\":\"110.00\"}]}";
    assert(usage_decode(PROVIDER_DEEPSEEK,deepseek,1792497600,&r));
    assert(r.balance_available && r.balance == 110 && !strcmp(r.currency,"CNY"));
    assert(r.windows[1].remaining_tenths == -1 && r.windows[2].remaining_tenths == -1);
    assert(!usage_decode(PROVIDER_DEEPSEEK,"{\"balance_infos\":[{\"currency\":\"CNY\",\"total_balance\":\"nan\"}]}",1792497600,&r));
    assert(!usage_decode(PROVIDER_DEEPSEEK,"{\"balance_infos\":[{\"currency\":\"CNY\",\"total_balance\":\"1.2junk\"}]}",1792497600,&r));
    assert(!usage_decode(PROVIDER_CODEX,"{\"error\":\"no data\"}",1792497600,&r));
    assert(!usage_decode(PROVIDER_CODEX,"{",1792497600,&r));
    assert(!usage_decode(PROVIDER_CODEX,codex,0,&r));
    provider_id_t p;
    assert(usage_previous_day(20261025)==20261024 && usage_previous_day(20250101)==20241231 && usage_previous_day(20240301)==20240229);
    assert(usage_packet_decode("{\"v\":1,\"p\":0,\"t\":1792497600,\"s\":2,\"yd\":[20261019,100,280]}",&p,&r));
    assert(r.yesterday_day==20261019 && r.yesterday_tokens==100 && r.yesterday_week_used_tenths==280);
    assert(!usage_packet_decode("{\"v\":1,\"p\":3,\"t\":1792497600,\"s\":2,\"yd\":[20261019,100,280]}",&p,&r));
    assert(!usage_packet_decode("{\"v\":1,\"p\":0,\"t\":1792497600,\"s\":2,\"yd\":[20260230,100,280]}",&p,&r));
    assert(!usage_packet_decode("{\"v\":1,\"p\":0,\"t\":1792497600,\"s\":2,\"yd\":[20261019,100,1001]}",&p,&r));
    assert(usage_packet_decode("{\"v\":1,\"p\":0,\"t\":1792497600,\"s\":2,\"w\":[[440,300,1792498000],[570,10080,1792890000],[-1,0,0]]}",&p,&r));
    assert(p==PROVIDER_CODEX && r.has_data && r.windows[0].remaining_tenths==440 && r.windows[2].remaining_tenths==-1);
    assert(usage_packet_decode("{\"v\":1,\"p\":2,\"t\":1792497600,\"s\":2,\"b\":110.5,\"c\":\"CNY\"}",&p,&r));
    assert(p==PROVIDER_DEEPSEEK && r.balance_available && r.balance==110.5);
    assert(!usage_packet_decode("{\"v\":1,\"p\":0,\"t\":1792497600,\"s\":2,\"access_token\":\"never-forward\"}",&p,&r));
    assert(!usage_packet_decode("{\"v\":1,\"p\":0,\"p\":1,\"t\":1792497600,\"s\":2}",&p,&r));
    assert(!usage_packet_decode("{\"v\":1,\"p\":0,\"t\":1792497600.5,\"s\":2}",&p,&r));
    assert(!usage_packet_decode("{\"v\":1,\"p\":0,\"t\":1792497600,\"s\":2,\"w\":[[true,0,0],[-1,0,0],[-1,0,0]]}",&p,&r));
    assert(!usage_packet_decode("{\"v\":1,\"p\":0,\"t\":1792497600,\"s\":2,\"b\":1,\"c\":\"evil\"}",&p,&r));
    assert(usage_packet_decode("{\"v\":1,\"p\":0,\"t\":1792497600,\"s\":2,\"d\":[20261020,-1],\"ld\":[20261019,28298767]}",&p,&r));
    assert(r.daily_tokens==-1 && r.latest_daily_tokens==28298767 && r.latest_token_day==20261019);
    assert(!usage_packet_decode("{\"v\":1,\"p\":0,\"t\":1792497600,\"s\":2,\"d\":[20260230,10]}",&p,&r));
    assert(!usage_packet_decode("{\"v\":1,\"p\":0,\"t\":1792497600,\"s\":2,\"d\":[20261020,-2]}",&p,&r));
    assert(!usage_packet_decode("{\"v\":1,\"p\":0,\"t\":1792497600,\"s\":2,\"d\":[20261020,1.5]}",&p,&r));
    assert(usage_packet_decode("{\"v\":1,\"p\":3,\"t\":1792497600,\"s\":2,\"n\":\"Argus\",\"d\":[20261020,120],\"u\":[100,20,60,3,2],\"dc\":0}",&p,&r));
    assert(p==PROVIDER_ARGUS && r.daily_tokens==120 && r.cached_tokens==60 && r.daily_cost_available && r.daily_cost_usd==0 && r.windows[0].remaining_tenths==-1);
    assert(!usage_packet_decode("{\"v\":1,\"p\":3,\"t\":1792497600,\"s\":2,\"n\":\"Codex\"}",&p,&r));
    assert(!usage_packet_decode("{\"v\":1,\"p\":4,\"t\":1792497600,\"s\":2,\"n\":\"Argus\"}",&p,&r));
    assert(!usage_packet_decode("{\"v\":1,\"p\":4,\"t\":1792497600,\"s\":2,\"n\":\"Test\",\"d\":[20261020,180],\"u\":[100,20,60,3,2]}",&p,&r)); // Cached input is already included.
    assert(usage_week_start(20261019)==20261019 && usage_week_start(20261025)==20261019);
    assert(usage_week_start(20261026)==20261026 && usage_week_start(20250101)==20241230);
    assert(usage_week_start(20240301)==20240226 && usage_week_start(20260230)==0);
    assert(usage_week_start(20261301)==0 && usage_week_start(0)==0);
    assert(usage_month_start(20261024)==20261001 && usage_month_start(20260230)==0 && usage_month_start(20250101)==20250101);
    assert(usage_packet_decode("{\"v\":1,\"p\":0,\"t\":1792497600,\"s\":2,\"mt\":[20261001,99]}",&p,&r));
    assert(r.token_month==20261001 && r.monthly_tokens==99);
    assert(!usage_packet_decode("{\"v\":1,\"p\":0,\"t\":1792497600,\"s\":2,\"mt\":[20261002,99]}",&p,&r));
    assert(!usage_packet_decode("{\"v\":1,\"p\":0,\"t\":1792497600,\"s\":2,\"mt\":[20261001,-1]}",&p,&r));
    assert(!usage_packet_decode("{\"v\":1,\"p\":0,\"t\":1792497600,\"s\":2,\"mt\":[20261001,1],\"mt\":[20261001,2]}",&p,&r));
    assert(usage_packet_decode("{\"v\":1,\"p\":3,\"t\":1792497600,\"s\":2,\"n\":\"Argus\",\"d\":[20261020,0],\"wt\":[20261019,120]}",&p,&r));
    assert(r.daily_tokens==0 && r.weekly_tokens==120 && r.token_week==20261019);
    assert(!usage_packet_decode("{\"v\":1,\"p\":3,\"t\":1792497600,\"s\":2,\"n\":\"Argus\",\"wt\":[20261020,120]}",&p,&r));
    assert(!usage_packet_decode("{\"v\":1,\"p\":3,\"t\":1792497600,\"s\":2,\"n\":\"Argus\",\"wt\":[20260230,120]}",&p,&r));
    assert(!usage_packet_decode("{\"v\":1,\"p\":3,\"t\":1792497600,\"s\":2,\"n\":\"Argus\",\"wt\":[20261019,-1]}",&p,&r));
    assert(!usage_packet_decode("{\"v\":1,\"p\":3,\"t\":1792497600,\"s\":2,\"n\":\"Argus\",\"wt\":[20261019,9007199254740992]}",&p,&r));
    assert(!usage_packet_decode("{\"v\":1,\"p\":3,\"t\":1792497600,\"s\":2,\"n\":\"Argus\",\"wt\":[20261019,120],\"wt\":[20261019,120]}",&p,&r));
    assert(usage_packet_decode("{\"v\":1,\"p\":3,\"t\":1792497600,\"s\":2,\"n\":\"Argus\",\"wt\":[20241230,0]}",&p,&r));
    assert(r.weekly_tokens==0 && r.token_week==20241230 && r.daily_tokens==-1);
    usage_record_t profiles[PROVIDER_COUNT];for(unsigned i=0;i<PROVIDER_COUNT;++i)usage_record_init(&profiles[i]);
    unsigned chosen[4]={99,99,99,99};
    assert(usage_overview_select(chosen,4)==3 && chosen[0]==PROVIDER_CODEX && chosen[1]==PROVIDER_ARGUS && chosen[2]==PROVIDER_EXTRA1 && chosen[3]==99);
    assert(usage_overview_select(chosen,1)==1 && chosen[0]==PROVIDER_CODEX);
    assert(usage_overview_select(chosen,0)==0 && usage_overview_select(NULL,3)==0);
    puts("Provider schemas, missing windows, monthly-money distinction and UTC resets: OK");
    assert(usage_clock_target(0,1792497600)==1792497600);
    assert(usage_clock_target(1792497600,1792497616)==1792497616); // New packet ahead of a cached startup clock.
    assert(usage_clock_target(1792497600,1792497540)==1792497600); // Cache cannot rewind minute logging.
    assert(usage_clock_target(1792497600,1792497602)==1792497600); // Ignore sub-three-second jitter.
    assert(usage_clock_target(1792497600,0)==1792497600);
    assert(usage_clock_target(1792497600,4102444801LL)==1792497600);
    assert(usage_clock_target(INT64_MAX,1792497600)==1792497600);
    return 0;
}
