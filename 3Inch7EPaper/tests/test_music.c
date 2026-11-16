#define _DARWIN_C_SOURCE 1
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/stat.h>
#include "music_library.h"
#include "music_pcm.h"
static void file(const char *dir,const char *name)
{
    char path[512];snprintf(path,sizeof(path),"%s/%s",dir,name);FILE *f=fopen(path,"wb");assert(f);fputc(1,f);assert(!fclose(f));
}
int main(void)
{
    assert(music_file_format("你好.MP3")==MUSIC_FORMAT_MP3);
    assert(music_file_format("track.OPUS")==MUSIC_FORMAT_OGG);
    assert(!music_filename_valid("../auth.mp3"));assert(!music_filename_valid("x/y.mp3"));
    assert(!music_filename_valid("._track.mp3"));assert(!music_filename_valid("bad\nname.wav"));
    assert(!music_filename_valid("x.wma"));assert(!music_filename_valid("/x.mp3"));
    char cut[7];music_text_copy(cut,sizeof(cut),"中文歌曲");assert(!strcmp(cut,"中文"));
    music_text_copy(cut,5,"中文歌曲");assert(!strcmp(cut,"中"));
    char dir[]="/private/tmp/homecraft-music-test.XXXXXX";assert(mkdtemp(dir));
    file(dir,"B.MP3");file(dir,"中文.flac");file(dir,".hidden.wav");file(dir,"text.txt");
    char sub[512];snprintf(sub,sizeof(sub),"%s/sub.mp3",dir);assert(!mkdir(sub,0700));
    music_library_t *l=calloc(1,sizeof(*l));assert(l);assert(!music_library_scan(l,dir));assert(l->count==2);
    assert(!strcmp(l->tracks[0].name,"B.MP3"));assert(!strcmp(l->tracks[1].name,"中文.flac"));
    for(unsigned i=0;i<140;++i){char name[32];snprintf(name,sizeof(name),"%03u.wav",i);file(dir,name);}
    assert(!music_library_scan(l,dir));assert(l->count==MUSIC_MAX_TRACKS&&l->skipped==14);
    for(unsigned i=0;i<140;++i){char path[512];snprintf(path,sizeof(path),"%s/%03u.wav",dir,i);unlink(path);}
    const char *names[]={"B.MP3","中文.flac",".hidden.wav","text.txt"};
    for(unsigned i=0;i<4;++i){char path[512];snprintf(path,sizeof(path),"%s/%s",dir,names[i]);unlink(path);}rmdir(sub);rmdir(dir);free(l);
    int16_t pcm[1024];uint8_t eight[]={0,128,255};assert(music_pcm_stereo16(eight,3,8,1,pcm)==3);
    assert(pcm[0]==-32768&&pcm[1]==-32768&&pcm[2]==0&&pcm[4]==32512);
    uint8_t sixteen[]={0,128,255,127};assert(music_pcm_stereo16(sixteen,4,16,2,pcm)==1);assert(pcm[0]==0&&pcm[1]==0);
    uint8_t twentyfour[]={0,0,128,255,255,127};assert(music_pcm_stereo16(twentyfour,6,24,1,pcm)==2);assert(pcm[0]==-32768&&pcm[2]==32767);
    uint8_t thirtytwo[]={0,0,0,128,255,255,255,127};assert(music_pcm_stereo16(thirtytwo,8,32,2,pcm)==1);assert(pcm[0]==0&&pcm[1]==0);
    assert(!music_pcm_supported(192000,24,2));assert(!music_pcm_supported(44100,16,6));assert(!music_pcm_supported(44100,20,2));
    puts("Music library/PCM: extension whitelist, traversal, hidden/directory skip, UTF-8,128-file cap,8/16/24/32-bit and mono/stereo vectors passed");
}
