#!/usr/bin/env python3
"""Passively read the E1002 UART log and print only allowlisted boot/app/frame lines.

Opens the port releasing RTS before DTR (unless --reset) so the board is not reset; other lines are
dropped unprinted and any standalone 8-digit number is masked. Run with the ESP-IDF Python (pyserial).
"""
import argparse,re,time
import serial
ALLOW=re.compile(r'^(ESP-ROM|Build:|rst:|boot:|load:|entry |[EW] \(\d+\) |I \(\d+\) (boot|cpu_start|esp_psram|octal_psram|spi_flash|heap_init|app_init|main_task|app|frame|display|ble)[:.]|.*(Guru Meditation|Backtrace|abort\(\)|assert failed|Brownout|Rebooting|stack overflow))')
def clean(raw):
 line=re.sub(r'\x1b\[[0-9;]*m','',raw.decode('utf-8','replace')).strip()
 return re.sub(r'(?<![\d(])\d{8}(?![\d)])','[8位数字]',line) if ALLOW.match(line) else None
def watch(port,seconds,until=None):
 """Print allowlisted lines for up to `seconds`; stop early once a line matches `until`."""
 end=time.time()+seconds;buf=b'';seen=[]
 while time.time()<end:
  buf+=port.read(512)
  *lines,buf=buf.split(b'\n')
  for raw in lines:
   line=clean(raw)
   if line:
    print(time.strftime('%H:%M:%S'),line,flush=True);seen.append(line)
    if until and re.search(until,line):return seen
 return seen
if __name__=='__main__':
 ap=argparse.ArgumentParser()
 ap.add_argument('--port',default='/dev/cu.usbserial-1420')
 ap.add_argument('--seconds',type=float,default=600)
 ap.add_argument('--until',help='stop after a line matching this regex')
 ap.add_argument('--reset',action='store_true',help='pulse EN via RTS (IO0 stays high: normal boot)')
 a=ap.parse_args()
 # macOS asserts DTR+RTS on open and pyserial then sets DTR before RTS; releasing DTR first pulls EN low
 # through the auto-reset transistors. Release RTS while DTR is still held, then DTR: no reset.
 port=serial.Serial();port.port=a.port;port.baudrate=115200;port.timeout=0.5;port.dtr=True;port.rts=False
 port.open();port.dtr=False
 try:
  if a.reset:port.rts=True;time.sleep(0.1);port.rts=False
  watch(port,a.seconds,a.until)
 finally:port.close()
