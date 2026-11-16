#include <assert.h>
#include <stdio.h>
#include "refresh_policy.h"
int main(void)
{
    refresh_policy_t s={0};
    assert(refresh_decide(&s,true,false,7,100)==REFRESH_FULL);
    refresh_commit(&s,REFRESH_FULL,true,7,100);
    assert(refresh_decide(&s,false,false,7,200)==REFRESH_SKIP);
    assert(refresh_decide(&s,true,false,7,200)==REFRESH_FULL); // Unconfirmed panel stays full.
    s.enabled=true;assert(refresh_decide(&s,true,false,7,200)==REFRESH_DIFFERENTIAL);
    assert(refresh_decide(&s,true,false,8,200)==REFRESH_FULL); // Page/orientation/layout change.
    assert(refresh_decide(&s,false,true,7,200)==REFRESH_FULL); // User recovery, even identical pixels.
    for(int i=0;i<20;++i)refresh_commit(&s,REFRESH_DIFFERENTIAL,true,7,200+i);
    assert(refresh_decide(&s,true,false,7,300)==REFRESH_DIFFERENTIAL);
    refresh_commit(&s,REFRESH_FULL,true,7,300);
    assert(refresh_decide(&s,true,false,7,1800000300LL)==REFRESH_DIFFERENTIAL);
    refresh_commit(&s,REFRESH_DIFFERENTIAL,false,7,400);
    assert(!s.ready && refresh_decide(&s,true,false,7,401)==REFRESH_FULL);
    puts("Refresh recovery, visual opt-in, no-op, page/rotation full, no periodic cleaning: OK");
}
