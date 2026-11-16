#include "usage_breakdown.h"
#include "cJSON.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(void) {
    FILE *f=fopen("../software/tests/fixtures/argus-breakdown-v1.json","rb");assert(f);char text[481]={0};size_t n=fread(text,1,480,f);assert(n>0 && n<480);fclose(f);
    usage_breakdown_t b;assert(usage_breakdown_decode(text,&b));assert(b.model_count==3 && b.provider_count==2 && b.models[0].tokens==70 && !strcmp(b.providers[0].label,"ID 11111111"));
    usage_record_t r;usage_record_init(&r);r.has_data=true;r.token_day=20261021;r.daily_tokens=120;r.fetched_at=1792584000;assert(usage_breakdown_matches(&b,&r));
    ++r.fetched_at;assert(!usage_breakdown_matches(&b,&r));--r.fetched_at;++r.daily_tokens;assert(!usage_breakdown_matches(&b,&r));
    for(unsigned i=0;i<8;++i) {
        cJSON *root=cJSON_Parse(text);
        if(i==0)cJSON_AddNumberToObject(root,"p",3);
        if(i==1)cJSON_AddStringToObject(root,"access_token","never-forward");
        if(i==2)cJSON_SetNumberValue(cJSON_GetObjectItem(root,"p"),0);
        if(i==3)cJSON_SetNumberValue(cJSON_GetArrayItem(cJSON_GetObjectItem(root,"d"),0),20260230);
        if(i==4)cJSON_SetNumberValue(cJSON_GetArrayItem(cJSON_GetObjectItem(root,"d"),1),119);
        if(i==5)cJSON_SetNumberValue(cJSON_GetArrayItem(cJSON_GetObjectItem(root,"g"),0),4);
        if(i==6)cJSON_SetValuestring(cJSON_GetArrayItem(cJSON_GetArrayItem(cJSON_GetObjectItem(root,"m"),0),0),"sk-private");
        if(i==7)cJSON_SetNumberValue(cJSON_GetArrayItem(cJSON_GetArrayItem(cJSON_GetObjectItem(root,"m"),0),1),1.5);
        char *invalid=cJSON_PrintUnformatted(root);assert(!usage_breakdown_decode(invalid,&b));free(invalid);cJSON_Delete(root);
    }
    assert(!usage_breakdown_decode("{}",&b));assert(!usage_breakdown_decode("null",&b));assert(!usage_breakdown_decode("[]",&b));
    puts("Argus breakdown: shared Rust/C fixture, strict fields, sums, labels and stale-generation rejection: OK");
}
