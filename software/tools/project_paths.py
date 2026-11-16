"""Resolve the selected board profile without loading BLE or account libraries."""
import json
import os
from pathlib import Path

SOFTWARE_ROOT = Path(__file__).resolve().parents[1]
def data_root():
    override = os.environ.get('EPAPER_PROJECT')
    workspace = Path(override) if override else SOFTWARE_ROOT.parent
    project = workspace / 'software/project.json'
    if override and not project.is_file():
        return workspace
    selection = json.loads(project.read_text())
    if set(selection) != {'schema_version', 'hardware'}:
        raise ValueError('硬件目录设置包含未知字段')
    hardware = Path(selection['hardware'])
    if selection.get('schema_version') != 1 or hardware.is_absolute() or not hardware.parts or any(p in {'.', '..'} for p in hardware.parts):
        raise ValueError('硬件目录设置无效')
    board = (workspace / hardware).resolve()
    if not board.is_relative_to(workspace.resolve()) or not (board/'board.json').is_file():
        raise ValueError('硬件目录缺少 board.json')
    return board / '.local'
ROOT = data_root()
