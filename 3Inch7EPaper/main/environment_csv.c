#include "environment_csv.h"
#include <stdio.h>
#include <string.h>
#include <math.h>
#include <time.h>
static bool end(const char *s){return !*s || !strcmp(s,"\n") || !strcmp(s,"\r\n");}
bool environment_csv_parse(const char *line,int day,environment_row_t *out,unsigned *minute)
{
    if(!line || strlen(line)>191)return false;
    char iso[32],expected[40];long long seconds;float temperature=0,humidity=0;int flag=0,battery=-1,used=0;
    bool parsed=sscanf(line,"%31[^,],%lld,%f,%f,%d,%d%n",iso,&seconds,&temperature,&humidity,&flag,&battery,&used)==6 && end(line+used) && flag==1 && isfinite(temperature) && isfinite(humidity) && temperature>=-45 && temperature<=130 && humidity>=0 && humidity<=100;
    if(!parsed){used=0;if(sscanf(line,"%31[^,],%lld,,,0,%d%n",iso,&seconds,&battery,&used)!=3 || !used || !end(line+used))return false;flag=0;}
    if(seconds<1735689600 || seconds>4102444800 || battery<-1 || battery>100)return false;
    time_t ts=seconds;struct tm local;localtime_r(&ts,&local);
    if((local.tm_year+1900)*10000+(local.tm_mon+1)*100+local.tm_mday!=day || local.tm_sec!=0)return false;
    strftime(expected,sizeof(expected),"%Y-%m-%dT%H:%M:%S+08:00",&local);
    if(strcmp(iso,expected))return false;
    *out=(environment_row_t){seconds,temperature,humidity,flag==1,battery};
    *minute=local.tm_hour*60+local.tm_min;return true;
}
bool environment_csv_format(char *line,size_t size,const environment_row_t *r)
{
    if(r->timestamp<1735689600 || r->timestamp>4102444800 || r->battery<-1 || r->battery>100)return false;
    if(r->valid && (!isfinite(r->temperature)||!isfinite(r->humidity)||r->temperature<-45||r->temperature>130||r->humidity<0||r->humidity>100))return false;
    time_t ts=r->timestamp;struct tm local;localtime_r(&ts,&local);if(local.tm_sec)return false;
    char iso[40];strftime(iso,sizeof(iso),"%Y-%m-%dT%H:%M:%S+08:00",&local);
    int n=r->valid?snprintf(line,size,"%s,%lld,%.2f,%.2f,1,%d\n",iso,(long long)ts,(double)r->temperature,(double)r->humidity,r->battery):snprintf(line,size,"%s,%lld,,,0,%d\n",iso,(long long)ts,r->battery);
    return n>0 && (size_t)n<size;
}
