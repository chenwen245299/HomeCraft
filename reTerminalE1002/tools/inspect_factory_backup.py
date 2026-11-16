#!/usr/bin/env python3
"""Report boot slot and app descriptors from the private factory backup; never contacts a device or decodes NVS."""
import struct,sys,zlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
image=Path(sys.argv[1]) if len(sys.argv)>1 else ROOT/'backups/factory-full-20261026.bin'
data=image.read_bytes();assert len(data)==32<<20,'expected a 32MiB backup'
SLOTS={'app0':0x90000,'app1':0xC90000};OTADATA=0x86000
def text(b):return b.split(b'\0',1)[0].decode('ascii','replace')
entries=[]
for i in range(2):
 seq,label,state,crc=struct.unpack_from('<I20sII',data,OTADATA+i*0x1000)
 valid=seq not in (0,0xFFFFFFFF) and crc==zlib.crc32(struct.pack('<I',seq),0xFFFFFFFF)
 entries.append((seq,state,valid))
 print(f'otadata[{i}] seq={seq:#x} state={state:#x} crc_valid={valid}')
live=[e for e in entries if e[2]]
boot='app0 (otadata empty: bootloader uses first OTA slot)' if not live else list(SLOTS)[(max(live)[0]-1)%2]
print('boot slot:',boot)
for name,off in SLOTS.items():
 magic,segments=data[off],data[off+1]
 if magic!=0xE9:print(f'{name}: no app image (first byte {magic:#04x})');continue
 desc=off+24+8  # image header + first segment header
 m,secure=struct.unpack_from('<II',data,desc)
 if m!=0xABCD5432:print(f'{name}: app image, {segments} segments, no esp_app_desc');continue
 version,project,time_,date,idf=(text(data[desc+16+o:desc+16+o+n]) for o,n in ((0,32),(32,32),(64,16),(80,16),(96,32)))
 print(f'{name}: project={project} version={version} idf={idf} built={date} {time_} segments={segments}')
