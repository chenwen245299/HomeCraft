#!/usr/bin/env python3
"""Run this board's isolated Arduino CLI; no global Arduino configuration."""
import json
from pathlib import Path
import subprocess
import sys
root=Path(__file__).resolve().parents[1]
settings=json.loads((root/'config/arduino-cli.template.json').read_text())
for key,value in settings['directories'].items():
    settings['directories'][key]=str(root/value)
config=root/'config/arduino-cli.yaml'
config.write_text(json.dumps(settings,indent=2)+'\n')
raise SystemExit(subprocess.call([str(root/'toolchains/arduino-cli'),'--config-file',str(config),*sys.argv[1:]],cwd=root))
