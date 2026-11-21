#include "usage.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>

int main(void)
{
    provider_id_t p;usage_record_t current,next,before;usage_record_init(&current);
    const char *first="{\"v\":1,\"p\":4,\"t\":1792540800,\"s\":2,\"n\":\"DeepSeek Harness\",\"d\":[20261021,165],\"wt\":[20261019,200],\"mt\":[20261001,500],\"u\":[155,10,50,1,1]}";
    assert(usage_packet_decode(first,&p,&next)&&p==PROVIDER_EXTRA1);
    assert(usage_display_update(&current,&next));before=current;
    for(unsigned i=0;i<100;++i)assert(usage_display_update(&current,&next));
    assert(!memcmp(&current,&before,sizeof(current)) && current.daily_tokens==165 && current.calls==1);
    // A lower corrected total replaces the previous snapshot; nothing is accumulated.
    const char *replacement="{\"v\":1,\"p\":4,\"t\":1792540800,\"s\":2,\"n\":\"DeepSeek Harness\",\"d\":[20261021,70],\"wt\":[20261019,80],\"u\":[60,10,20,1,1]}";
    assert(usage_packet_decode(replacement,&p,&next)&&usage_display_update(&current,&next));
    assert(current.daily_tokens==70 && current.weekly_tokens==80 && current.monthly_tokens==-1 && current.input_tokens==60);
    const char *partial="{\"v\":1,\"p\":4,\"t\":1792540900,\"s\":2,\"n\":\"DeepSeek Harness\",\"d\":[20261021,-1],\"wt\":[20261019,80]}";
    assert(usage_packet_decode(partial,&p,&next)&&usage_display_update(&current,&next));
    assert(current.daily_tokens==-1 && current.weekly_tokens==80 && current.monthly_tokens==-1 && !current.usage_available && current.calls==0);
    const char *unknown="{\"v\":1,\"p\":4,\"t\":1792541000,\"s\":6,\"n\":\"DeepSeek Harness\"}";
    assert(usage_packet_decode(unknown,&p,&next)&&usage_display_update(&current,&next));
    assert(!current.has_data && current.status==USAGE_BAD_RESPONSE && current.daily_tokens==-1 && current.weekly_tokens==-1 && current.monthly_tokens==-1);
    before=current;assert(usage_packet_decode(first,&p,&next)&&usage_display_update(&current,&next));
    assert(!memcmp(&current,&before,sizeof(current)));
    // Per-machine sources are no longer part of the display protocol.
    const char *legacy="{\"v\":1,\"p\":4,\"t\":1792541001,\"s\":2,\"n\":\"DeepSeek Harness\",\"d\":[20261021,165],\"o\":{\"id\":\"0000000000000001\",\"name\":\"Mac-mini\"}}";
    assert(!usage_packet_decode(legacy,&p,&next));
    assert(!usage_display_update(NULL,&current) && !usage_display_update(&current,NULL));
    puts("Display snapshots: exact replacement, duplicates, downward corrections, unknown clearing, stale packets and source rejection passed.");
}
