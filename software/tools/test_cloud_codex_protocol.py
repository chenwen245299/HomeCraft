#!/usr/bin/env python3
"""Offline app-server contract checks. Synthetic accounts, no BLE or real login."""
import datetime
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

binary = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix='homecraft-account-contract-') as directory:
    root = Path(directory)
    stub = root / 'codex'
    stub.write_text('#!' + sys.executable + '\n' + r'''
import datetime,json,pathlib,sys
root=pathlib.Path(__file__).parent
if '--version' in sys.argv:
    print('codex-cli 0.141.0');raise SystemExit
reads=0
for line in sys.stdin:
    req=json.loads(line);method=req.get('method')
    if 'id' not in req:continue
    with (root/'calls').open('a') as f:f.write(method+'\n')
    if method=='initialize':result={'userAgent':'fixture'}
    elif method=='account/read':
        reads+=1
        email='other@example.test' if (root/'switch').exists() and reads>1 else 'sample@example.test'
        result={'account':{'type':'chatgpt','email':email,'planType':'plus'},'requiresOpenaiAuth':True}
        if (root/'no-email').exists():result['account']['email']=None
    elif method=='account/rateLimits/read':result={'rateLimits':{'primary':{'usedPercent':20,'windowDurationMins':300,'resetsAt':1900000000},'secondary':{'usedPercent':10,'windowDurationMins':10080,'resetsAt':1900000000}}}
    elif method=='account/usage/read':
        today=datetime.datetime.now(datetime.timezone(datetime.timedelta(hours=8))).date().isoformat()
        result={'summary':{'lifetimeTokens':123456},'dailyUsageBuckets':[{'startDate':today,'tokens':1234}]}
    else:result={}
    print(json.dumps({'id':req['id'],'result':result}),flush=True)
''')
    stub.chmod(0o700)
    config=root/'.home-ui.json'
    config.write_text(json.dumps({'codex_cli':str(stub)}));config.chmod(0o600)
    env={**os.environ,'EPAPER_PROJECT':str(root),'EPAPER_CODEX':str(stub)}
    expected=['initialize','account/read','account/rateLimits/read','account/usage/read','account/read']
    def run():
        (root/'calls').unlink(missing_ok=True)
        output=subprocess.check_output([str(binary),'--refresh-codex'],env=env,text=True,timeout=30)
        assert 'sample@example.test' not in output and 'other@example.test' not in output
        assert (root/'calls').read_text().splitlines()==expected
        return json.loads(output)
    first=run();assert first['packet']['s']==2
    assert first['tokens']['account']['verified'] and first['tokens']['account']['label']=='sam***'
    assert first['packet']['d'][1]==1234
    today=datetime.datetime.now(datetime.timezone(datetime.timedelta(hours=8))).date()
    monday=today-datetime.timedelta(days=today.weekday())
    assert first['packet']['wt']==[int(monday.strftime('%Y%m%d')),1234]
    assert first['packet']['mt']==[int(today.replace(day=1).strftime('%Y%m%d')),1234]
    assert first['packet']['yd']==[int((today-datetime.timedelta(days=1)).strftime('%Y%m%d')),-1,-1]
    (root/'switch').touch();changed=run();assert changed['packet']['s']!=2
    assert changed['packet'].get('d') is None
    (root/'switch').unlink();(root/'no-email').touch()
    fallback=run();assert fallback['packet']['s']==2 and not fallback['tokens']['account']['verified']
    assert fallback['tokens']['account']['label']==fallback['tokens']['account']['device']['name']
print(json.dumps({'official_methods_only':True,'masked_identity':True,'switch_during_read_rejected':True,'null_email_device_fallback':True,'weekly_from_official_daily_buckets':True,'hardware_access':False}))
