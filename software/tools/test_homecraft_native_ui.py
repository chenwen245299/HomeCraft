#!/usr/bin/env python3
"""Isolated HomeCraft UI smoke: --ui-preview never starts the data/BLE backend."""
import json,os,plistlib
from pathlib import Path
import socket,subprocess,sys,tempfile,time
app=Path(sys.argv[1]).resolve()
info_path=app.parent.parent/'Info.plist'
info=plistlib.loads(info_path.read_bytes()) if info_path.is_file() else {}
output=Path(sys.argv[2]).resolve()
output.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='homecraft-ui-',dir='/private/tmp') as directory:
    root=Path(directory);control=root/'control';settings=root/'data';settings.mkdir(mode=0o700)
    process=subprocess.Popen([str(app),'--ui-preview','--ui-control',str(control)],env={**os.environ,'EPAPER_PROJECT':str(settings)},stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    def request(method,**params):
        with socket.socket(socket.AF_UNIX) as stream:
            stream.settimeout(15);stream.connect(str(control/'control.sock'))
            stream.sendall((json.dumps({'method':method,'params':params})+'\n').encode())
            answer=json.loads(stream.makefile('r').readline())
            assert answer.get('ok'),answer.get('error')
            return answer.get('result')
    def wait(predicate):
        end=time.monotonic()+12
        while time.monotonic()<end:
            try:
                value=request('state.inspect')
                if predicate(value):return value
            except (OSError,ValueError):pass
            time.sleep(.1)
        raise AssertionError('UI state did not settle')
    def activate(key):request('ui.activate',id=key);time.sleep(.15)
    def click(key):
        rect=wait(lambda s:key in s['targets'])['targets'][key]
        request('ui.pointer',x=rect[0]+rect[2]/2,y=rect[1]+rect[3]/2)
        time.sleep(.2)
    checks={}
    try:
        initial=wait(lambda s:s['preview'] and s['visible'])
        assert not initial['connected'] and not initial['connecting']
        assert initial['dock_visible'],'Main window must have a Dock entry'
        checks['open_window_has_dock_entry']=True
        checks['isolated_no_backend']=True
        request('ui.resize',width=850,height=620)
        wait(lambda s:s['size']==[850.,620.])
        home=wait(lambda s:'home.tokens' in s['targets'] and 'home.environment' in s['targets'])
        cards=home['targets'];left=cards['home.tokens'];right=cards['home.environment']
        assert left[1]==right[1] and left[3]==right[3]
        assert left[0]+left[2]<right[0]
        activate('home.tokens');wait(lambda s:s['page']=='usage')
        activate('nav.home');wait(lambda s:s['page']=='home')
        activate('home.environment');wait(lambda s:s['page']=='climate')
        checks['home_token_and_climate_summaries_and_navigation']=True
        for page in ['home','automation','discover','climate','displays','usage','room']:
            activate('nav.'+page);wait(lambda s:s['page']==page)
        checks['seven_native_pages']=True
        activate('add');wait(lambda s:s['sheet']=='Add' and 'connection.scan' in s['targets'])
        activate('sheet.close');wait(lambda s:s['sheet']=='None')
        checks['empty_scan_form_remains_usable']=True
        activate('settings');wait(lambda s:s['sheet']=='Settings')
        activate('refresh.settings');wait(lambda s:s['sheet']=='Refresh' and 'sheet.back' in s['targets'])
        activate('sheet.back');wait(lambda s:s['sheet']=='Settings' and 'codex.settings' in s['targets'])
        activate('codex.settings');wait(lambda s:s['sheet']=='Codex')
        activate('sheet.back');wait(lambda s:s['sheet']=='Settings')
        checks['native_settings_back']=True
        activate('update.settings');wait(lambda s:s['sheet']=='Update' and 'update.check' in s['targets'])
        assert initial['software_update']['current_version']==info['CFBundleShortVersionString']
        assert initial['software_update']['current_build']==int(info['CFBundleVersion'])
        click('update.auto_check');click('update.auto_download');click('update.save')
        wait(lambda s:s['software_update']['settings']=={'auto_check':False,'auto_download':False})
        click('update.check');wait(lambda s:s['sheet']=='Update' and not s['software_update']['busy'])
        activate('sheet.back');wait(lambda s:s['sheet']=='Settings')
        checks['update_settings_actions_and_saved_preferences_without_network']=True
        activate('sheet.close');wait(lambda s:s['sheet']=='None')
        for _ in range(3):
            request('ui.close');wait(lambda s:not s['visible'] and not s['dock_visible'])
            request('ui.show');wait(lambda s:s['visible'] and s['dock_visible'])
        checks['close_removes_dock_entry_and_reopen_restores_it']=True
        request('ui.minimize',minimized=True);wait(lambda s:s['minimized'] and s['dock_visible'])
        request('ui.minimize',minimized=False);wait(lambda s:not s['minimized'] and s['dock_visible'])
        checks['minimized_window_keeps_dock_entry']=True
        checks['close_hides_and_reopen']=True
        request('ui.resize',width=1100,height=780);wait(lambda s:s['size']==[1100.,780.])
        checks['resize_both_sizes']=True
        assert not any(settings.iterdir()),'preview must not create app credentials/settings'
        checks['preview_writes_no_user_data']=True
        report={'version':info.get('CFBundleShortVersionString','development'),'build':info.get('CFBundleVersion'),'checks':checks,'hardware_access':False,'real_accounts':False}
        (output/'native-ui-validation.json').write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps(report))
    finally:
        try:request('app.quit')
        except (OSError,ValueError,AssertionError):pass
        try:process.wait(timeout=5)
        except subprocess.TimeoutExpired:process.terminate();process.wait(timeout=5)
