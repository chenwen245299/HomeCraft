#!/usr/bin/env python3
"""Keep the verified latest project installer; never touch firmware or private backups."""
import argparse
import json
from pathlib import Path
import re

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--keep", required=True, type=Path)
args = parser.parse_args()
release_dir = Path(__file__).resolve().parent.parent / "dist" / "releases"
keep = args.keep.absolute()
pattern = re.compile(r"(?:HomeCraft|EPaper-Home|EPaper-Bluetooth)-(\d+)\.(\d+)\.(\d+)(?:-[A-Za-z0-9.-]+)?-macos-arm64(?:-\d+)?\.(?:dmg|zip)")
match = pattern.fullmatch(keep.name)
if keep.parent != release_dir or keep.is_symlink() or not keep.is_file() or not match:
    parser.error("Only an existing project installer inside dist/releases can be kept.")
version = tuple(map(int, match.groups()))
old = []
for path in release_dir.iterdir():
    match = pattern.fullmatch(path.name)
    if not match or path.is_symlink() or not path.is_file() or path == keep:
        continue
    if tuple(map(int, match.groups())) > version:
        parser.error("A newer installer already exists; nothing was deleted.")
    old.append(path)
freed = sum(path.stat().st_size for path in old)
for path in old:
    path.unlink()
print(json.dumps({"kept": keep.name, "removed_count": len(old), "bytes_freed": freed}))
