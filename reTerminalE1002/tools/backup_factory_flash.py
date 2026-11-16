#!/usr/bin/env python3
"""Read-only, segment-verified 32MiB factory flash backup for this E1002; never writes or erases flash.

Run with the ESP-IDF Python (esptool 5.x). Output stays in backups/ (700/600); contents are never printed.
"""
import argparse,hashlib,json,os,struct,sys,time
from pathlib import Path
import esptool
from esptool.cmds import detect_chip,run_stub
from esptool.util import FatalError
ROOT=Path(__file__).resolve().parents[1]
SIZE=32<<20;HALF=16<<20
TABLE=ROOT/'backups/factory-partition-table-20261026.bin'

def log(*a):print(time.strftime('%H:%M:%S'),*a,flush=True)

def session(port,baud):
 esp=detect_chip(port,115200,'default-reset',connect_attempts=4)
 if esp.CHIP_NAME!='ESP32-S3':raise SystemExit(f'Unexpected chip {esp.CHIP_NAME}; refusing.')
 esp=run_stub(esp)  # the stub attaches SPI flash; ROM reports ff/ffff before attach
 fid=esp.flash_id();maker=fid&0xff;device=((fid>>8)&0xff)<<8|(fid>>16)&0xff
 if maker!=0xef or device!=0x4019:
  close(esp);raise SystemExit(f'Unexpected flash {maker:02x}/{device:04x}; refusing.')
 esp.flash_set_parameters(SIZE)
 if baud!=115200:esp.change_baud(baud)
 return esp

def close(esp):
 try:esp._port.close()
 except Exception:pass

def read_segment(esp,offset,length,block,inflight):
 # Same stub READ_FLASH protocol as esptool, with smaller packets/in-flight window for the CH340 link.
 esp.check_command('read flash',esp.ESP_CMDS['READ_FLASH'],struct.pack('<IIII',offset,length,block,inflight))
 data=bytearray()
 while len(data)<length:
  esp._port.timeout=3
  p=esp.read()
  if len(data)+len(p)<length and len(p)!=block:raise FatalError(f'short packet {len(p):#x} at {offset+len(data):#x}')
  data+=p;esp.write(struct.pack('<I',len(data)))
 digest=esp.read()
 if len(data)!=length or len(digest)!=16 or hashlib.md5(data).digest()!=digest:raise FatalError(f'digest mismatch at {offset:#x}')
 return bytes(data)

def main():
 ap=argparse.ArgumentParser()
 ap.add_argument('--port',default='/dev/cu.usbserial-1420')
 ap.add_argument('--date',default=time.strftime('%Y%m%d'))
 ap.add_argument('--bauds',default='460800,230400,115200')
 ap.add_argument('--segment',type=lambda s:int(s,0),default=0x40000)
 ap.add_argument('--block',type=lambda s:int(s,0),default=0x400)
 ap.add_argument('--inflight',type=int,default=2)
 ap.add_argument('--retries',type=int,default=8)
 a=ap.parse_args()
 os.umask(0o077);(ROOT/'backups').mkdir(mode=0o700,exist_ok=True);os.chmod(ROOT/'backups',0o700)
 final=ROOT/f'backups/factory-full-{a.date}.bin';partial=final.with_suffix('.bin.partial');state_path=ROOT/f'backups/.factory-full-{a.date}.state.json'
 record=ROOT/f'artifacts/factory-backup-{a.date}.json'
 if final.exists():raise SystemExit(f'{final.name} already exists; not overwriting.')
 bauds=[int(b) for b in a.bauds.split(',')];level=0
 state=json.loads(state_path.read_text()) if state_path.exists() and partial.exists() else {'segment':a.segment,'done':{}}
 if state['segment']!=a.segment:raise SystemExit('Segment size differs from saved progress; remove partial state first.')
 if not partial.exists():
  with open(partial,'wb') as f:f.truncate(SIZE)
 esp=session(a.port,bauds[level]);t0=time.time()
 log(f'Connected {esp.get_chip_description()}, stub, {bauds[level]} baud, flash 32MiB')
 if esp.flash_md5sum(0x8000,0x1000)!=hashlib.md5(TABLE.read_bytes()).hexdigest():
  raise SystemExit('Partition table on device differs from the saved factory table; refusing.')
 low,high=esp.flash_md5sum(0,0x1000),esp.flash_md5sum(HALF,0x1000)
 half_low,half_high=esp.flash_md5sum(0,HALF),esp.flash_md5sum(HALF,HALF)
 if low==high or half_low==half_high:raise SystemExit('Upper 16MiB mirrors lower half (address wrap); refusing.')
 full_before=esp.flash_md5sum(0,SIZE)
 log('Partition table matches; upper half distinct from lower half; device full-flash MD5 recorded')
 segments=SIZE//a.segment;retries=0;bauds_used={bauds[level]}
 with open(partial,'r+b') as f:
  for i in range(segments):
   off=i*a.segment
   if str(i) in state['done']:
    f.seek(off)
    if hashlib.md5(f.read(a.segment)).hexdigest()==state['done'][str(i)]:continue
   for attempt in range(a.retries):
    try:
     expected=esp.flash_md5sum(off,a.segment)
     data=read_segment(esp,off,a.segment,a.block,a.inflight)
     if hashlib.md5(data).hexdigest()!=expected:raise FatalError(f'device MD5 mismatch at {off:#x}')
     f.seek(off);f.write(data);f.flush();os.fsync(f.fileno())
     state['done'][str(i)]=expected;state_path.write_text(json.dumps(state))
     break
    except (FatalError,OSError,StopIteration,RuntimeError) as e:
     retries+=1;log(f'segment {i} attempt {attempt+1} failed: {e}')
     close(esp);time.sleep(1.5)
     if attempt>=1 and level<len(bauds)-1:level+=1
     esp=session(a.port,bauds[level]);bauds_used.add(bauds[level])
   else:
    close(esp);raise SystemExit(f'Segment {i} could not be verified; partial kept, no full backup claimed.')
   if i%8==7 or i==segments-1:
    done=len(state['done']);rate=done*a.segment/max(1,time.time()-t0)/1024
    log(f'{done}/{segments} segments verified ({rate:.1f} KiB/s this run)')
 full_after=esp.flash_md5sum(0,SIZE)
 h_md5=hashlib.md5();h_sha=hashlib.sha256()
 with open(partial,'rb') as f:
  for chunk in iter(lambda:f.read(1<<20),b''):h_md5.update(chunk);h_sha.update(chunk)
 if not (h_md5.hexdigest()==full_before==full_after) or partial.stat().st_size!=SIZE:
  close(esp);raise SystemExit('Assembled file does not match device full-flash MD5; not saved as full backup.')
 os.replace(partial,final);os.chmod(final,0o600);state_path.unlink()
 log('Full backup verified against device MD5 before and after reading')
 esp.hard_reset();close(esp)
 record.write_text(json.dumps({
  'date':time.strftime('%Y-%m-%d'),'port':a.port,'usb_vid_pid':'1a86:7523','chip':'ESP32-S3','flash_id':'ef/4019',
  'esptool':esptool.__version__,'mode':'stub, read-only (READ_FLASH + SPI_FLASH_MD5)','bauds_used':sorted(bauds_used),
  'segment_bytes':a.segment,'packet_bytes':a.block,'max_inflight':a.inflight,'segments':segments,'retries':retries,
  'partition_table_matches_saved':True,'upper_half_distinct':True,
  'file':str(final.relative_to(ROOT)),'size':SIZE,'md5':h_md5.hexdigest(),'sha256':h_sha.hexdigest(),
  'device_md5_before_equals_after':True,'file_mode':'600','directory_mode':'700',
  'flashed':False,'erased':False,'nvs_decoded_or_printed':False,'reset':'RTS hard reset after read'},indent=2)+'\n')
 os.chmod(record,0o600)
 log(f'Record written to {record.relative_to(ROOT)}')

if __name__=='__main__':main()
