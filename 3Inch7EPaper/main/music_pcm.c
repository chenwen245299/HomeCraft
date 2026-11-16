#include <stdbool.h>
#include "music_pcm.h"
bool music_pcm_supported(unsigned rate,unsigned bits,unsigned channels)
{
    return rate>=8000&&rate<=96000&&(bits==8||bits==16||bits==24||bits==32)&&(channels==1||channels==2);
}
static int16_t sample(const uint8_t *p,unsigned bits)
{
    if(bits==8)return (int16_t)(((int)p[0]-128)*256);
    if(bits==16)return (int16_t)((unsigned)p[0]|((unsigned)p[1]<<8));
    if(bits==24) {
        int32_t v=(int32_t)((uint32_t)p[0]|((uint32_t)p[1]<<8)|((uint32_t)p[2]<<16));
        if(v&0x800000)v-=0x1000000;
        return (int16_t)(v/256);
    }
    uint32_t u=(uint32_t)p[0]|((uint32_t)p[1]<<8)|((uint32_t)p[2]<<16)|((uint32_t)p[3]<<24);
    int64_t v=u;if(u&0x80000000U)v-=INT64_C(4294967296);return (int16_t)(v/65536);
}
size_t music_pcm_stereo16(const uint8_t *p,size_t bytes,unsigned bits,unsigned channels,int16_t out[1024])
{
    if(!music_pcm_supported(44100,bits,channels))return 0;
    size_t stride=(bits/8)*channels,n=bytes/stride;if(n>512)n=512;
    for(size_t i=0;i<n;++i) {
        int32_t left=sample(p+i*stride,bits),right=channels==1?left:sample(p+i*stride+bits/8,bits);
        // The board has one speaker: include both source channels without digital clipping.
        out[i*2]=out[i*2+1]=(int16_t)((left+right)/2);
    }
    return n;
}
