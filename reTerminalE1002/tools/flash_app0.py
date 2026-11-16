#!/usr/bin/env python3
"""Write only the HomeCraft app to app0 (0x90000) on this E1002, then boot it and show allowlisted log lines.

Bootloader, partition table, otadata and phy_init must equal the verified factory backup; NVS must be
byte-identical before and after writing (the app adds its own namespace at runtime, so it is compared
with itself, and its match with the factory copy is only reported). app0 must hold the factory app or a
homecraft_e1002 build. Any mismatch stops before reset. Run with the ESP-IDF Python (esptool 5.x).
"""
import argparse,hashlib,json,struct,sys,time,zlib
from pathlib import Path
from esptool.cmds import detect_chip,run_stub,write_flash
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'tools'))
from read_frame_log import watch
BACKUP=ROOT/'backups/factory-full-20261026.bin'
APP0,APP0_SIZE=0x90000,0xC00000
FACTORY={'bootloader':(0x0,0x8000),'partition table':(0x8000,0x1000),'otadata':(0x86000,0x2000),'phy_init':(0x88000,0x1000)}
NVS=(0x9000,0x7D000)
def log(*a):print(time.strftime('%H:%M:%S'),*a,flush=True)
def main():
 ap=argparse.ArgumentParser()
 ap.add_argument('--port',default='/dev/cu.usbserial-1420')
 ap.add_argument('--baud',type=int,default=230400)
 ap.add_argument('--image',default=str(ROOT/'build/homecraft/homecraft_e1002.bin'))
 ap.add_argument('--record',default=str(ROOT/'artifacts/firmware-0.3.0-build.json'))
 ap.add_argument('--watch',type=float,default=90)
 a=ap.parse_args()
 image=Path(a.image).read_bytes();backup=BACKUP.read_bytes()
 assert len(backup)==32<<20,'factory backup missing or wrong size'
 recorded=json.loads(Path(a.record).read_text())['sha256']
 if hashlib.sha256(image).hexdigest()!=recorded:raise SystemExit('Image differs from the recorded build; refusing.')
 if image[0]!=0xE9 or struct.unpack_from('<H',image,12)[0]!=9 or len(image)>APP0_SIZE:raise SystemExit('Not an ESP32-S3 app image that fits app0; refusing.')
 seq,_,_,crc=struct.unpack_from('<I20sII',backup,0x86000)
 if not (seq==1 and crc==zlib.crc32(struct.pack('<I',seq),0xFFFFFFFF)) or struct.unpack_from('<I',backup,0x87000)[0]!=0xFFFFFFFF:
  raise SystemExit('Backup otadata does not select app0; refusing.')
 esp=detect_chip(a.port,115200,'default-reset',connect_attempts=4)
 if esp.CHIP_NAME!='ESP32-S3':raise SystemExit('Unexpected chip; refusing.')
 esp=run_stub(esp);fid=esp.flash_id()
 if fid&0xff!=0xef or ((fid>>8)&0xff)<<8|(fid>>16)&0xff!=0x4019:raise SystemExit('Unexpected flash; refusing.')
 esp.flash_set_parameters(32<<20);esp.change_baud(a.baud)
 def factory(stage):
  for name,(off,size) in FACTORY.items():
   if esp.flash_md5sum(off,size)!=hashlib.md5(backup[off:off+size]).hexdigest():raise SystemExit(f'{stage}: {name} differs from the factory backup; stopping without reset.')
  return esp.flash_md5sum(*NVS)
 nvs_before=factory('before')
 if esp.flash_md5sum(APP0,APP0_SIZE)==hashlib.md5(backup[APP0:APP0+APP0_SIZE]).hexdigest():current='factory app'
 else:
  head=esp.read_flash(APP0,0x100)
  if head[0]!=0xE9 or struct.unpack_from('<I',head,32)[0]!=0xABCD5432 or head[32+48:32+48+32].split(b'\0')[0]!=b'homecraft_e1002':
   raise SystemExit('before: app0 holds neither the factory app nor a homecraft_e1002 build; refusing.')
  current='homecraft_e1002 '+head[32+16:32+48].split(b'\0')[0].decode()
 nvs_factory=nvs_before==hashlib.md5(backup[NVS[0]:NVS[0]+NVS[1]]).hexdigest()
 log(f'before: bootloader, partition table, otadata, phy_init equal the backup; app0 holds {current}; NVS equals factory copy: {nvs_factory}')
 write_flash(esp,[(APP0,image)],flash_freq='keep',flash_mode='keep',flash_size='keep',compress=True)
 if factory('after')!=nvs_before:raise SystemExit('after: NVS changed during flashing; stopping without reset.')
 if esp.flash_md5sum(APP0,len(image))!=hashlib.md5(image).hexdigest():raise SystemExit('after: app0 does not match the image; stopping without reset.')
 log('after: factory regions unchanged, NVS byte-identical to before, app0 equals the image')
 log('Resetting into the new app; showing allowlisted log lines')
 esp.hard_reset();port=esp._port;port.baudrate=115200;port.timeout=0.5;port.reset_input_buffer()
 try:watch(port,a.watch,r'frame: result=')
 finally:port.close()
if __name__=='__main__':main()
