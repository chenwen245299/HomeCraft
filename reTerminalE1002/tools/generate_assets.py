#!/usr/bin/env python3
"""Generate board-local glyph subsets, 24px service logos and six-ink icons (no anti-aliasing)."""
from pathlib import Path
import math
from PIL import Image,ImageDraw,ImageFont
ROOT=Path(__file__).resolve().parents[1]
text=''.join(p.read_text() for p in (ROOT/'main').glob('*.c') if p.name not in ('fonts.c','logos.c','icons.c'))
ascii_chars=set(chr(i) for i in range(32,127))
source=set(c for c in text if ord(c)>127)|set('°—●○…·')
def gb2312(rows):
 out=set()
 for hi in rows:
  for lo in range(0xA1,0xFF):
   try:out.add(bytes((hi,lo)).decode('gb2312'))
   except UnicodeDecodeError:pass
 return out
symbols=gb2312(range(0xA1,0xAA))  # GB2312 punctuation and full-width forms
level1=gb2312(range(0xB0,0xD8))
level2=gb2312(range(0xD8,0xF8))
# Todo text and poems come from HomeCraft, so body sizes carry all of GB2312; display sizes carry level 1.
full=ascii_chars|source|symbols|level1|level2
common=ascii_chars|source|symbols|level1
# Poems come from HomeCraft's bundled list and are drawn bold, so bold sizes carry their characters too.
poems=ROOT.parent/'software/assets/poems/poems.json'
import json
verse=set(''.join(p['title']+''.join(p['lines']) for p in json.loads(poems.read_text()))) if poems.is_file() else set()
bold=ascii_chars|source|verse
digits=set('0123456789.,%+-/:KMGBTP °?')
specs=[('text14',14,False,ascii_chars|source),('text16',16,False,full),('text18',18,False,full),('text20',20,False,full),('text24',24,False,common),
 ('bold16',16,True,bold),('bold18',18,True,bold),('bold20',20,True,bold),('bold24',24,True,bold),('bold32',32,True,bold),
 ('digits28',28,True,digits),('digits40',40,True,digits),('digits56',56,True,digits),('digits80',80,True,digits)]
out=['// Generated raster glyph subset; source fonts are not redistributed. Glyphs sorted by codepoint.','#include "color_canvas.h"']
for name,size,heavy,selection in specs:
 regular=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial Unicode.ttf',size)
 latin=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial Bold.ttf',size) if heavy else regular
 # Arial Unicode has no bold face: heavy sizes take CJK from Hiragino Sans GB W6.
 wide=ImageFont.truetype('/System/Library/Fonts/Hiragino Sans GB.ttc',size,index=2) if heavy else regular
 ascent,descent=regular.getmetrics();height=ascent+descent
 data=bytearray();glyphs=[]
 for char in sorted(selection):
  f=latin if ord(char)<256 else wide
  width=max(1,math.ceil(f.getlength(char)))
  image=Image.new('1',(width,height));ImageDraw.Draw(image).text((0,ascent),char,font=f,fill=1,anchor='ls')
  glyphs.append((ord(char),len(data),width));data.extend(image.tobytes())
 out.append(f'static const uint8_t pixels_{name}[]={{')
 out.extend(','.join(f'0x{x:02x}' for x in data[i:i+32])+',' for i in range(0,len(data),32));out.append('};')
 out.append(f'static const glyph_t glyphs_{name}[]={{')
 out.extend(f'{{{c},{off},{w}}},' for c,off,w in glyphs);out.append('};')
 out.append(f'const bitmap_font_t {name}={{glyphs_{name},pixels_{name},{len(glyphs)},{height},{ascent}}};')
(ROOT/'main/fonts.c').write_text('\n'.join(out)+'\n')
palette=[(0,0,0),(255,255,255),(255,210,0),(205,30,45),(255,255,255),(35,80,185),(20,140,65)]
INK={'k':palette[0],'w':palette[1],'y':palette[2],'r':palette[3],'b':palette[5],'g':palette[6]}
CLEAR=(255,0,255)
def ink_index(rgb):
 if rgb==CLEAR:return 4
 return min([0,1,2,3,5,6],key=lambda n:sum((rgb[k]-palette[n][k])**2 for k in range(3)))
def pack(im):
 w,h=im.size;row=[ink_index(im.getpixel((x,y))) for y in range(h) for x in range(w)]
 if len(row)%2:row.append(4)
 return [(row[i]<<4)|row[i+1] for i in range(0,len(row),2)]
LOGO=24
brand=ROOT.parent/'software/assets/brands'
out=['// Existing HomeCraft service logos mapped to the physical six-ink palette (24px).','#include "color_canvas.h"',f'static const uint8_t logos[5][{LOGO*LOGO//2}]={{']
for name in ['openai.png','claude.png','deepseek.ico','argus.png','deepseek.ico']:
 image=Image.open(brand/name).convert('RGBA');image.thumbnail((LOGO,LOGO),Image.Resampling.LANCZOS)
 bg=Image.new('RGBA',(LOGO,LOGO),'white');bg.alpha_composite(image,((LOGO-image.width)//2,(LOGO-image.height)//2))
 out.append('{'+','.join(str(x) for x in pack(bg.convert('RGB')))+'},')
out.extend(['};',f'void color_logo(color_canvas_t *c,int provider,int x,int y){{if(provider<0||provider>4)return;for(int yy=0;yy<{LOGO};++yy)for(int xx=0;xx<{LOGO};++xx){{uint8_t v=logos[provider][(yy*{LOGO}+xx)/2];color_pixel(c,x+xx,y+yy,(xx&1)?v&15:v>>4);}}}}'])
(ROOT/'main/logos.c').write_text('\n'.join(out)+'\n')
# Icons are drawn directly in ink colours so edges stay crisp on the six-colour panel.
def canvas(w,h):
 im=Image.new('RGB',(w,h),CLEAR);return im,ImageDraw.Draw(im)
# Weather icons are designed on a 44px grid and drawn directly at SIZE so lines stay solid.
SIZE=32;K=SIZE/44
def k(v):return round(v*K)
def kw(w):return max(1,round(w*K))
def cloud(d,dx=0,dy=0,fill='w'):
 parts=[(15,27,8),(24,21,11),(33,28,7)]
 for cx,cy,r in parts:d.ellipse((k(cx-r+dx),k(cy-r+dy),k(cx+r+dx),k(cy+r+dy)),fill=INK['k'])
 d.rectangle((k(15+dx),k(27+dy),k(33+dx),k(35+dy)),fill=INK['k'])
 for cx,cy,r in parts:d.ellipse((k(cx-r+dx)+2,k(cy-r+dy)+2,k(cx+r+dx)-2,k(cy+r+dy)-2),fill=INK[fill])
 d.rectangle((k(15+dx),k(27+dy),k(33+dx),k(35+dy)-2),fill=INK[fill])
def sun(d,cx,cy,r,rays=True):
 if rays:
  for i in range(8):
   a=i*math.pi/4;d.line((k(cx+math.cos(a)*(r+3)),k(cy+math.sin(a)*(r+3)),k(cx+math.cos(a)*(r+7)),k(cy+math.sin(a)*(r+7))),fill=INK['k'],width=kw(2))
 d.ellipse((k(cx-r),k(cy-r),k(cx+r),k(cy+r)),fill=INK['k']);d.ellipse((k(cx-r)+2,k(cy-r)+2,k(cx+r)-2,k(cy+r)-2),fill=INK['y'])
icons=[]
im,d=canvas(SIZE,SIZE);sun(d,22,22,10);icons.append(('SUN',im))
im,d=canvas(SIZE,SIZE);sun(d,29,15,7);cloud(d,-3,6);icons.append(('PARTLY',im))
im,d=canvas(SIZE,SIZE);cloud(d,-2,0,'w');icons.append(('CLOUD',im))
im,d=canvas(SIZE,SIZE)
for i,y in enumerate((14,21,28,35)):d.line((k(6+(i%2)*4),k(y),k(38-(i%2)*4),k(y)),fill=INK['k'],width=kw(3))
icons.append(('FOG',im))
im,d=canvas(SIZE,SIZE);cloud(d,-2,-6)
for x in (14,23,32):d.line((k(x),k(32),k(x-4),k(41)),fill=INK['b'],width=kw(3))
icons.append(('RAIN',im))
im,d=canvas(SIZE,SIZE);cloud(d,-2,-6)
for x,y in ((14,36),(23,40),(32,36)):
 d.line((k(x-4),k(y),k(x+4),k(y)),fill=INK['b'],width=kw(2));d.line((k(x),k(y-4),k(x),k(y+4)),fill=INK['b'],width=kw(2))
icons.append(('SNOW',im))
im,d=canvas(SIZE,SIZE);cloud(d,-2,-6)
bolt=[(24,28),(16,38),(22,38),(18,44),(30,33),(24,33),(28,28)]
d.polygon([(k(x),k(y)) for x,y in bolt],fill=INK['y'],outline=INK['k']);icons.append(('THUNDER',im))
def rune(d,ink):
 d.line((8,2,8,18),fill=INK[ink],width=2);d.line((8,2,13,6),fill=INK[ink],width=2);d.line((13,6,3,14),fill=INK[ink],width=2)
 d.line((8,18,13,14),fill=INK[ink],width=2);d.line((13,14,3,6),fill=INK[ink],width=2)
im,d=canvas(16,20);rune(d,'b');icons.append(('BT',im))
im,d=canvas(16,20);rune(d,'k');d.line((1,18,15,2),fill=INK['r'],width=2);icons.append(('BT_OFF',im))
im,d=canvas(12,22);d.rounded_rectangle((3,0,8,15),radius=2,fill=INK['k']);d.ellipse((0,12,11,21),fill=INK['k'])
d.rectangle((5,2,6,14),fill=INK['r']);d.ellipse((2,14,9,19),fill=INK['r']);icons.append(('THERMO',im))
im,d=canvas(14,20);d.polygon([(7,0),(1,11),(13,11)],fill=INK['b']);d.ellipse((1,6,13,19),fill=INK['b'])
d.ellipse((4,11,7,15),fill=INK['w']);icons.append(('DROP',im))
im,d=canvas(28,26);d.polygon([(14,1),(1,13),(27,13)],fill=INK['r']);d.rectangle((5,12,23,25),fill=INK['k']);d.rectangle((7,14,21,23),fill=INK['w'])
d.rectangle((12,17,16,25),fill=INK['k']);icons.append(('HOME',im))
im,d=canvas(13,13);d.ellipse((0,0,12,12),fill=INK['k']);d.ellipse((2,2,10,10),fill=INK['w'])
d.line((6,3,6,6),fill=INK['b'],width=2);d.line((6,6,9,8),fill=INK['b'],width=2);icons.append(('CLOCK',im))
out=['// Generated six-ink icons; 4 marks transparent pixels.','#include "color_canvas.h"']
for name,im in icons:out.append(f'static const uint8_t icon_{name.lower()}[]={{'+','.join(str(x) for x in pack(im))+'};')
out.append('static const struct {const uint8_t *data;uint8_t w,h;} icons[]={'+','.join(f'{{icon_{n.lower()},{im.size[0]},{im.size[1]}}}' for n,im in icons)+'};')
out.append('void color_icon(color_canvas_t *c,int icon,int x,int y){if(icon<0||icon>=(int)(sizeof(icons)/sizeof(icons[0])))return;for(int yy=0;yy<icons[icon].h;++yy)for(int xx=0;xx<icons[icon].w;++xx){int i=yy*icons[icon].w+xx;uint8_t v=icons[icon].data[i/2];v=(i&1)?v&15:v>>4;if(v!=4)color_pixel(c,x+xx,y+yy,v);}}')
(ROOT/'main/icons.c').write_text('\n'.join(out)+'\n')
enum=' '.join(f'ICON_{n}={i},' for i,(n,_) in enumerate(icons))
header=ROOT/'main/color_canvas.h';h=header.read_text()
start=h.index('// icons:begin');end=h.index('// icons:end')
header.write_text(h[:start]+'// icons:begin\nenum {'+enum+'ICON_COUNT};\n'+h[end:])
print(f'Generated fonts (body sizes carry full GB2312), 24px logos and {len(icons)} six-ink icons.')
