#include "music_library.h"
#include <dirent.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/stat.h>

music_format_t music_file_format(const char *name)
{
    const char *ext=name?strrchr(name,'.'):NULL;if(!ext)return MUSIC_FORMAT_NONE;
    const char *extensions[]={".mp3",".wav",".flac",".aac",".m4a",".ogg"};
    for(unsigned i=0;i<6;++i)if(!strcasecmp(ext,extensions[i]))return (music_format_t)(i+1);
    if(!strcasecmp(ext,".opus")||!strcasecmp(ext,".oga"))return MUSIC_FORMAT_OGG;
    return MUSIC_FORMAT_NONE;
}
const char *music_format_label(music_format_t f)
{
    const char *names[]={"--","MP3","WAV","FLAC","AAC","M4A","OGG"};
    return f<=MUSIC_FORMAT_OGG?names[f]:"--";
}
bool music_filename_valid(const char *name)
{
    if(!name||!*name||name[0]=='.'||strlen(name)>MUSIC_NAME_MAX)return false;
    for(const unsigned char *p=(const unsigned char *)name;*p;++p)
        if(*p<32||*p==127||*p=='/'||*p=='\\')return false;
    return music_file_format(name)!=MUSIC_FORMAT_NONE;
}
void music_text_copy(char *out,size_t size,const char *text)
{
    if(!size)return;
    size_t n=strlen(text);if(n>=size)n=size-1;
    while(n && ((unsigned char)text[n]&0xc0)==0x80)--n;
    memcpy(out,text,n);out[n]=0;
}
static int compare_tracks(const void *a,const void *b)
{
    return strcasecmp(((const music_track_t *)a)->name,((const music_track_t *)b)->name);
}
int music_library_scan(music_library_t *l,const char *directory)
{
    l->count=l->skipped=0;
    if(mkdir(directory,0775)!=0&&errno!=EEXIST)return -errno;
    DIR *dir=opendir(directory);if(!dir)return -errno;
    struct dirent *entry;int result=0;
    while((entry=readdir(dir))) {
        if(!music_filename_valid(entry->d_name))continue;
        char path[320];int n=snprintf(path,sizeof(path),"%s/%s",directory,entry->d_name);
        if(n<0||n>=(int)sizeof(path)){++l->skipped;continue;}
        struct stat st;if(stat(path,&st)||!S_ISREG(st.st_mode)||st.st_size<=0)continue;
        if(l->count>=MUSIC_MAX_TRACKS){++l->skipped;continue;}
        music_track_t *track=&l->tracks[l->count++];music_text_copy(track->name,sizeof(track->name),entry->d_name);
        track->format=music_file_format(track->name);
    }
    if(closedir(dir))result=-errno;
    qsort(l->tracks,l->count,sizeof(l->tracks[0]),compare_tracks);return result;
}
