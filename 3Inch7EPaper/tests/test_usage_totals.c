#include "usage.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>

int main(void)
{
    usage_totals_t hub={0};
    const char *packet="{\"v\":1,\"op\":\"totals\",\"d\":[20261028,20200000],\"wt\":[20261026,105400000],\"mt\":[20261001,2760000000]}";
    assert(usage_totals_decode(packet,&hub));
    assert(hub.received && hub.tokens[2]==2760000000LL);
    const char *invalid[]={
        "null", "[]", "{}", "{", "{\"v\":1,\"op\":\"totals\"}",
        "{\"v\":2,\"op\":\"totals\",\"d\":[20261028,0],\"wt\":[20261026,0],\"mt\":[20261001,0]}",
        "{\"v\":1,\"op\":\"total\",\"d\":[20261028,0],\"wt\":[20261026,0],\"mt\":[20261001,0]}",
        "{\"v\":1,\"v\":1,\"op\":\"totals\",\"d\":[20261028,0],\"wt\":[20261026,0],\"mt\":[20261001,0]}",
        "{\"v\":1,\"op\":\"totals\",\"d\":[20261028,0],\"wt\":[20261026,0],\"mt\":[20261001,0],\"key\":\"x\"}",
        "{\"v\":1,\"op\":\"totals\",\"d\":[20260230,0],\"wt\":[20260223,0],\"mt\":[20260201,0]}",
        "{\"v\":1,\"op\":\"totals\",\"d\":[20261028,0],\"wt\":[20261019,0],\"mt\":[20261001,0]}",
        "{\"v\":1,\"op\":\"totals\",\"d\":[20261028,0],\"wt\":[20261026,0],\"mt\":[20260901,0]}",
        "{\"v\":1,\"op\":\"totals\",\"d\":[20261028,-2],\"wt\":[20261026,0],\"mt\":[20261001,0]}",
        "{\"v\":1,\"op\":\"totals\",\"d\":[20261028,0.5],\"wt\":[20261026,0],\"mt\":[20261001,0]}",
        "{\"v\":1,\"op\":\"totals\",\"d\":[20261028,\"0\"],\"wt\":[20261026,0],\"mt\":[20261001,0]}",
        "{\"v\":1,\"op\":\"totals\",\"d\":[20261028,0,0],\"wt\":[20261026,0],\"mt\":[20261001,0]}",
        "{\"v\":1,\"op\":\"totals\",\"d\":[20261028,9007199254740992],\"wt\":[20261026,0],\"mt\":[20261001,0]}",
        "{\"v\":1,\"op\":\"totals\",\"d\":[20261028,0],\"wt\":[20261026,0],\"mt\":[20261001,0]} x"
    };
    for(unsigned i=0;i<sizeof(invalid)/sizeof(invalid[0]);++i){
        usage_totals_t before=hub;
        assert(!usage_totals_decode(invalid[i],&hub));
        assert(memcmp(&hub,&before,sizeof(hub))==0);
    }
    char long_packet[482];memset(long_packet,' ',481);long_packet[481]=0;
    assert(!usage_totals_decode(long_packet,&hub));
    assert(!usage_totals_decode(NULL,&hub) && !usage_totals_decode(packet,NULL));

    usage_record_t providers[PROVIDER_COUNT];
    for(unsigned i=0;i<PROVIDER_COUNT;++i)usage_record_init(&providers[i]);
    usage_record_t *r=&providers[PROVIDER_CODEX];
    r->has_data=true;r->token_day=20261028;r->daily_tokens=1000000;
    r->token_week=20261026;r->weekly_tokens=10000000;
    r->token_month=20261001;r->monthly_tokens=1320000000;
    int64_t values[3];
    usage_summary_totals(providers,PROVIDER_COUNT,NULL,20261028,values);
    assert(values[0]==1000000 && values[1]==10000000 && values[2]==1320000000);
    usage_summary_totals(providers,PROVIDER_COUNT,&hub,20261028,values);
    assert(values[0]==20200000 && values[1]==105400000 && values[2]==2760000000LL);
    // A new day keeps same-week/month hub totals but never yesterday's daily count.
    usage_summary_totals(providers,PROVIDER_COUNT,&hub,20261029,values);
    assert(values[0]==-1 && values[1]==105400000 && values[2]==2760000000LL);
    usage_totals_t previous=hub;
    previous.periods[0]=20261025;previous.periods[1]=20261019;
    usage_summary_totals(providers,PROVIDER_COUNT,&previous,20261028,values);
    assert(values[0]==-1 && values[1]==-1 && values[2]==2760000000LL);
    usage_summary_totals(providers,PROVIDER_COUNT,&hub,20261102,values);
    assert(values[0]==-1 && values[1]==-1 && values[2]==-1);
    assert(usage_totals_decode("{\"v\":1,\"op\":\"totals\",\"d\":[20261028,-1],\"wt\":[20261026,0],\"mt\":[20261001,9007199254740991]}",&hub));
    usage_summary_totals(providers,PROVIDER_COUNT,&hub,20261028,values);
    assert(values[0]==-1 && values[1]==0 && values[2]==9007199254740991LL);
    assert(usage_totals_decode("{\"v\":1,\"op\":\"totals\",\"d\":[20250101,0],\"wt\":[20241230,0],\"mt\":[20250101,0]}",&hub));
    usage_summary_totals(providers,PROVIDER_COUNT,&hub,20250101,values);
    assert(values[0]==0 && values[1]==0 && values[2]==0);
    puts("Hub totals: multi-account precedence, unknown/zero, invalid packets, legacy fallback and calendar rollover passed.");
}
