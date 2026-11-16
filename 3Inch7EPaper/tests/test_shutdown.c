#include <assert.h>
#include <stdio.h>
#include <string.h>
#include "shutdown.h"
typedef struct { char order[16]; unsigned count, fail; } fixture_t;
static bool step(void *context, char ch, unsigned failure)
{
    fixture_t *f=context;f->order[f->count++]=ch;return f->fail!=failure;
}
static bool storage(void *c){return step(c,'S',1);}
static bool network(void *c){return step(c,'N',2);}
static bool frame(void *c){return step(c,'F',3);}
static void display_sleep(void *c){step(c,'D',0);}
static void power_off(void *c){step(c,'P',0);}
static void standby(void *c){step(c,'Z',0);}
static void resume(void *c){step(c,'R',0);}
int main(void)
{
    shutdown_ops_t ops={storage,network,frame,display_sleep,power_off,standby,resume};
    fixture_t f={0};
    assert(shutdown_run(&ops,&f)==SHUTDOWN_STANDBY);
    assert(!strcmp(f.order,"SNFDPZ")); // No power cut before closed files and completed full frame.
    f=(fixture_t){.fail=1};assert(shutdown_run(&ops,&f)==SHUTDOWN_STORAGE_BUSY);
    assert(!strcmp(f.order,"SR"));
    f=(fixture_t){.fail=2};assert(shutdown_run(&ops,&f)==SHUTDOWN_NETWORK_BUSY);
    assert(!strcmp(f.order,"SNR"));
    f=(fixture_t){.fail=3};assert(shutdown_run(&ops,&f)==SHUTDOWN_DISPLAY_FAILED);
    assert(!strcmp(f.order,"SNFR")); // No sleeping/power-off with a stale or failed screen.
    puts("Shutdown ordering and storage/network/display failure cancellation: OK");
}
