"""Generate a compact common-Chinese filename font from the existing system font."""
from pathlib import Path
import math
from PIL import Image,ImageDraw,ImageFont
root=Path(__file__).resolve().parents[1]
chars=set(chr(i) for i in range(32,127))
for lead in range(0xb0,0xd8):
    for trail in range(0xa1,0xff):
        try:chars.add(bytes([lead,trail]).decode('gb2312'))
        except UnicodeDecodeError:pass
chars.update('鄧麗臺灣國樂愛夢風雲華張劉陳楊黃葉許吳龍謝蘇鄭陽聽見說話來過這樣與誰為個時長讓會動歡總還難聲歲當記憶無盡頭離開轉錄檔體後裡遠輕約雙對淚單戀綠藍紅飛銀陰燈馬傳點親壞續寫溫柔')
font=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial Unicode.ttf',20)
ascent,descent=font.getmetrics();height=ascent+descent;data=bytearray();glyphs=[]
for char in sorted(chars):
    width=max(1,math.ceil(font.getlength(char)));im=Image.new('1',(width,height))
    ImageDraw.Draw(im).text((0,ascent),char,font=font,fill=1,anchor='ls')
    glyphs.append((ord(char),len(data),width));data.extend(im.tobytes())
out=['// Generated common-Chinese filename glyph bitmaps. No font source file embedded.','#include "canvas.h"','static const uint8_t pixels[] = {']
out.extend(' '+','.join(f'0x{x:02x}' for x in data[i:i+24])+',' for i in range(0,len(data),24));out+=['};','static const glyph_t glyphs[] = {']
out.extend(f' {{{cp},{offset},{width}}},' for cp,offset,width in glyphs)
out+=['};',f'const bitmap_font_t font_music20={{glyphs,pixels,{len(glyphs)},{height},{ascent}}};']
(root/'main/music_fonts.c').write_text('\n'.join(out)+'\n');print(f'Music font: {len(glyphs)} glyphs, {len(data)} bitmap bytes')
