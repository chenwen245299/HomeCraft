#include "token_typography.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>

int main(void)
{
    char text[32];
    token_format_compact(text,sizeof(text),999949);assert(!strcmp(text,"999.9K"));
    token_format_compact(text,sizeof(text),999950);assert(!strcmp(text,"1.0M"));
    token_format_compact(text,sizeof(text),999950000);assert(!strcmp(text,"1.00B"));
    token_format_compact(text,sizeof(text),99995000000LL);assert(!strcmp(text,"100.0B"));
    token_format_compact(text,sizeof(text),999950000000LL);assert(!strcmp(text,"1.0T"));
    const int photo_widths[]={183,212};
    const int64_t photo_totals[]={14200000,54600000};
    const bool known[]={true,true};
    const unsigned preferred[]={digits96.height,digits64.height};
    token_number_layout_t result[2];
    token_pair_layout(photo_totals,known,photo_widths,preferred,result);
    assert(!strcmp(result[0].number,"14.2") && !strcmp(result[0].unit,"M"));
    assert(result[0].font==&digits80 && result[1].font==&digits64);
    const int widths[][2]={{183,212},{147,155},{249,278},{340,340},{241,270},{569,598}};
    const int64_t values[]={-1,0,9,99,999,1000,9999,10000,999949,999950,999999,
        9999499,9999500,10000000,14200000,54600000,99949999,100000000,999949999,
        999950000,1000000000,999994999999LL,1000000000000LL,9007199254740991LL};
    unsigned cases=0;
    for(unsigned slot=0;slot<sizeof(widths)/sizeof(widths[0]);++slot)
        for(unsigned a=0;a<sizeof(values)/sizeof(values[0]);++a)
            for(unsigned b=0;b<sizeof(values)/sizeof(values[0]);++b) {
                int64_t totals[]={values[a],values[b]};bool available[]={values[a]>=0,values[b]>=0};
                unsigned heights[]={digits96.height,slot==1?digits48.height:digits64.height};
                token_pair_layout(totals,available,widths[slot],heights,result);
                assert(result[0].font->height>result[1].font->height);
                for(unsigned i=0;i<2;++i){
                    assert(result[i].width<=widths[slot][i]);
                    assert(result[i].font->height<=heights[i]);
                    assert(result[i].unit_font->height<result[i].font->height);
                    if(!available[i])assert(!strcmp(result[i].number,"--") && !result[i].unit[0]);
                }
                ++cases;
            }
    // The shortest portrait cards cap the weekly row at 40px.
    const int narrow_portrait[]={249,278};
    const unsigned small_heights[]={digits96.height,digits40.height};
    token_pair_layout(photo_totals,known,narrow_portrait,small_heights,result);
    assert(result[0].font->height>result[1].font->height && result[1].font==&digits40);
    unsigned triples=0;
    const int uniform_widths[]={87,150,158,194,212,218,322,396};
    for(unsigned g=0;g<sizeof(uniform_widths)/sizeof(uniform_widths[0]);++g)
    for(unsigned a=0;a<sizeof(values)/sizeof(values[0]);++a)
        for(unsigned b=0;b<sizeof(values)/sizeof(values[0]);++b) {
            int64_t totals[]={values[a],values[b],values[sizeof(values)/sizeof(values[0])-1-b]};
            bool available[]={totals[0]>=0,totals[1]>=0,totals[2]>=0};
            int rooms[]={uniform_widths[g],uniform_widths[g],uniform_widths[g]};
            unsigned size=g==0?font_bold24.height:digits64.height;unsigned heights[]={size,size,size};
            token_number_layout_t triple[3];token_triple_layout(totals,available,rooms,heights,triple);
            assert(triple[0].font==triple[1].font && triple[1].font==triple[2].font);
            assert(triple[0].unit_font==triple[1].unit_font && triple[1].unit_font==triple[2].unit_font);
            token_number_layout_t zero[3];int64_t zeros[]={0,0,0};bool all[]={true,true,true};
            token_triple_layout(zeros,all,rooms,heights,zero);
            assert(zero[0].font==triple[0].font);
            for(unsigned i=0;i<3;++i)assert(triple[i].width<=rooms[i]);
            ++triples;
        }
    printf("Token typography: %u pairs, %u uniform triples; stable zero/unknown/unit transitions and equal fonts: OK\n",cases,triples);
}
