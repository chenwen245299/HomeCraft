#!/usr/bin/env python3
"""Version, allowlisted release artifacts and GitHub update manifest; no user data."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import plistlib
import re
import subprocess
import sys
import tomllib
import zipfile

SOFTWARE = Path(__file__).resolve().parents[1]
VERSION = re.compile(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?')


def metadata(software=SOFTWARE):
    meta = json.loads((software / 'release.json').read_text())
    match = VERSION.fullmatch(meta['version'])
    if not match or any(p.isdigit() and len(p) > 1 and p[0] == '0' for p in (match[4] or '').split('.')):
        raise ValueError('版本必须是 MAJOR.MINOR.PATCH[-PRERELEASE]')
    if type(meta['build']) is not int or not 1 <= meta['build'] <= 2_147_483_647:
        raise ValueError('build 必须是有效正整数')
    for key in ['repository', 'source_repository']:
        if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]*/[A-Za-z0-9][A-Za-z0-9_.-]*', meta[key]):
            raise ValueError(key + ' 必须是 OWNER/REPO')
    if not re.fullmatch(r'[0-9]+\.[0-9]+', meta['minimum_macos']):
        raise ValueError('minimum_macos 无效')
    cargo = tomllib.loads((software / 'mac-app/Cargo.toml').read_text())
    lock = tomllib.loads((software / 'mac-app/Cargo.lock').read_text())
    locked = next(p['version'] for p in lock['package'] if p['name'] == 'epaper-ble')
    if cargo['package']['version'] != meta['version'] or locked != meta['version']:
        raise ValueError('release.json、Cargo.toml、Cargo.lock 的 App 版本必须一致')
    return meta


def stem(meta):
    return f"HomeCraft-{meta['version']}-build{meta['build']}-macos-arm64"


def check_tag(meta, tag, repository=None):
    if tag != 'v' + meta['version']:
        raise ValueError('标签必须与 release.json 完全一致：v' + meta['version'])
    if repository and repository != meta['repository']:
        raise ValueError('发布仓库与 release.json 的更新来源不一致')


def plist(meta):
    return {
        'CFBundleIdentifier': 'local.epaper.bluetooth', 'CFBundleName': 'HomeCraft',
        'CFBundleDisplayName': 'HomeCraft', 'CFBundleExecutable': 'HomeCraft',
        'CFBundlePackageType': 'APPL', 'CFBundleShortVersionString': meta['version'],
        'CFBundleVersion': str(meta['build']), 'CFBundleIconFile': 'HomeCraft',
        'LSMinimumSystemVersion': meta['minimum_macos'], 'LSUIElement': True,
        'NSHighResolutionCapable': True,
        'NSBluetoothAlwaysUsageDescription': '扫描并连接您的墨水屏，通过加密蓝牙同步用量、接收温湿度并控制显示页面。',
        'NSBluetoothPeripheralUsageDescription': '连接您的墨水屏、查看温湿度并切换屏幕内容。',
    }


def notes(meta, tag, workspace=SOFTWARE.parent):
    fallback = (workspace / 'software/release-notes' / f'{tag}.md').read_text().strip()
    kind = subprocess.run(['git', '-C', str(workspace), 'cat-file', '-t', f'refs/tags/{tag}'], capture_output=True, text=True)
    if kind.returncode == 0 and kind.stdout.strip() == 'tag':
        message = subprocess.check_output(['git', '-C', str(workspace), 'for-each-ref', '--format=%(contents)', f'refs/tags/{tag}'], text=True)
        text = message.split('-----BEGIN PGP SIGNATURE-----', 1)[0].strip() or fallback
    else:
        text = fallback
    if len(text.encode()) > 60_000:
        raise ValueError('更新说明过长')
    return text


def validate_archive(path, meta):
    allowed = {
        'HomeCraft.app/Contents/Info.plist', 'HomeCraft.app/Contents/MacOS/HomeCraft',
        'HomeCraft.app/Contents/Resources/HomeCraft.icns', 'HomeCraft.app/Contents/Resources/poems.json',
        'HomeCraft.app/Contents/_CodeSignature/CodeResources',
    }
    with zipfile.ZipFile(path) as archive:
        entries = archive.infolist()
        directories = {str(p) + '/' for name in allowed for p in Path(name).parents if str(p) != '.'}
        if len({i.filename for i in entries}) != len(entries) or any(i.filename not in allowed | directories for i in entries):
            raise ValueError('更新 ZIP 包含额外路径或重复项')
        files = [i for i in archive.infolist() if not i.is_dir()]
        if {i.filename for i in files} != allowed or len(files) != len(allowed):
            raise ValueError('更新 ZIP 必须仅包含 HomeCraft App 交付白名单')
        if any((i.external_attr >> 16) & 0o170000 == 0o120000 for i in entries):
            raise ValueError('更新 ZIP 不允许符号链接')
        info = plistlib.loads(archive.read('HomeCraft.app/Contents/Info.plist'))
        if any(info.get(k) != plist(meta)[k] for k in ['CFBundleIdentifier', 'CFBundleExecutable', 'CFBundleVersion', 'CFBundleShortVersionString']):
            raise ValueError('更新 ZIP 内的 App 版本/身份不匹配')


def write_manifest(meta, tag, repository, out, text):
    check_tag(meta, tag, repository)
    archive = out / (stem(meta) + '.zip')
    dmg = out / (stem(meta) + '.dmg')
    if not archive.is_file() or not dmg.is_file():
        raise ValueError('缺少本版本的 ZIP 或 DMG')
    validate_archive(archive, meta)
    sums = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in [archive, dmg]}
    base = f'https://github.com/{repository}/releases'
    manifest = {
        'schema': 1, 'version': meta['version'], 'build': meta['build'], 'tag': tag,
        'minimum_macos': meta['minimum_macos'], 'notes': text,
        'pub_date': datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),
        'release_url': f'{base}/tag/{tag}',
        'platforms': {'macos-arm64': {'url': f'{base}/download/{tag}/{archive.name}',
                                     'sha256': sums[archive.name], 'size': archive.stat().st_size}},
    }
    (out / 'latest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
    (out / 'SHA256SUMS.txt').write_text(''.join(f'{digest}  {name}\n' for name, digest in sorted(sums.items())))
    (out / 'notes.md').write_text(text + '\n')
    body = text + f'\n\n## 下载\n\nApple 芯片 Mac 请下载 `{dmg.name}`，打开后将 HomeCraft 拖入“应用程序”。\n\n'
    body += '`*.zip` 和 `latest.json` 供 App 内更新使用，`SHA256SUMS.txt` 用于核对下载文件。\n'
    body += '\n更新仅替换 App，不包含用户数据、配对秘密或硬件恢复备份，也不会自动刷写固件。\n'
    (out / 'body.md').write_text(body)
    return manifest


def publish(meta, tag, repository, out):
    check_tag(meta, tag, repository)
    # gh's --repo belongs to individual subcommands, not the top-level command.
    def command(*args, **kwargs):
        return subprocess.run(['gh', *args, '--repo', repository], check=True, **kwargs)
    visibility = subprocess.check_output(['gh', 'api', f'repos/{repository}', '--jq', '.visibility'], text=True).strip()
    if visibility != 'public':
        raise ValueError('更新发布库必须公开，App 不会携带 GitHub 私有访问令牌')
    existing = subprocess.run(['gh', 'release', 'view', tag, '--repo', repository, '--json', 'isDraft'], capture_output=True, text=True)
    if existing.returncode == 0 and not json.loads(existing.stdout)['isDraft']:
        raise ValueError('已发布的 Release 不覆盖；请使用新版本标签')
    if existing.returncode:
        # A separate distribution repo tags only its own public README commit, never private source.
        verify = ['--verify-tag'] if meta['source_repository'] == repository else []
        command('release', 'create', tag, '--draft', *verify, '--title', f"HomeCraft {meta['version']}", '--notes-file', str(out / 'body.md'))
    command('release', 'upload', tag, '--clobber', *(str(out / name) for name in [stem(meta) + '.dmg', stem(meta) + '.zip', 'SHA256SUMS.txt', 'latest.json']))
    prerelease = '-' in meta['version']
    latest = not prerelease
    previous = subprocess.run(['gh', 'api', f'repos/{repository}/releases/latest'], capture_output=True, text=True)
    if previous.returncode == 0:
        old = json.loads(previous.stdout)['tag_name'].removeprefix('v')
        if VERSION.fullmatch(old) and '-' not in old:
            latest = latest and tuple(map(int, meta['version'].split('.'))) >= tuple(map(int, old.split('.')))
    elif 'HTTP 404' not in previous.stderr:
        raise ValueError('无法确认当前 latest Release；保留草稿，稍后重跑')
    command('release', 'edit', tag, '--draft=false', f'--prerelease={str(prerelease).lower()}', f'--latest={str(latest).lower()}', '--notes-file', str(out / 'body.md'))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    sub.add_parser('check')
    value = sub.add_parser('value'); value.add_argument('key', choices=['version', 'build', 'minimum_macos', 'repository', 'stem'])
    info = sub.add_parser('plist'); info.add_argument('path', type=Path)
    for name in ['prepare', 'publish']:
        item = sub.add_parser(name)
        item.add_argument('--tag', required=True); item.add_argument('--repository', required=True)
        item.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    try:
        meta = metadata()
        if args.command == 'value': print(stem(meta) if args.key == 'stem' else meta[args.key])
        elif args.command == 'plist': args.path.write_bytes(plistlib.dumps(plist(meta)))
        elif args.command == 'prepare': write_manifest(meta, args.tag, args.repository, args.out, notes(meta, args.tag))
        elif args.command == 'publish': publish(meta, args.tag, args.repository, args.out)
        else: print('Release version, build and Cargo lock match.')
    except (OSError, ValueError, KeyError, StopIteration, subprocess.CalledProcessError, zipfile.BadZipFile) as error:
        print(f'Release: {error}', file=sys.stderr); return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
