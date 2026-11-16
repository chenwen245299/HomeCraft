// Generated from Apple Foundation Chinese Calendar, Asia/Shanghai, 2024-2100.
// Independent date checks use Hong Kong Observatory Gregorian-Lunar tables.
#include "lunar_calendar.h"
#include <stdio.h>
#include <string.h>
static const struct {int new_year;unsigned short mask;unsigned char leap,months;} years[]={
    {20240210,1746,0,12},
    {20250129,3749,6,13},
    {20260217,1829,0,12},
    {20270206,1611,0,12},
    {20280126,3223,5,13},
    {20290213,3243,0,12},
    {20300203,1370,0,12},
    {20310123,2774,3,13},
    {20320211,2921,0,12},
    {20330131,5970,11,13},
    {20340219,2898,0,12},
    {20350208,2853,0,12},
    {20360128,6731,6,13},
    {20370215,2635,0,12},
    {20380204,1195,0,12},
    {20390124,1371,5,13},
    {20400212,1453,0,12},
    {20410201,2922,0,12},
    {20420122,6994,2,13},
    {20430210,3474,0,12},
    {20440130,7461,7,13},
    {20450217,3365,0,12},
    {20460206,2645,0,12},
    {20470126,5293,5,13},
    {20480214,1206,0,12},
    {20490202,1461,0,12},
    {20500123,3498,3,13},
    {20510211,3785,0,12},
    {20520201,7826,8,13},
    {20530219,3730,0,12},
    {20540208,3366,0,12},
    {20550128,2646,6,13},
    {20560215,2647,0,12},
    {20570204,1238,0,12},
    {20580124,1749,4,13},
    {20590212,1877,0,12},
    {20600202,1865,0,12},
    {20610121,3731,3,13},
    {20620209,1683,0,12},
    {20630129,5419,7,13},
    {20640217,1323,0,12},
    {20650205,2651,0,12},
    {20660126,5466,5,13},
    {20670214,1386,0,12},
    {20680203,2917,0,12},
    {20690123,5962,4,13},
    {20700211,2890,0,12},
    {20710131,6805,8,13},
    {20720219,2709,0,12},
    {20730207,1325,0,12},
    {20740127,2733,6,13},
    {20750215,2741,0,12},
    {20760205,1450,0,12},
    {20770124,2981,4,13},
    {20780212,3493,0,12},
    {20790202,3402,0,12},
    {20800122,7317,3,13},
    {20810209,3222,0,12},
    {20820129,6478,7,13},
    {20830217,1366,0,12},
    {20840206,2741,0,12},
    {20850126,5554,5,13},
    {20860214,1746,0,12},
    {20870203,3749,0,12},
    {20880124,3658,4,13},
    {20890210,1675,0,12},
    {20900130,3223,8,13},
    {20910218,1195,0,12},
    {20920207,1371,0,12},
    {20930127,2774,6,13},
    {20940215,2922,0,12},
    {20950205,1874,0,12},
    {20960125,5925,4,13},
    {20970212,2853,0,12},
    {20980201,2699,0,12},
    {20990121,5275,2,13},
    {21000209,1195,0,12},
};
static int ordinal(int day)
{
    int y=day/10000,m=day/100%100,d=day%100;
    static const int before[]={0,31,59,90,120,151,181,212,243,273,304,334};
    return 365*(y-1)+(y-1)/4-(y-1)/100+(y-1)/400+before[m-1]+d+(m>2 && y%4==0 && (y%100!=0 || y%400==0));
}
bool lunar_calendar_date(int date,lunar_date_t *out)
{
    int y=date/10000,m=date/100%100,d=date%100;
    if(y<2024 || y>2100 || m<1 || m>12 || d<1 || d>31)return false;
    static const int lengths[]={31,28,31,30,31,30,31,31,30,31,30,31};
    int limit=lengths[m-1]+(m==2 && y%4==0 && (y%100!=0 || y%400==0));if(d>limit)return false;
    int index=y-2024;if(date<years[index].new_year)--index;if(index<0)return false;
    int remaining=ordinal(date)-ordinal(years[index].new_year);int month=1;
    for(unsigned i=0;i<years[index].months;++i) {
        int length=29+((years[index].mask>>i)&1);
        bool leap=years[index].leap && i==years[index].leap;
        if(remaining<length){*out=(lunar_date_t){month,remaining+1,leap};return true;}
        remaining-=length;
        if(!years[index].leap || i+1!=years[index].leap)++month;
    }
    return false;
}
bool lunar_calendar_text(int date,char out[48])
{
    static const char *months[]={"正","二","三","四","五","六","七","八","九","十","冬","腊"};
    static const char *numbers[]={"一","二","三","四","五","六","七","八","九","十"};
    lunar_date_t lunar;if(!lunar_calendar_date(date,&lunar)){strcpy(out,"农历 --");return false;}
    char day[16];
    if(lunar.day<=10)snprintf(day,sizeof(day),"初%s",numbers[lunar.day-1]);
    else if(lunar.day<20)snprintf(day,sizeof(day),"十%s",numbers[lunar.day-11]);
    else if(lunar.day==20)strcpy(day,"二十");
    else if(lunar.day<30)snprintf(day,sizeof(day),"廿%s",numbers[lunar.day-21]);
    else strcpy(day,"三十");
    snprintf(out,48,"农历 %s%s月%s",lunar.leap?"闰":"",months[lunar.month-1],day);return true;
}
