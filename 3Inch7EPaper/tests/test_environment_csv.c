#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include "environment_csv.h"
int main(void)
{
    setenv("TZ","SGT-8",1);tzset();
    environment_row_t r={1792497600,25.75f,61.2f,true,100},out;
    unsigned minute;char line[192];
    assert(environment_csv_format(line,sizeof(line),&r));
    assert(strstr(line,"2026-10-20T20:00:00+08:00"));
    assert(environment_csv_parse(line,20261020,&out,&minute)&&minute==1200&&out.valid&&fabsf(out.temperature-r.temperature)<.001f);
    assert(!environment_csv_parse(line,20261021,&out,&minute));
    r.timestamp=1792512000;assert(environment_csv_format(line,sizeof(line),&r));
    assert(environment_csv_parse(line,20261021,&out,&minute)&&minute==0);
    assert(!environment_csv_parse(line,20261020,&out,&minute));
    r.valid=false;r.battery=-1;r.temperature=NAN;assert(environment_csv_format(line,sizeof(line),&r));
    assert(strstr(line,",,,0,-1\n")&&environment_csv_parse(line,20261021,&out,&minute)&&!out.valid);
    assert(!environment_csv_parse("2026-10-21T00:00:00+08:00,1792512000,nan,55,1,100\n",20261021,&out,&minute));
    assert(!environment_csv_parse("2026-10-21T00:00:00+08:00,1792512000,23,55,1,",20261021,&out,&minute));
    assert(!environment_csv_parse("2026-10-20T23:00:00+08:00,1792512000,23,55,1,100\n",20261021,&out,&minute));
    r.timestamp=0;assert(!environment_csv_format(line,sizeof(line),&r));
    puts("Minute CSV timestamps, midnight rollover, missing versus zero, and damaged rows: OK");
}
