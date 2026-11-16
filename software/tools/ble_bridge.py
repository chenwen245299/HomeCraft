#!/usr/bin/env python3
"""Read local account usage and send credential-free snapshots using encrypted BLE."""
from __future__ import annotations
import argparse
import asyncio
import fcntl
import json
import math
import os
from pathlib import Path
import shutil
import signal
import sys
import time
import urllib.error
import urllib.request

from project_paths import ROOT, SOFTWARE_ROOT
VENDOR = SOFTWARE_ROOT / 'tools/vendor/esp_security2'
sys.path[:0] = [str(VENDOR), str(VENDOR/'proto')]
from security.security2 import Security2
from codex_bridge import CodexClient, BridgeError, normalize_limits, normalize_window, UsbLink, find_port

NOT_CONFIGURED, WAITING, OK, AUTH_ERROR, RATE_LIMITED, NETWORK_ERROR, BAD_RESPONSE, ACCESS_DENIED = range(8)
OTHER_CACHE={}

def save_private(path, value):
    fd = os.open(path, os.O_WRONLY|os.O_CREAT|os.O_TRUNC|getattr(os,'O_NOFOLLOW',0),0o600)
    os.fchmod(fd,0o600)
    with os.fdopen(fd,'w') as f: json.dump(value,f);f.write('\n')

def pair_usb(path, port):
    link=UsbLink(find_port(port));buffer=bytearray()
    try:
        link.drain();link.send(b'\nEPAPER_PAIR\n');deadline=time.monotonic()+20
        while time.monotonic()<deadline:
            buffer.extend(link.drain())
            while b'\n' in buffer:
                line,_,tail=buffer.partition(b'\n');buffer=bytearray(tail)
                if line.startswith(b'EPAPER_PAIR '):
                    config=json.loads(line[len(b'EPAPER_PAIR '):])
                    if not all(config.get(k) for k in ('name','username','password','service_uuid')):
                        raise BridgeError('Incomplete pairing response')
                    save_private(path,config)
                    print('Mac 已绑定 '+config['name']+'；配对文件权限为 600，不含账户令牌。',flush=True)
                    return
                if line.startswith(b'EPAPER_PAIR_DENIED'):
                    # USB becomes ready before the BLE service during a fresh boot.
                    time.sleep(1);link.send(b'EPAPER_PAIR\n')
            if len(buffer)>8192: buffer.clear()
            time.sleep(.05)
        raise BridgeError('配对窗口尚未打开；请在设备网络页双击 BOOT 后重试。')
    finally: link.close()

def packet(provider,status=OK):
    return {'v':1,'p':provider,'t':int(time.time()),'s':status,'w':[[-1,0,0],[-1,0,0],[-1,0,0]]}

def safe_number(value):
    return isinstance(value,(int,float)) and not isinstance(value,bool) and math.isfinite(value)

def put_window(result,window):
    w=normalize_window(window);mins=w['window_mins']
    index=2 if 27*1440<=mins<=32*1440 else (1 if 5*1440<=mins<=8*1440 else 0)
    result['w'][index]=[w['remaining_tenths'],mins,w['reset_at']]

def codex_packet(result):
    # Select only quota fields; credentials and account IDs never enter this packet.
    normalized=normalize_limits(result)
    p=packet(0);p['t']=normalized['fetched_at']
    for name in ('primary','secondary'):
        w=normalized[name];mins=w['window_mins']
        i=2 if 27*1440<=mins<=32*1440 else (1 if 5*1440<=mins<=8*1440 else 0)
        p['w'][i]=[w['remaining_tenths'],mins,w['reset_at']]
    return p

def http_json(url,headers):
    request=urllib.request.Request(url,headers={**headers,'Accept':'application/json'})
    # Fixed provider URLs only. Never follow redirects carrying credentials.
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self,*args,**kwargs): return None
    with urllib.request.build_opener(NoRedirect()).open(request,timeout=15) as response:
        body=response.read(32769)
        if len(body)>32768: raise ValueError('Oversized provider response')
        return json.loads(body)

def settings():
    path=ROOT/'.ble-settings.json'
    if not path.exists():return {}
    if path.stat().st_mode&0o077:raise BridgeError('.ble-settings.json 必须仅当前用户可读（chmod 600）。')
    return json.loads(path.read_text())

def provider_status(error):
    if isinstance(error,urllib.error.HTTPError):
        return {401:AUTH_ERROR,403:ACCESS_DENIED,429:RATE_LIMITED}.get(error.code,NETWORK_ERROR)
    return NETWORK_ERROR

def claude_packet(config):
    # Official Claude Code statusLine output only; never call private OAuth URLs.
    path=ROOT/'.claude-statusline.json'
    if not path.exists():return packet(1,NOT_CONFIGURED)
    try:
        if path.stat().st_mode&0o077:raise BridgeError('Claude usage cache must be private')
        value=json.loads(path.read_text()).get('packet')
        if not isinstance(value,dict) or value.get('p')!=1:return packet(1,BAD_RESPONSE)
        encode_packet(value)
        for w in value.get('w',[]):
            if w[2] and w[2]<=int(time.time()):w[:]=[-1,0,0]
        return value
    except Exception:return packet(1,BAD_RESPONSE)

def deepseek_packet(config):
    key=os.environ.get('DEEPSEEK_API_KEY') or config.get('deepseek_api_key')
    if not key:return packet(2,NOT_CONFIGURED)
    try:
        result=http_json('https://api.deepseek.com/user/balance',{'Authorization':'Bearer '+key})
        balances=result.get('balance_infos') or []
        chosen=next((b for b in balances if b.get('currency')=='CNY'),None) or next((b for b in balances if b.get('currency')=='USD'),None)
        if not chosen:return packet(2,BAD_RESPONSE)
        value=float(chosen['total_balance'])
        if not math.isfinite(value):return packet(2,BAD_RESPONSE)
        p=packet(2);p.update(b=value,c=chosen['currency']);return p
    except Exception as error:return packet(2,provider_status(error))

def collect_packets():
    config=settings();codex=shutil.which('codex');client=None
    try:
        if not codex:p=packet(0,NOT_CONFIGURED)
        else:
            client=CodexClient(codex);client.start();p=codex_packet(client.limits())
    except (BridgeError,OSError):p=packet(0,NETWORK_ERROR)
    finally:
        if client:client.close()
    others=[]
    for provider,reader in [(1,claude_packet),(2,deepseek_packet)]:
        previous=OTHER_CACHE.get(provider)
        if previous and time.monotonic()<previous[0]:value=previous[1]
        else:
            value=reader(config);cooldown=900 if value['s']==RATE_LIMITED else 300
            OTHER_CACHE[provider]=(time.monotonic()+cooldown,value)
        others.append(value)
    return [p,*others]

def encode_packet(value):
    # Whitelist again at the transport boundary to reject accidental secrets.
    allowed={'v','p','t','s','w','b','c','mu','ml','d','ld'}
    if set(value)-allowed:raise ValueError('Unsupported or sensitive packet fields')
    payload=json.dumps(value,separators=(',',':'),allow_nan=False).encode()
    if len(payload)>480:raise ValueError('BLE snapshot too large')
    return payload

class SecureLink:
    def __init__(self,client,config):self.client=client;self.config=config;self.security=None;self.endpoints={}
    async def discover_endpoints(self):
        service=self.client.services.get_service(self.config['service_uuid'])
        if not service:raise BridgeError('Data service missing')
        for characteristic in service.characteristics:
            for descriptor in characteristic.descriptors:
                if descriptor.uuid.startswith('00002901-'):
                    name=bytes(await self.client.read_gatt_descriptor(descriptor.handle)).decode()
                    self.endpoints[name]=characteristic.uuid
        for name in ('proto-ver','prov-session','usage-data'):
            if name not in self.endpoints:raise BridgeError('Missing BLE endpoint '+name)
    async def exchange(self,endpoint,payload):
        await self.client.write_gatt_char(endpoint,payload,response=True)
        return bytes(await self.client.read_gatt_char(endpoint))
    async def authenticate(self):
        await self.discover_endpoints()
        version=json.loads(await self.exchange(self.endpoints['proto-ver'],b'EPaper'))
        prov=version.get('prov') or {}
        if prov.get('sec_ver')!=2 or prov.get('sec_patch_ver')!=1:
            raise BridgeError('Require official Security 2 with nonce-counter patch; refusing downgrade')
        self.security=Security2(1,self.config['username'],self.config['password'],False)
        response=None
        for _ in range(3):
            request=self.security.security_session(response)
            if request is None:return
            response=(await self.exchange(self.endpoints['prov-session'],request.encode('latin-1'))).decode('latin-1')
        raise BridgeError('Security handshake did not complete')
    async def send(self,payload):
        encrypted=self.security.encrypt_data(payload)
        response=await self.exchange(self.endpoints['usage-data'],encrypted)
        result=json.loads(self.security.decrypt_data(response))
        if result.get('ok') is not True:raise BridgeError('Device rejected snapshot')
        return result

async def run(args):
    from bleak import BleakClient,BleakScanner
    path=Path(args.config)
    if path.stat().st_mode&0o077:raise BridgeError('配对文件必须仅当前用户可读（chmod 600）。')
    config=json.loads(path.read_text());backoff=3
    while True:
        try:
            print('正在连接 '+config['name']+'…',flush=True)
            device=await BleakScanner.find_device_by_filter(lambda d,a:a.local_name==config['name'] or d.name==config['name'],timeout=15,service_uuids=[config['service_uuid']])
            if not device:raise BridgeError('找不到墨水屏，请确认已通电且未在手机配网模式。')
            async with BleakClient(device,timeout=30) as client:
                link=SecureLink(client,config);await link.authenticate();backoff=3
                print('加密蓝牙已连接；只发送额度快照。',flush=True)
                next_poll=0.;last_poll=0.;sequence=None
                while client.is_connected:
                    now=time.monotonic()
                    if now>=next_poll:
                        packets=await asyncio.to_thread(collect_packets)
                        for p in packets:
                            reply=await link.send(encode_packet(p));sequence=reply['refresh']
                        print(time.strftime('%H:%M:%S')+' 已同步 '+', '.join(('Codex','Claude','DeepSeek')[p['p']]+':'+str(p['s']) for p in packets),flush=True)
                        if args.once:return
                        last_poll=time.monotonic();next_poll=last_poll+args.interval
                    await asyncio.sleep(10)
                    reply=await link.send(b'?')
                    if sequence is not None and reply['refresh']!=sequence:
                        next_poll=min(next_poll,max(time.monotonic(),last_poll+60));sequence=reply['refresh']
                raise BridgeError('蓝牙连接已断开')
        except asyncio.CancelledError:raise
        except Exception as error:
            print('蓝牙暂不可用：'+type(error).__name__+(' · '+str(error) if isinstance(error,BridgeError) else '')+'；稍后重试。',flush=True)
            if args.once:raise
            await asyncio.sleep(backoff);backoff=min(backoff*2,60)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config',default=str(ROOT/'.ble-pairing.json'))
    parser.add_argument('--pair-usb',action='store_true')
    parser.add_argument('--port')
    parser.add_argument('--once',action='store_true')
    parser.add_argument('--stop',action='store_true',help='停止本项目正在运行的蓝牙发送程序')
    parser.add_argument('--stdout',action='store_true',help='只读取额度快照，不连接蓝牙')
    parser.add_argument('--interval',type=int,default=300)
    args=parser.parse_args()
    if args.stop:
        path=ROOT/'.ble-sender.lock'
        if not path.exists():print('蓝牙发送程序未运行。');return
        with path.open('r+') as lock:
            try:fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
            except BlockingIOError:
                lock.seek(0);pid=int(lock.read().strip());os.kill(pid,signal.SIGINT)
                print('已请求停止蓝牙发送。');return
            print('蓝牙发送程序未运行。');return
    if args.interval<60:parser.error('最短查询间隔 60 秒')
    if args.pair_usb:pair_usb(args.config,args.port);return
    if args.stdout:
        for p in collect_packets():print(encode_packet(p).decode())
        return
    with (ROOT/'.ble-sender.lock').open('a+') as lock:
        os.chmod(ROOT/'.ble-sender.lock',0o600)
        try:fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
        except BlockingIOError:raise BridgeError('已有一个蓝牙发送程序在运行。')
        lock.seek(0);lock.truncate();lock.write(str(os.getpid()));lock.flush()
        try:asyncio.run(run(args))
        except KeyboardInterrupt:print('蓝牙发送已停止。')

if __name__=='__main__':
    try:main()
    except (BridgeError,OSError) as error:
        print(str(error),file=sys.stderr);raise SystemExit(1)
