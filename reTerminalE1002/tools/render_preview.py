#!/usr/bin/env python3
"""Render the actual firmware C renderer with synthetic fixtures; never contacts a device."""
from pathlib import Path
import json,subprocess
from PIL import Image
ROOT=Path(__file__).resolve().parents[1]
out=ROOT/'artifacts/ui-preview';out.mkdir(parents=True,exist_ok=True)
binary=ROOT/'build/host/render_dashboard';binary.parent.mkdir(parents=True,exist_ok=True)
subprocess.run(['clang','-std=c11','-Wall','-Wextra','-Werror=misleading-indentation','-Wno-unused-parameter','-Wno-sign-compare','-D_POSIX_C_SOURCE=200809L','-O1','-g','-fsanitize=address,undefined','-I'+str(ROOT/'main'),'-I'+str(ROOT/'components/cjson'),*[str(ROOT/'main'/n) for n in ['color_canvas.c','dashboard.c','fonts.c','logos.c','icons.c','lunar.c','privacy.c','usage.c','protocol.c']],str(ROOT/'components/cjson/cJSON.c'),str(ROOT/'tests/render_dashboard.c'),'-lm','-o',str(binary)],check=True)
palette={0:(20,24,27),1:(255,255,250),2:(255,210,0),3:(205,30,45),5:(35,80,185),6:(20,140,65)}
for name in ['overview','critical','details','empty','long','pairing','expired','privacy']:
 raw=out/(name+'.bin');subprocess.run([str(binary),name,str(raw)],check=True)
 pixels=raw.read_bytes();rgb=bytearray()
 for b in pixels:
  rgb.extend(palette[b>>4]);rgb.extend(palette[b&15])
 Image.frombytes('RGB',(800,480),bytes(rgb)).save(out/(name+'.png'))
(out/'manifest.json').write_text(json.dumps({'synthetic':True,'sent_to_hardware':False,'renderer':'actual firmware C renderer','palette':'six inks; onscreen RGB is illustrative, physical colors require visual confirmation','sanitizers':['ASan','UBSan'],'scenarios':['overview','critical','details','empty','long','pairing','expired','privacy']},indent=2)+'\n')
print('Eight firmware-rendered synthetic previews passed ASan/UBSan and palette bounds.')
