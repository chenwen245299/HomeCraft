#!/usr/bin/env python3
"""Rasterize official service identification marks for the monochrome display."""
from pathlib import Path
from PIL import Image

ROOT=Path(__file__).resolve().parents[1]
out=['// Generated from official marks; provenance: assets/brands/SOURCES.md.', '#include "brand_assets.h"']
for provider,name in enumerate(['openai.png','claude.png','deepseek.ico','argus.png']):
    original=Image.open(ROOT.parent/'software/assets/brands'/name).convert('RGBA')
    background=Image.new('RGBA',original.size,'white');background.alpha_composite(original)
    threshold=140 if provider==3 else 220
    mask=background.convert('L').point(lambda p:255 if p<threshold else 0)
    # Keep the symbol's own geometry; only remove empty surrounding whitespace.
    mask=mask.crop(mask.getbbox())
    for size in [44,56]:
        content=mask.copy();content.thumbnail((size-6,size-6),Image.Resampling.LANCZOS)
        image=Image.new('L',(size,size));image.paste(content,((size-content.width)//2,(size-content.height)//2))
        image=image.point(lambda p:255 if p>=128 else 0).convert('1')
        data=image.tobytes()
        out.append(f'static const uint8_t logo{provider}_{size}[]={{')
        for i in range(0,len(data),24):out.append('  '+','.join(f'0x{b:02x}'for b in data[i:i+24])+',')
        out.append('};')
out.append('''void brand_icon(canvas_t *c,unsigned provider,int x,int y,unsigned size)
{
    static const uint8_t *small[]={logo0_44,logo1_44,logo2_44,logo3_44};
    static const uint8_t *large[]={logo0_56,logo1_56,logo2_56,logo3_56};
    if(provider>=4) {
        canvas_round_box(c,x+4,y+4,size-8,size-8,8,2);
        canvas_text(c,x+size/2-8,y+size/2-10,"AI",&font16,1,true);
        return;
    }
    if(size!=56) size=44;
    const uint8_t *pixels=size==56?large[provider]:small[provider];
    unsigned stride=(size+7)/8;
    for(unsigned yy=0;yy<size;++yy) for(unsigned xx=0;xx<size;++xx)
        if(pixels[yy*stride+xx/8]&(0x80>>(xx%8))) canvas_pixel(c,x+xx,y+yy,true);
}
''')
(ROOT/'main/brand_assets.c').write_text('\n'.join(out))
print('Generated four service marks at 44 / 56 pixels.')
