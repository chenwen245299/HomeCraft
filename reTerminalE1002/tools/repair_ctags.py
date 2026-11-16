#!/usr/bin/env python3
"""Restore the pinned board-local native ctags after Arduino reinstalls its Intel package."""
from pathlib import Path
import hashlib
import json
import shutil
import subprocess
root=Path(__file__).resolve().parents[1]
lock=json.loads((root/'config/environment.lock.json').read_text())['ctags']
source=root/'reference/arduino-ctags'
assert subprocess.check_output(['git','-C',str(source),'rev-parse','HEAD'],text=True).strip()==lock['source_commit']
patch=root/lock['patch']
assert hashlib.sha256(patch.read_bytes()).hexdigest()==lock['patch_sha256']
native=root/'toolchains/ctags-arm64'
if not native.is_file():
    reverse=subprocess.run(['git','apply','--reverse','--check',str(patch)],cwd=source,capture_output=True)
    if reverse.returncode:subprocess.run(['git','apply',str(patch)],cwd=source,check=True)
    subprocess.run(['./configure'],cwd=source,check=True)
    subprocess.run(['make','-j4'],cwd=source,check=True)
    shutil.copy2(source/'ctags',native)
assert 'arm64' in subprocess.check_output(['file',str(native)],text=True)
# A rebuild may have a different build timestamp; the exact source and patch stay pinned.
target=root/'.arduino/data/packages/builtin/tools/ctags/5.8-arduino11/ctags'
assert target.parent.is_dir(),'Install this board Arduino packages first'
shutil.copy2(native,target)
subprocess.run([str(target),'--version'],check=True)
print('Restored board-local arm64 Arduino ctags; system tools unchanged.')
