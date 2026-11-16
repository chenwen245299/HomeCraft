#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "dashboard.h"
#include "refresh_policy.h"
int main(int argc,char **argv)
{
    unsigned cases=0;
    uint8_t guarded[SCREEN_BYTES+32];
    for(unsigned rotation=0;rotation<360;rotation+=90)for(unsigned state=0;state<=MUSIC_ERROR;++state)for(unsigned count_case=0;count_case<4;++count_case) {
        memset(guarded,0xa5,sizeof(guarded));canvas_t c={.pixels=guarded+16,.rotation=rotation};
        dashboard_state_t s={.page=PAGE_MUSIC,.auto_rotate=true,.power={.valid=true,.usb_present=true,.percent=100}};
        s.music.state=state;s.music.count=(unsigned[]){0,1,8,128}[count_case];s.music.volume=95;s.music.sample_rate=44100;
        s.music.selected=s.music.count?s.music.count-1:0;s.music.first_row=(s.music.selected/6)*6;
        s.music.rows=s.music.count-s.music.first_row;if(s.music.rows>6)s.music.rows=6;
        s.music.elapsed_seconds=123;s.music.format=MUSIC_FORMAT_FLAC;
        strcpy(s.music.title,"邓丽君 - 月亮代表我的心.flac");strcpy(s.music.error,"文件损坏或编码不支持");
        for(unsigned i=0;i<s.music.rows;++i)snprintf(s.music.names[i],sizeof(s.music.names[i]),"%03u 中文歌曲很长的名称与英文 Song.FLAC",s.music.first_row+i+1);
        dashboard_render(&c,&s);
        for(unsigned i=0;i<16;++i)assert(guarded[i]==0xa5&&guarded[SCREEN_BYTES+16+i]==0xa5);
        assert(dashboard_page_available(&s,PAGE_MUSIC));
        refresh_policy_t policy={.enabled=true};uint32_t key=dashboard_layout_key(&s,&c);refresh_commit(&policy,REFRESH_FULL,true,key,0);
        assert(refresh_decide(&policy,true,false,key,1)==REFRESH_DIFFERENTIAL);
        if(argc>1&&state==MUSIC_PLAYING&&count_case==2) {
            char path[512];snprintf(path,sizeof(path),"%s/music-%u.pgm",argv[1],rotation);
            FILE *f=fopen(path,"wb");assert(f);fprintf(f,"P5\n800 480\n255\n");
            for(unsigned i=0;i<SCREEN_BYTES;++i)for(unsigned bit=0;bit<8;++bit)fputc((c.pixels[i]&(0x80>>bit))?255:0,f);fclose(f);
        }
        ++cases;
    }
    printf("Music renderer: %u states/counts/orientations, navigation and differential cases passed\n",cases);
}
