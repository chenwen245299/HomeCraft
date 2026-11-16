#include <assert.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include "dashboard.h"
#include "refresh_policy.h"

static uint8_t previous[SCREEN_BYTES],expected[SCREEN_BYTES],patched[SCREEN_BYTES],mask[SCREEN_BYTES];
// Deterministic time keeps header minute/day changes out of this power-only audit.
time_t time(time_t *out) {time_t t=1792936800;if(out)*out=t;return t;}

int main(void)
{
    const power_status_t states[]={
        {.valid=true,.battery_present=true,.percent=73},
        {.valid=true,.battery_present=true,.usb_present=true,.charging=true,.percent=73},
        {.valid=true,.battery_present=true,.usb_present=true,.percent=100},
        {.valid=true,.battery_present=true,.percent=100},
        {.valid=true,.battery_present=true,.percent=0},
        {.valid=true,.usb_present=true,.percent=-1},
        {.percent=-1},
        {.valid=true,.battery_present=true,.percent=-1},
    };
    const unsigned transitions[][2]={{0,1},{1,3},{2,0},{3,5},{5,0},{0,4},{4,6},{6,7}};
    unsigned cases=0;
    for(unsigned rot=0;rot<360;rot+=90)for(unsigned page=0;page<PAGE_COUNT;++page)for(unsigned paused=0;paused<2;++paused) {
        dashboard_state_t s={.page=page,.paused=paused,.sensor_valid=true,.imu_valid=true,.auto_rotate=true,.temperature=25,.humidity=50};
        for(unsigned i=0;i<PROVIDER_COUNT;++i) {
            usage_record_init(&s.network.providers[i]);
            s.network.providers[i].configured=s.network.providers[i].has_data=true;
            s.network.providers[i].status=USAGE_OK;
            s.network.providers[i].fetched_at=time(NULL);
        }
        canvas_t a={.pixels=previous,.rotation=rot},b={.pixels=expected,.rotation=rot},p={.pixels=patched,.rotation=rot};
        canvas_t m={.pixels=mask,.rotation=rot};
        memset(mask,0xff,sizeof(mask));
        int y=canvas_width(&m)<canvas_height(&m)?40:20;
        canvas_fill(&m,canvas_width(&m)-104,y-4,104,32,true);
        for(unsigned edge=0;edge<sizeof(transitions)/sizeof(transitions[0]);++edge) {
            s.power=states[transitions[edge][0]];dashboard_render(&a,&s);
            dashboard_state_t next=s;next.power=states[transitions[edge][1]];
            dashboard_render(&b,&next);
            memcpy(patched,previous,sizeof(patched));dashboard_render_power(&p,&next.power,next.paused);
            if(memcmp(patched,expected,SCREEN_BYTES)!=0) {
                fprintf(stderr,"Power patch mismatch rot=%u page=%u edge=%u\n",rot,page,edge);
                for(unsigned i=0;i<SCREEN_BYTES;++i)if(patched[i]!=expected[i]) {
                    fprintf(stderr,"first byte=%u physical_xy=%u,%u expected=%02x patched=%02x\n",i,(i%100)*8,i/100,expected[i],patched[i]);break;
                }
                assert(0);
            }
            for(unsigned i=0;i<SCREEN_BYTES;++i)assert(((patched[i]^previous[i])&mask[i])==0);
            uint32_t key=dashboard_layout_key(&s,&a);
            assert(key==dashboard_layout_key(&next,&b));
            refresh_policy_t policy={.enabled=true};refresh_commit(&policy,REFRESH_FULL,true,key,0);
            bool changed=memcmp(previous,patched,SCREEN_BYTES)!=0;
            assert(refresh_decide(&policy,changed,false,key,100000)==(changed?REFRESH_DIFFERENTIAL:REFRESH_SKIP));
            // A new sensor reading must stay offscreen during the power-only patch.
            next.temperature=39.5f;next.humidity=10.0f;
            memcpy(patched,previous,sizeof(patched));dashboard_render_power(&p,&next.power,next.paused);
            assert(memcmp(patched,expected,SCREEN_BYTES)==0);
            ++cases;
        }
    }
    printf("Power header: %u page/orientation/edge cases; body preservation and differential policy: OK\n",cases);
}
