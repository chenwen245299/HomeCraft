#include "harness_stats.h"
#include <stdio.h>
#include <string.h>

static bool label(const char *s,unsigned capacity,bool id)
{
    const char *end=memchr(s,0,capacity);
    if(!end || end==s || (id && end-s!=16))return false;
    for(;s<end;++s)if(id?!strchr("0123456789abcdef",*s):!((*s>='a'&&*s<='z')||(*s>='A'&&*s<='Z')||(*s>='0'&&*s<='9')||strchr(" ._-",*s)))return false;
    return true;
}
static bool valid(const harness_device_t *d)
{
    const int64_t limit=9007199254740991LL/HARNESS_MAX_DEVICES;
    return label(d->id,sizeof(d->id),true)&&label(d->name,sizeof(d->name),false)&&
        d->fetched_at>=1735689600 && d->fetched_at<=4102444800 &&
        usage_week_start(d->day)!=0 && usage_week_start(d->week)==d->week && usage_week_start(d->day)==d->week &&
        d->daily>=0 && d->daily<=limit && d->weekly>=d->daily && d->weekly<=limit &&
        ((!d->month && d->monthly==-1) || (d->month==usage_month_start(d->day) && d->monthly>=d->daily && d->monthly<=limit)) &&
        d->input>=0 && d->output>=0 && d->input<=d->daily && d->output==d->daily-d->input &&
        d->cached>=0 && d->cached<=d->input && d->calls>=0 && d->calls<=limit;
}
bool harness_stats_valid(const harness_stats_t *s)
{
    if(!s || s->version!=2 || s->count>HARNESS_MAX_DEVICES)return false;
    for(unsigned i=0;i<s->count;++i) {
        if(!valid(&s->devices[i]))return false;
        for(unsigned j=0;j<i;++j)if(!strcmp(s->devices[i].id,s->devices[j].id))return false;
    }
    return true;
}
bool harness_stats_restore(const void *blob,size_t length,harness_stats_t *out)
{
    typedef struct {char id[17],name[25];int64_t fetched_at;int day,week;int64_t daily,weekly,input,output,cached,calls;} legacy_device_t;
    typedef struct {uint32_t version,count;legacy_device_t devices[HARNESS_MAX_DEVICES];} legacy_stats_t;
    _Static_assert(offsetof(harness_device_t,month)==sizeof(legacy_device_t),"Legacy Harness prefix changed");
    if(!blob || !out)return false;
    harness_stats_t restored={.version=2};
    if(length==sizeof(restored)){memcpy(&restored,blob,length);}
    else if(length==sizeof(legacy_stats_t)){
        legacy_stats_t old;memcpy(&old,blob,sizeof(old));
        if(old.version!=1 || old.count>HARNESS_MAX_DEVICES)return false;
        restored.count=old.count;
        for(unsigned i=0;i<old.count;++i){memcpy(&restored.devices[i],&old.devices[i],sizeof(legacy_device_t));restored.devices[i].monthly=-1;}
    }else return false;
    if(!harness_stats_valid(&restored))return false;
    *out=restored;return true;
}
bool harness_stats_update(harness_stats_t *s,const usage_record_t *r,bool *changed)
{
    *changed=false;
    if(!s || !r || r->status!=USAGE_OK || !r->usage_available || strcmp(r->name,"DeepSeek Harness"))return false;
    harness_device_t d={0};
    memcpy(d.id,r->source_id,sizeof(d.id));memcpy(d.name,r->source_name,sizeof(d.name));
    d.fetched_at=r->fetched_at;d.day=r->token_day;d.week=r->token_week;d.daily=r->daily_tokens;d.weekly=r->weekly_tokens;
    d.input=r->input_tokens;d.output=r->output_tokens;d.cached=r->cached_tokens;d.calls=r->calls;
    d.month=r->token_month;d.monthly=r->monthly_tokens;
    if(!valid(&d))return false;
    if(!s->version)s->version=2;
    if(!harness_stats_valid(s))return false;
    unsigned index=0;while(index<s->count && strcmp(s->devices[index].id,d.id))++index;
    if(index==s->count) {if(s->count==HARNESS_MAX_DEVICES)return false;++s->count;*changed=true;}
    else {
        harness_device_t previous=s->devices[index];
        if(d.fetched_at<previous.fetched_at)return false;
        if(d.fetched_at==previous.fetched_at && memcmp(&previous,&d,sizeof(d)))return false;
        previous.fetched_at=d.fetched_at;*changed=memcmp(&previous,&d,sizeof(d))!=0;
    }
    s->devices[index]=d;return true;
}
void harness_stats_total(const harness_stats_t *s,int day,usage_record_t *out)
{
    usage_record_init(out);strcpy(out->name,"DeepSeek Harness");
    if(!s || !s->count || !harness_stats_valid(s))return;
    int week=usage_week_start(day);if(!week)return;
    out->configured=out->has_data=out->usage_available=true;out->status=USAGE_OK;
    bool daily_known=false,weekly_known=false,monthly_known=false;
    out->token_day=day;out->token_week=week;out->daily_tokens=out->weekly_tokens=0;
    out->token_month=usage_month_start(day);out->monthly_tokens=0;
    for(unsigned i=0;i<s->count;++i) {
        const harness_device_t *d=&s->devices[i];
        if(d->fetched_at>out->fetched_at)out->fetched_at=d->fetched_at;
        if(d->day==day) {daily_known=true;out->daily_tokens+=d->daily;out->input_tokens+=d->input;out->output_tokens+=d->output;out->cached_tokens+=d->cached;out->calls+=d->calls;}
        if(d->week==week){weekly_known=true;out->weekly_tokens+=d->weekly;}
        if(d->month==out->token_month && d->monthly>=0){monthly_known=true;out->monthly_tokens+=d->monthly;}
    }
    if(!daily_known){out->daily_tokens=-1;out->usage_available=false;}
    if(!weekly_known)out->weekly_tokens=-1;
    if(!monthly_known)out->monthly_tokens=-1;
    out->missing_cost_calls=out->calls;
}
bool harness_stats_encode(const harness_stats_t *s,unsigned offset,char out[481])
{
    if(!s || offset>=HARNESS_MAX_DEVICES || (s->version && !harness_stats_valid(s)))return false;
    int n=snprintf(out,481,"{\"ok\":true,\"v\":1,\"count\":%u,\"offset\":%u,\"devices\":[",(unsigned)s->count,offset);
    for(unsigned i=offset;i<s->count && i<offset+1;++i) {
        const harness_device_t *d=&s->devices[i];
        n+=snprintf(out+n,481-(size_t)n,"%s{\"id\":\"%s\",\"name\":\"%s\",\"t\":%lld,\"d\":[%d,%lld],\"wt\":[%d,%lld],\"u\":[%lld,%lld,%lld,%lld]",i==offset?"":",",d->id,d->name,(long long)d->fetched_at,d->day,(long long)d->daily,d->week,(long long)d->weekly,(long long)d->input,(long long)d->output,(long long)d->cached,(long long)d->calls);
        if(d->month && d->monthly>=0)n+=snprintf(out+n,481-(size_t)n,",\"mt\":[%d,%lld]",d->month,(long long)d->monthly);
        n+=snprintf(out+n,481-(size_t)n,"}");
        if(n<0 || n>478)return false;
    }
    n+=snprintf(out+n,481-(size_t)n,"]}");return n>0&&n<=480;
}
