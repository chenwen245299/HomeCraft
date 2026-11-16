#!/usr/bin/env python3
"""App-only upgrade for this board; run with the ESP-IDF Python after quitting HomeCraft.

Requires an explicit USB port, chip MAC and image. Backs up the current application and private
configuration, verifies the board's partition layout, and proves both configuration regions are
unchanged before reboot. Never writes bootloader, partition table, NVS or SD.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import struct
from datetime import datetime, timezone

import esptool
from esptool import cmds


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def partitions(prefix):
    entries = []
    for offset in range(0x8000, 0x8C00, 32):
        magic, kind, subtype, start, size, label, flags = struct.unpack_from('<HBBII16sI', prefix, offset)
        if magic in (0xEBEB, 0xFFFF):
            break
        require(magic == 0x50AA, 'Unrecognized partition table; no write allowed')
        entries.append((label.rstrip(b'\0').decode('ascii'), kind, subtype, start, size, flags))
    return entries


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', required=True)
    parser.add_argument('--expected-mac', required=True)
    parser.add_argument('--image', required=True, type=Path)
    args = parser.parse_args()
    board = Path(__file__).resolve().parents[1]
    image_path = args.image.resolve()
    require(image_path.is_relative_to(board), 'Image must belong to this board directory')
    image = image_path.read_bytes()
    require(48 <= len(image) <= 0x300000 and image[0] == 0xE9, 'Invalid application image or size')
    require(struct.unpack_from('<I', image, 32)[0] == 0xABCD5432, 'Missing application descriptor')
    version = image[48:80].split(b'\0')[0].decode('ascii')
    project = image[80:112].split(b'\0')[0].decode('ascii')
    require(project == 'epaper_codex_dashboard', 'Wrong firmware project')
    os.umask(0o077)
    backup_root = board / 'backups'
    backup_root.mkdir(mode=0o700, exist_ok=True)
    os.chmod(backup_root, 0o700)
    stamp = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')
    backup = backup_root / f'pre-app-{version}-{stamp}'
    backup.mkdir(mode=0o700)
    esp = esptool.detect_chip(args.port, baud=115200)
    try:
        mac = ':'.join(f'{byte:02x}' for byte in esp.read_mac())
        require(esp.CHIP_NAME == 'ESP32-S3' and mac == args.expected_mac.lower(), 'Chip identity mismatch; no write allowed')
        esp = esp.run_stub()
        cmds.attach_flash(esp)
        require((esp.flash_id() >> 16) & 0xFF == 24, 'Expected 16MB flash; no write allowed')

        def read(address, size):
            data = cmds.read_flash(esp, address, size, no_progress=True)
            require(len(data) == size, 'Incomplete flash read')
            require(hashlib.md5(data).hexdigest() == esp.flash_md5sum(address, size), 'Flash read digest mismatch')
            return data

        prefix = read(0, 0x10000)
        expected = [('nvs', 1, 2, 0x9000, 0x6000, 0), ('phy_init', 1, 1, 0xF000, 0x1000, 0),
                    ('factory', 0, 0, 0x10000, 0x300000, 0), ('dashboard', 1, 2, 0x310000, 0x50000, 0)]
        require(partitions(prefix) == expected, 'Unexpected partition layout; no write allowed')
        (backup / 'pre-app-region.bin').write_bytes(prefix)
        dashboard = read(0x310000, 0x50000)
        (backup / 'dashboard.bin').write_bytes(dashboard)
        previous_app = read(0x10000, 0x300000)
        (backup / 'application.bin').write_bytes(previous_app)
        print('Target identity, partition layout and private backups verified; writing app at 0x10000 only.', flush=True)
        cmds.write_flash(esp, [(0x10000, image)], no_progress=True)
        cmds.verify_flash(esp, [(0x10000, image)])
        require(read(0, 0x10000) == prefix, 'Bootloader/partition table/system NVS changed; not rebooting')
        require(read(0x310000, 0x50000) == dashboard, 'Dashboard NVS changed; not rebooting')
        record = {
            'date_utc': datetime.now(timezone.utc).isoformat(), 'version': version,
            'image': str(image_path.relative_to(board)), 'bytes': len(image),
            'sha256': hashlib.sha256(image).hexdigest(), 'app_only_offset': '0x10000',
            'flash_bytes_verified': True, 'pre_app_region_byte_identical': True,
            'dashboard_nvs_byte_identical': True, 'private_backup': str(backup.relative_to(board)),
            'sd_touched': False,
        }
        (backup / 'validation.json').write_text(json.dumps(record, indent=2) + '\n')
        (board / 'artifacts' / f'app-{version}-flash-validation.json').write_text(json.dumps(record, indent=2) + '\n')
        print('Application verified; bootloader, partition table and both NVS regions byte-identical. Rebooting.', flush=True)
        cmds.reset_chip(esp)
    finally:
        esp._port.close()


if __name__ == '__main__':
    main()
