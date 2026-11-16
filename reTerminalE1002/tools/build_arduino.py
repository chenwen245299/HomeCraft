#!/usr/bin/env python3
"""Compile a board environment check or pinned official examples. Never uploads."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
root=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('target',choices=['environment','display','sensor','battery','rtc'],default='environment',nargs='?')
a=p.parse_args()
profile=json.loads((root/'config/arduino-profile.json').read_text())
sketches={'environment':root/'examples/environment_check','display':root/'reference/Seeed_GxEPD2/examples/GxEPD2_reTerminal_E1002','sensor':root/'reference/OSHW-reTerminal-Series-E-D/examples/base/SHT4x_Sensor','battery':root/'reference/OSHW-reTerminal-Series-E-D/examples/base/Battery_Monitor','rtc':root/'reference/OSHW-reTerminal-Series-E-D/examples/base/RTC_PCF8563'}
command=[sys.executable,str(root/'tools/arduino.py'),'compile','--fqbn',profile['fqbn'],'--jobs','4','--build-path',str(root/'build/arduino'/a.target),'--library',str(root/'reference/Seeed_GxEPD2'),'--build-property','compiler.cpp.extra_flags=-I'+str(root/'config'),str(sketches[a.target])]
raise SystemExit(subprocess.call(command,cwd=root))
