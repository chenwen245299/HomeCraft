#include "usage_breakdown.h"
#include "cJSON.h"
#include <math.h>
#include <string.h>
static bool integer(const cJSON *v, int64_t low,int64_t high,int64_t *out)
{
    if(!cJSON_IsNumber(v)||!isfinite(v->valuedouble)||floor(v->valuedouble)!=v->valuedouble||v->valuedouble<low||v->valuedouble>high)return false;
    *out=(int64_t)v->valuedouble;return true;
}
static bool group(const cJSON *rows,unsigned count,int64_t total,usage_rank_t out[3],uint8_t *length)
{
    if(!cJSON_IsArray(rows)||cJSON_GetArraySize(rows)!=(int)(count>3?3:count))return false;
    int64_t sum=0;unsigned n=0;
    for(const cJSON *row=rows->child;row;row=row->next) {
        const cJSON *name=cJSON_GetArrayItem(row,0);int64_t tokens;
        if(!cJSON_IsArray(row)||cJSON_GetArraySize(row)!=2||!cJSON_IsString(name)||!name->valuestring[0]||strlen(name->valuestring)>32||
           !integer(cJSON_GetArrayItem(row,1),0,9007199254740991LL,&tokens))return false;
        for(const unsigned char *p=(void*)name->valuestring;*p;++p)
            if(!((*p>='a'&&*p<='z')||(*p>='A'&&*p<='Z')||(*p>='0'&&*p<='9')||strchr(" ._-/~",*p)))return false;
        if(!strncmp(name->valuestring,"sk-",3)||!strncmp(name->valuestring,"Bearer ",7))return false;
        for(unsigned i=0;i<n;++i)if(!strcmp(out[i].label,name->valuestring))return false;
        sum+=tokens; if(sum>total)return false;
        strcpy(out[n].label,name->valuestring);out[n++].tokens=tokens;
    }
    if(count>3 && strcmp(out[2].label,"Other"))return false;
    *length=n;return !count||sum==total;
}
bool usage_breakdown_decode(const char *text,usage_breakdown_t *out)
{
    if(!text||!out||strlen(text)>480)return false;
    cJSON *root=cJSON_ParseWithOpts(text,NULL,true);
    const char *keys[]={"v","p","t","d","g","m","r"};unsigned seen=0;
    bool ok=cJSON_IsObject(root);
    for(const cJSON *item=ok?root->child:NULL;item;item=item->next) {
        unsigned i=0;while(i<7&&strcmp(item->string,keys[i]))++i;
        if(i==7||(seen&(1U<<i))){ok=false;break;}seen|=1U<<i;
    }
    usage_breakdown_t b={0};int64_t v,p,day,mc,pc;
    const cJSON *d=cJSON_GetObjectItemCaseSensitive(root,"d"),*g=cJSON_GetObjectItemCaseSensitive(root,"g");
    ok=ok&&seen==127&&integer(cJSON_GetObjectItemCaseSensitive(root,"v"),1,1,&v)&&integer(cJSON_GetObjectItemCaseSensitive(root,"p"),3,3,&p)&&
        integer(cJSON_GetObjectItemCaseSensitive(root,"t"),1735689600,4102444800LL,&b.fetched_at)&&
        cJSON_IsArray(d)&&cJSON_GetArraySize(d)==2&&integer(cJSON_GetArrayItem(d,0),20250101,20991231,&day)&&usage_week_start((int)day)>0&&
        integer(cJSON_GetArrayItem(d,1),0,9007199254740991LL,&b.total)&&
        cJSON_IsArray(g)&&cJSON_GetArraySize(g)==2&&integer(cJSON_GetArrayItem(g,0),0,4096,&mc)&&integer(cJSON_GetArrayItem(g,1),0,4096,&pc);
    if(ok){b.day=(int)day;b.model_count=(unsigned)mc;b.provider_count=(unsigned)pc;
        ok=group(cJSON_GetObjectItemCaseSensitive(root,"m"),b.model_count,b.total,b.models,&b.model_rows)&&
           group(cJSON_GetObjectItemCaseSensitive(root,"r"),b.provider_count,b.total,b.providers,&b.provider_rows);}
    cJSON_Delete(root);
    if(ok){b.received=true;*out=b;}return ok;
}
bool usage_breakdown_matches(const usage_breakdown_t *b,const usage_record_t *r)
{
    return b->received&&r->has_data&&b->day==r->token_day&&b->total==r->daily_tokens&&b->fetched_at==r->fetched_at;
}
