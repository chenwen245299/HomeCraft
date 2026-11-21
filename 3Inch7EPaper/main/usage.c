#include "usage.h"
#include "cJSON.h"
#include <errno.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static const cJSON *field(const cJSON *object, const char *key)
{
    return cJSON_GetObjectItemCaseSensitive(object, key);
}

static bool number(const cJSON *item, double *out)
{
    if (cJSON_IsNumber(item) && isfinite(item->valuedouble)) { *out = item->valuedouble; return true; }
    return false;
}

static bool packet_integer(const cJSON *item, int64_t low, int64_t high, int64_t *out)
{
    double value;
    if (!number(item, &value) || floor(value) != value || value < low || value > high) return false;
    *out = (int64_t)value; return true;
}

static int days_in_month(int year, int month)
{
    static const unsigned days[]={31,28,31,30,31,30,31,31,30,31,30,31};
    if(month<1 || month>12)return 0;
    return days[month-1]+(month==2 && year%4==0 && (year%100!=0 || year%400==0));
}

static bool valid_date(int day)
{
    int year=day/10000,month=day/100%100,date=day%100;
    return year>=2024 && year<=2099 && date>=1 && date<=days_in_month(year,month);
}

int usage_week_start(int day)
{
    if(!valid_date(day))return 0;
    int year=day/10000,month=day/100%100,date=day%100;
    static const int shifts[]={0,3,2,5,0,3,5,1,4,6,2,4};
    int y=year-(month<3);
    int sunday=(y+y/4-y/100+y/400+shifts[month-1]+date)%7;
    date-=(sunday+6)%7;
    if(date<1) {if(--month==0){month=12;--year;} date+=days_in_month(year,month);}
    return year*10000+month*100+date;
}

int64_t usage_clock_target(int64_t current, int64_t fetched_at)
{
    if(fetched_at<INT64_C(1735689600) || fetched_at>INT64_C(4102444800))return current;
    if(current<INT64_C(1735689600) || current>INT64_C(4102444800))return fetched_at;
    return fetched_at>current+2?fetched_at:current;
}
int usage_month_start(int day){return valid_date(day)?day-day%100+1:0;}

bool usage_totals_decode(const char *text, usage_totals_t *out)
{
    if(!text || !out || strlen(text)>480)return false;
    cJSON *json=cJSON_ParseWithOpts(text,NULL,true);
    bool valid=cJSON_IsObject(json);
    const char *keys[]={"v","op","d","wt","mt"};
    unsigned seen=0;
    for(const cJSON *item=valid?json->child:NULL;item;item=item->next){
        unsigned i=0;
        while(i<5 && strcmp(item->string,keys[i]))++i;
        if(i==5 || (seen&(1U<<i))){valid=false;break;}
        seen|=1U<<i;
    }
    int64_t version;
    const cJSON *op=field(json,"op");
    valid=valid && seen==31 && packet_integer(field(json,"v"),1,1,&version) &&
        cJSON_IsString(op) && !strcmp(op->valuestring,"totals");
    usage_totals_t fresh={.received=true};
    for(unsigned i=0;valid && i<3;++i){
        const cJSON *pair=field(json,keys[i+2]);int64_t period;
        valid=cJSON_IsArray(pair) && cJSON_GetArraySize(pair)==2 &&
            packet_integer(cJSON_GetArrayItem(pair,0),20241230,20991231,&period) &&
            packet_integer(cJSON_GetArrayItem(pair,1),-1,9007199254740991LL,&fresh.tokens[i]);
        if(valid)fresh.periods[i]=(int)period;
    }
    valid=valid && usage_week_start(fresh.periods[0])!=0 &&
        usage_week_start(fresh.periods[0])==fresh.periods[1] &&
        usage_month_start(fresh.periods[0])==fresh.periods[2];
    cJSON_Delete(json);
    if(valid)*out=fresh;
    return valid;
}

int usage_previous_day(int day){
    if(!valid_date(day))return 0;
    int y=day/10000,m=day/100%100,d=day%100;
    if(--d==0){if(--m==0){m=12;--y;}d=days_in_month(y,m);}
    return y*10000+m*100+d;
}

bool usage_packet_decode(const char *text, provider_id_t *provider, usage_record_t *out)
{
    if (!text || strlen(text) > 480) return false;
    cJSON *json = cJSON_ParseWithOpts(text, NULL, true);
    bool valid = cJSON_IsObject(json);
    const char *keys[] = {"v", "p", "t", "s", "w", "b", "c", "mu", "ml", "d", "ld", "n", "u", "dc", "wt", "mt", "yd"};
    const unsigned key_count=sizeof(keys)/sizeof(keys[0]);
    unsigned seen = 0;
    for (const cJSON *item = valid ? json->child : NULL; item; item = item->next) {
        unsigned i;
        for (i = 0; i < key_count && strcmp(item->string, keys[i]); ++i) {}
        if (i == key_count || (seen & (1U << i))) { valid = false; break; }
        seen |= 1U << i;
    }
    int64_t version, p, timestamp, status;
    valid = valid && packet_integer(field(json,"v"),1,1,&version) &&
        packet_integer(field(json,"p"),0,PROVIDER_COUNT-1,&p) &&
        packet_integer(field(json,"t"),1735689600,4102444800,&timestamp) &&
        packet_integer(field(json,"s"),USAGE_NOT_CONFIGURED,USAGE_ACCESS_DENIED,&status);
    usage_record_t fresh; usage_record_init(&fresh);
    if (valid) {
        fresh.fetched_at = timestamp; fresh.status = status;
        fresh.configured = status != USAGE_NOT_CONFIGURED;
        fresh.has_data = status == USAGE_OK;
        const cJSON *windows = field(json,"w");
        if (windows) {
            valid = cJSON_IsArray(windows) && cJSON_GetArraySize(windows) == 3;
            for (unsigned i = 0; valid && i < 3; ++i) {
                const cJSON *w = cJSON_GetArrayItem(windows,i); int64_t r,m,t;
                valid = cJSON_IsArray(w) && cJSON_GetArraySize(w) == 3 &&
                    packet_integer(cJSON_GetArrayItem(w,0),-1,1000,&r) &&
                    packet_integer(cJSON_GetArrayItem(w,1),0,525600,&m) &&
                    packet_integer(cJSON_GetArrayItem(w,2),0,4102444800,&t);
                if (valid) fresh.windows[i] = (quota_window_t){r,m,t};
            }
        }
        if (field(json,"b")) {
            const cJSON *currency = field(json,"c");
            valid = valid && number(field(json,"b"),&fresh.balance) &&
                fabs(fresh.balance) <= 1e12 && cJSON_IsString(currency) &&
                (!strcmp(currency->valuestring,"CNY") || !strcmp(currency->valuestring,"USD"));
            if (valid) { strcpy(fresh.currency,currency->valuestring); fresh.balance_available = true; }
        }
        if (field(json,"mu") || field(json,"ml")) {
            valid = valid && number(field(json,"mu"),&fresh.monthly_used) && fresh.monthly_used >= 0 &&
                number(field(json,"ml"),&fresh.monthly_limit) && fresh.monthly_limit >= 0 &&
                fresh.monthly_used <= 1e12 && fresh.monthly_limit <= 1e12;
            fresh.monthly_spend_available = valid;
        }
        const char *daily_keys[] = {"d", "ld"};
        for (unsigned i=0; valid && i<2; ++i) {
            const cJSON *daily=field(json,daily_keys[i]);
            if (!daily) continue;
            int64_t day, tokens;
            valid=cJSON_IsArray(daily) && cJSON_GetArraySize(daily)==2 &&
                packet_integer(cJSON_GetArrayItem(daily,0),20250101,20991231,&day) &&
                packet_integer(cJSON_GetArrayItem(daily,1),-1,9007199254740991LL,&tokens);
            if (valid) {
                valid=valid_date(day);
                if(i==0) {fresh.token_day=day;fresh.daily_tokens=tokens;}
                else {fresh.latest_token_day=day;fresh.latest_daily_tokens=tokens;}
            }
        }
        const cJSON *weekly=field(json,"wt");
        if(valid && weekly) {
            int64_t day,tokens;
            valid=cJSON_IsArray(weekly) && cJSON_GetArraySize(weekly)==2 &&
                packet_integer(cJSON_GetArrayItem(weekly,0),20241230,20991231,&day) &&
                packet_integer(cJSON_GetArrayItem(weekly,1),0,9007199254740991LL,&tokens) &&
                usage_week_start(day)==day;
            if(valid){fresh.token_week=day;fresh.weekly_tokens=tokens;}
        }
        const cJSON *yesterday=field(json,"yd");
        if(valid && yesterday){
            int64_t day,tokens,used;
            valid=p==PROVIDER_CODEX && cJSON_IsArray(yesterday) && cJSON_GetArraySize(yesterday)==3 &&
                packet_integer(cJSON_GetArrayItem(yesterday,0),20241231,20991231,&day) && valid_date((int)day) &&
                packet_integer(cJSON_GetArrayItem(yesterday,1),-1,9007199254740991LL,&tokens) &&
                packet_integer(cJSON_GetArrayItem(yesterday,2),-1,1000,&used);
            if(valid){fresh.yesterday_day=(int)day;fresh.yesterday_tokens=tokens;fresh.yesterday_week_used_tenths=(int)used;}
        }
        const cJSON *name=field(json,"n");
        const cJSON *monthly=field(json,"mt");
        if(valid && monthly){
            int64_t day,tokens;
            valid=cJSON_IsArray(monthly) && cJSON_GetArraySize(monthly)==2 &&
                packet_integer(cJSON_GetArrayItem(monthly,0),20250101,20991201,&day) &&
                packet_integer(cJSON_GetArrayItem(monthly,1),0,9007199254740991LL,&tokens) && usage_month_start((int)day)==day;
            if(valid){fresh.token_month=(int)day;fresh.monthly_tokens=tokens;}
        }
        if(valid && name) {
            valid=p>=3 && cJSON_IsString(name) && name->valuestring[0] && strlen(name->valuestring)<=24;
            if(valid) for(const unsigned char *s=(void*)name->valuestring;*s;++s)
                if(!((*s>='A'&&*s<='Z')||(*s>='a'&&*s<='z')||(*s>='0'&&*s<='9')||strchr(" ._-",*s))) {valid=false;break;}
            if(valid && p==PROVIDER_ARGUS) valid=!strcmp(name->valuestring,"Argus");
            if(valid && p>=PROVIDER_EXTRA1) for(unsigned i=0;i<4;++i)
                if(!strcmp(name->valuestring,usage_provider_name(i))) valid=false;
            if(valid) strcpy(fresh.name,name->valuestring);
        }
        if(valid && p>=3 && !name) valid=false;
        const cJSON *usage=field(json,"u");
        if(valid && usage) {
            int64_t values[5];
            valid=cJSON_IsArray(usage) && cJSON_GetArraySize(usage)==5;
            for(unsigned i=0;valid&&i<5;++i) valid=packet_integer(cJSON_GetArrayItem(usage,i),0,9007199254740991LL,&values[i]);
            if(valid) valid=values[2]<=values[0] && values[4]<=values[3] && fresh.daily_tokens>=0 && values[0]+values[1]==fresh.daily_tokens;
            if(valid) {fresh.usage_available=true;fresh.input_tokens=values[0];fresh.output_tokens=values[1];fresh.cached_tokens=values[2];fresh.calls=values[3];fresh.missing_cost_calls=values[4];}
        }
        if(valid && field(json,"dc")) {
            valid=fresh.usage_available && number(field(json,"dc"),&fresh.daily_cost_usd) && fresh.daily_cost_usd>=0 && fresh.daily_cost_usd<=1e12;
            fresh.daily_cost_available=valid;
        }
    }
    if (valid) { *provider = p; *out = fresh; }
    cJSON_Delete(json); return valid;
}

bool usage_display_update(usage_record_t *current,const usage_record_t *incoming)
{
    if(!current || !incoming)return false;
    if(incoming->fetched_at<current->fetched_at)return true;
    *current=*incoming;
    return true;
}

void usage_summary_totals(const usage_totals_t *hub,int day,int64_t out[3])
{
    int periods[]={day,usage_week_start(day),usage_month_start(day)};
    for(unsigned i=0;i<3;++i)
        out[i]=hub && hub->received && periods[i] && hub->periods[i]==periods[i]
            ? hub->tokens[i] : -1;
}

static bool money(const cJSON *item, double *out)
{
    if (number(item, out)) return fabs(*out) < 1e12;
    if (!cJSON_IsString(item) || !item->valuestring[0]) return false;
    char *end;
    errno = 0;
    double value = strtod(item->valuestring, &end);
    if (*end || errno || !isfinite(value) || fabs(value) >= 1e12) return false;
    *out = value;
    return true;
}

static int remaining(const cJSON *item)
{
    double used;
    if (!number(item, &used)) return -1;
    return (int)lround(fmax(0, fmin(100, 100 - used)) * 10);
}

static int64_t epoch(const cJSON *item)
{
    double value;
    if (number(item, &value)) return value >= 1577836800 && value <= 4102444800 ? (int64_t)value : 0;
    if (!cJSON_IsString(item)) return 0;
    int year, month, day, hour, minute, second, used = 0;
    if (sscanf(item->valuestring, "%d-%d-%dT%d:%d:%d%n", &year, &month, &day, &hour, &minute, &second, &used) != 6 ||
        year < 2020 || year > 2100 || month < 1 || month > 12 || day < 1 || day > 31 ||
        hour < 0 || hour > 23 || minute < 0 || minute > 59 || second < 0 || second > 59) return 0;
    const char *tail = item->valuestring + used;
    if (*tail == '.') { ++tail; while (*tail >= '0' && *tail <= '9') ++tail; }
    int offset = 0;
    if ((*tail == 'Z' || *tail == 'z') && tail[1] == 0) { }
    else if (*tail == '+' || *tail == '-') {
        int oh, om, n = 0;
        if (sscanf(tail + 1, "%2d:%2d%n", &oh, &om, &n) != 2 || tail[1+n] || oh > 23 || om > 59) return 0;
        offset = (oh * 60 + om) * 60 * (*tail == '+' ? 1 : -1);
    } else return 0;
    // Civil date -> UTC epoch without changing the process timezone.
    year -= month <= 2;
    int era = year / 400, yoe = year - era * 400;
    int doy = (153 * (month + (month > 2 ? -3 : 9)) + 2) / 5 + day - 1;
    int64_t days = (int64_t)era * 146097 + yoe * 365 + yoe/4 - yoe/100 + doy - 719468;
    return days * 86400 + hour * 3600 + minute * 60 + second - offset;
}

void usage_record_init(usage_record_t *r)
{
    memset(r, 0, sizeof(*r));
    for (unsigned i = 0; i < 3; ++i) r->windows[i].remaining_tenths = -1;
    r->daily_tokens = r->latest_daily_tokens = r->weekly_tokens = r->monthly_tokens = -1;
    r->yesterday_tokens=-1;r->yesterday_week_used_tenths=-1;
}

bool usage_decode(provider_id_t provider, const char *json, int64_t now, usage_record_t *out)
{
    if (!json || strlen(json) > 32768 || now < 1577836800) return false;
    const char *end;
    cJSON *root = cJSON_ParseWithOpts(json, &end, true);
    if (!cJSON_IsObject(root)) { cJSON_Delete(root); return false; }
    usage_record_t r;
    usage_record_init(&r);
    bool valid = false;
    if (provider == PROVIDER_CODEX) {
        const cJSON *limits = field(root, "rate_limit");
        if (cJSON_IsObject(limits)) {
            valid = true;
            const char *keys[] = {"primary_window", "secondary_window"};
            for (unsigned i = 0; i < 2; ++i) {
                const cJSON *w = field(limits, keys[i]);
                if (!cJSON_IsObject(w)) continue;
                double seconds;
                int mins = number(field(w, "limit_window_seconds"), &seconds) && seconds > 0 && seconds <= 31536000 ? (int)(seconds / 60) : 0;
                unsigned slot = mins >= 27*1440 && mins <= 32*1440 ? 2 : (mins >= 5*1440 && mins <= 8*1440 ? 1 : 0);
                r.windows[slot] = (quota_window_t){remaining(field(w, "used_percent")), mins, epoch(field(w, "reset_at"))};
            }
        }
    } else if (provider == PROVIDER_CLAUDE) {
        const char *keys[] = {"five_hour", "seven_day", "monthly"};
        const int minutes[] = {300, 10080, 43200};
        for (unsigned i = 0; i < 3; ++i) {
            const cJSON *w = field(root, keys[i]);
            if (w) valid = true; // Explicit null windows remain unknown.
            if (!cJSON_IsObject(w)) continue;
            r.windows[i] = (quota_window_t){remaining(field(w, "utilization")), minutes[i], epoch(field(w, "resets_at"))};
        }
        const cJSON *extra = field(root, "extra_usage");
        double spent, limit;
        if (cJSON_IsTrue(field(extra, "is_enabled")) && money(field(extra, "used_credits"), &spent) && money(field(extra, "monthly_limit"), &limit) && spent >= 0 && limit >= 0) {
            r.monthly_spend_available = true;
            r.monthly_used = spent / 100; r.monthly_limit = limit / 100;
            strcpy(r.currency, "USD");
            // Extra-usage budget is money, never the subscription's monthly quota.
        }
    } else if (provider == PROVIDER_DEEPSEEK) {
        const cJSON *balances = field(root, "balance_infos");
        if (cJSON_IsArray(balances)) {
            const cJSON *row;
            cJSON_ArrayForEach(row, balances) {
                const cJSON *currency = field(row, "currency");
                double value;
                if (!cJSON_IsString(currency) ||
                    (strcmp(currency->valuestring, "CNY") && strcmp(currency->valuestring, "USD")) ||
                    !money(field(row, "total_balance"), &value)) continue;
                r.balance_available = true; r.balance = value;
                snprintf(r.currency, sizeof(r.currency), "%s", currency->valuestring);
                valid = true;
                if (!strcmp(r.currency, "CNY")) break;
            }
        }
    }
    cJSON_Delete(root);
    if (!valid) return false;
    r.fetched_at = now; r.configured = true; r.has_data = true; r.status = USAGE_OK;
    *out = r;
    return true;
}

const char *usage_provider_name(provider_id_t p)
{
    static const char *names[] = {"Codex", "Claude Code", "DeepSeek", "Argus", "Service 5", "Service 6", "Service 7", "Service 8"};
    return p < PROVIDER_COUNT ? names[p] : "Unknown";
}

const char *usage_record_name(provider_id_t p,const usage_record_t *r)
{
    return p>=3 && r->name[0]?r->name:usage_provider_name(p);
}

unsigned usage_overview_select(unsigned *out,unsigned capacity)
{
    static const unsigned services[]={PROVIDER_CODEX,PROVIDER_ARGUS,PROVIDER_EXTRA1};
    if(!out)return 0;
    unsigned used=0;
    while(used<capacity && used<sizeof(services)/sizeof(services[0])){
        out[used]=services[used];
        ++used;
    }
    return used;
}

const char *usage_status_text(usage_status_t status)
{
    static const char *names[] = {"未配置", "等待联网", "已同步", "登录已过期", "接口限流", "网络错误", "接口数据异常", "访问受限"};
    return status <= USAGE_ACCESS_DENIED ? names[status] : "未知状态";
}
