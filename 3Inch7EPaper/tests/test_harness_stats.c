#include "harness_stats.h"
#include "device_protocol.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
int main(void)
{
    harness_stats_t stats={.version=2};usage_record_t r,total;provider_id_t p;bool changed;
    const char *packet="{\"v\":1,\"p\":4,\"t\":1792540800,\"s\":2,\"n\":\"DeepSeek Harness\",\"d\":[20261021,165],\"wt\":[20261019,200],\"u\":[155,10,50,1,1],\"o\":{\"id\":\"0000000000000001\",\"name\":\"Mac-mini\"}}";
    assert(usage_packet_decode(packet,&p,&r));assert(p==4);
    assert(harness_stats_update(&stats,&r,&changed)&&changed);
    assert(harness_stats_update(&stats,&r,&changed)&&!changed);
    harness_stats_total(&stats,20261021,&total);assert(total.daily_tokens==-1 && total.weekly_tokens==-1);
    strcpy(r.source_id,"0000000000000002");strcpy(r.source_name,"MacBook-Pro");
    assert(harness_stats_update(&stats,&r,&changed)&&changed);
    harness_stats_total(&stats,20261021,&total);assert(total.daily_tokens==-1 && !total.has_data);
    ++r.fetched_at;r.daily_tokens=200;r.input_tokens=190;r.weekly_tokens=235;
    assert(harness_stats_update(&stats,&r,&changed)&&changed);
    harness_stats_total(&stats,20261021,&total);assert(total.daily_tokens==-1 && stats.devices[1].daily==200);
    assert(harness_stats_update(&stats,&r,&changed)&&!changed);
    --r.fetched_at;assert(!harness_stats_update(&stats,&r,&changed));++r.fetched_at;
    harness_stats_total(&stats,20261022,&total);assert(total.daily_tokens==-1&&!total.usage_available&&total.weekly_tokens==-1);
    harness_stats_total(&stats,20261026,&total);assert(total.daily_tokens==-1&&total.weekly_tokens==-1);
    harness_stats_t restored;memcpy(&restored,&stats,sizeof(stats));assert(harness_stats_valid(&restored));
    for(unsigned i=3;i<=8;++i){snprintf(r.source_id,sizeof(r.source_id),"%016x",i);assert(harness_stats_update(&stats,&r,&changed));}
    strcpy(r.source_id,"0000000000000009");assert(!harness_stats_update(&stats,&r,&changed));
    char out[481];for(unsigned i=0;i<8;++i)assert(harness_stats_encode(&stats,i,out)&&strlen(out)<=480);
    assert(!harness_stats_encode(&stats,8,out));
    strcpy(r.source_id,"0000000000000001");++r.fetched_at;r.token_month=20261001;r.monthly_tokens=400;
    assert(harness_stats_update(&stats,&r,&changed));
    harness_stats_total(&stats,20261021,&total);assert(total.monthly_tokens==-1 && stats.devices[0].monthly==400);
    harness_stats_total(&stats,20261101,&total);assert(total.monthly_tokens==-1);
    assert(harness_stats_encode(&stats,0,out) && strstr(out,"\"mt\":[20261001,400]"));
    typedef struct {char id[17],name[25];int64_t fetched_at;int day,week;int64_t daily,weekly,input,output,cached,calls;} old_device_t;
    typedef struct {uint32_t version,count;old_device_t devices[8];} old_stats_t;
    old_stats_t old={.version=1,.count=stats.count};
    for(unsigned i=0;i<old.count;++i)memcpy(&old.devices[i],&stats.devices[i],sizeof(old.devices[i]));
    assert(harness_stats_restore(&old,sizeof(old),&restored));
    assert(restored.version==2 && restored.count==stats.count && restored.devices[0].daily==stats.devices[0].daily);
    assert(restored.devices[0].month==0 && restored.devices[0].monthly==-1);
    assert(harness_stats_restore(&stats,sizeof(stats),&restored) && restored.devices[0].monthly==400);
    assert(!harness_stats_restore(&old,3,&restored));
    device_command_t command;assert(device_command_decode("{\"v\":1,\"op\":\"harness\",\"offset\":0}",&command)&&command.operation==DEVICE_HARNESS);
    assert(!device_command_decode("{\"v\":1,\"op\":\"harness\",\"offset\":8}",&command));
    assert(!device_command_decode("{\"v\":1,\"op\":\"harness\",\"offset\":0,\"password\":\"x\"}",&command));
    // A deduplicated hub result atomically supersedes all overlapping legacy caches.
    const char *aggregate="{\"v\":1,\"p\":4,\"t\":1792540900,\"s\":2,\"n\":\"DeepSeek Harness\",\"d\":[20261021,165],\"wt\":[20261019,200],\"mt\":[20261001,500],\"u\":[155,10,50,1,1]}";
    assert(usage_packet_decode(aggregate,&p,&r));
    assert(harness_stats_update(&stats,&r,&changed)&&changed);
    assert(stats.version==3 && stats.count==1);
    harness_stats_total(&stats,20261021,&total);
    assert(total.daily_tokens==165 && total.weekly_tokens==200 && total.monthly_tokens==500 && total.calls==1);
    assert(harness_stats_update(&stats,&r,&changed)&&!changed);
    ++r.fetched_at;assert(harness_stats_update(&stats,&r,&changed)&&!changed);
    assert(harness_stats_encode(&stats,0,out) && strstr(out,"\"count\":0") && !strstr(out,"Shared Harness"));
    assert(harness_stats_restore(&stats,sizeof(stats),&restored) && restored.version==3);
    // An old Mac connecting later cannot reintroduce per-Mac totals.
    assert(usage_packet_decode(packet,&p,&r));r.fetched_at+=1000;
    assert(harness_stats_update(&stats,&r,&changed)&&!changed);
    harness_stats_total(&stats,20261021,&total);assert(total.daily_tokens==165);
    // Yesterday's cache never becomes today's zero; known week/month remain available.
    harness_stats_total(&stats,20261022,&total);assert(total.daily_tokens==-1 && total.weekly_tokens==200 && total.monthly_tokens==500);
    const char *partial="{\"v\":1,\"p\":4,\"t\":1792541000,\"s\":2,\"n\":\"DeepSeek Harness\",\"d\":[20261021,-1],\"wt\":[20261019,200]}";
    assert(usage_packet_decode(partial,&p,&r));assert(harness_stats_update(&stats,&r,&changed)&&changed);
    harness_stats_total(&stats,20261021,&total);assert(total.daily_tokens==-1 && total.weekly_tokens==200 && total.monthly_tokens==-1);
    // A conflict/unknown result explicitly clears the authoritative cache and survives reboot.
    const char *unknown="{\"v\":1,\"p\":4,\"t\":1792541100,\"s\":6,\"n\":\"DeepSeek Harness\"}";
    assert(usage_packet_decode(unknown,&p,&r));assert(harness_stats_update(&stats,&r,&changed)&&changed);
    assert(stats.version==3 && stats.count==0 && harness_stats_restore(&stats,sizeof(stats),&restored));
    assert(usage_packet_decode(packet,&p,&r));assert(harness_stats_update(&stats,&r,&changed)&&!changed);
    harness_stats_total(&stats,20261021,&total);assert(total.daily_tokens==-1 && !total.has_data);
    assert(usage_packet_decode(aggregate,&p,&r));
    assert(harness_stats_update(&stats,&r,&changed)&&!changed);
    assert(stats.count==0); // A stale success cannot resurrect a conflict cleared later.
    puts("Harness: legacy migration, authoritative deduplicated aggregate, no source echo, unknown periods, reboot and stale-Mac protection OK");
}
