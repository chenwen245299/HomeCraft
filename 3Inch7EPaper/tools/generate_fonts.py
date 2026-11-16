#!/usr/bin/env python3
"""Generate the fixed UI glyph subset; Pillow is needed only to regenerate it."""
import argparse
import math
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--font", default="/System/Library/Fonts/Supplemental/Arial Unicode.ttf")
args = parser.parse_args()
texts = "".join(p.read_text() for p in (ROOT / "main").glob("*.c") if p.name != "fonts.c")
chars = sorted(set(chr(i) for i in range(32, 127)) | set(ch for ch in texts if ord(ch) > 127))
out = ['// Generated UI glyph bitmaps, not a copy of the source font file.', '#include "canvas.h"', '']
bold_path = "/System/Library/Fonts/Supplemental/Arial Bold.ttf"
specs = []
for size in (12, 16, 20, 24, 28, 32, 40, 48, 56, 64, 80, 96):
    numeric = size >= 40
    specs.append((f'digits{size}' if numeric else f'font{size}', size,
                  bold_path if numeric else args.font,
                  list("0123456789.,%+-/KMGBTP ") if numeric else chars))
# Provider names use actual bold glyphs at every responsive fallback size.
specs.extend((f'font_bold{size}', size, bold_path, [chr(i) for i in range(32, 127)] + ['°'])
             for size in (12, 16, 20, 24, 28, 32))
specs.append(('font_shutdown64', 64, args.font, list('已关机')))
for name, size, font_path, glyph_chars in specs:
    font = ImageFont.truetype(font_path, size)
    ascent, descent = font.getmetrics()
    height = ascent + descent
    data = bytearray()
    glyphs = []
    for char in glyph_chars:
        width = max(1, math.ceil(font.getlength(char)))
        image = Image.new("1", (width, height))
        ImageDraw.Draw(image).text((0, ascent), char, font=font, fill=1, anchor="ls", stroke_width=0)
        glyphs.append((ord(char), len(data), width))
        data.extend(image.tobytes())
    assert len(data) < 2**32
    out.append(f'static const uint8_t pixels_{name}[] = {{')
    for pos in range(0, len(data), 24):
        out.append('    ' + ','.join(f'0x{x:02x}' for x in data[pos:pos+24]) + ',')
    out.append('};')
    out.append(f'static const glyph_t glyphs_{name}[] = {{')
    out.extend(f'    {{{code}, {offset}, {width}}},' for code, offset, width in glyphs)
    out.append('};')
    out.append(f'const bitmap_font_t {name} = {{glyphs_{name}, pixels_{name}, {len(glyphs)}, {height}, {ascent}}};\n')
(ROOT / "main/fonts.c").write_text('\n'.join(out) + '\n')
print(f'Generated {len(chars)} UI glyphs at 12–32 px; bold provider names and reading units including ° at 12–32 px; numeric glyphs at 40–96 px; shutdown title at 64 px.')
