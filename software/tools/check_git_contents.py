#!/usr/bin/env python3
"""Read-only preflight for this repository; never prints matched credential values."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess

MAX_FILE = 20 * 1024 * 1024  # Project policy, not a GitHub platform limit.
LOCAL_DIRS = {
    '.local', 'backups', '.cache', '.device-auth', 'build', 'target', 'dist',
    'managed_components', 'node_modules', '.venv', 'venv', '.ble-venv', '.conda',
    '__pycache__', '.arduino', '.vscode', '.idea',
    '.aws', '.ssh',
}
LOCAL_FILES = {
    '.ble-pairing.json', '.ble-settings.json', '.ble-device.json', '.home-ui.json',
    '.device-credentials.json', '.device-settings.json', 'sdkconfig', 'sdkconfig.old',
    '.netrc', 'auth.json', 'credentials.json',
    'id_rsa', 'id_ed25519',
}
OUTPUT_SUFFIXES = {
    '.bin', '.elf', '.hex', '.uf2', '.dmg', '.pkg', '.o', '.a', '.map', '.log',
    '.pyc', '.pyo', '.key', '.p12', '.pfx',
    '.pem', '.ppk', '.keystore',
}
SECRET_PATTERNS = [
    ('private key', re.compile(rb'-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY-----')),
    ('GitHub token', re.compile(rb'(?<![A-Za-z0-9_])(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,})')),
    ('API key', re.compile(rb'(?<![A-Za-z0-9_-])sk-(?:proj-|ant-)?[A-Za-z0-9_-]{25,}')),
    ('AWS access key', re.compile(rb'(?<![A-Z0-9])(?:AKIA|ASIA)[A-Z0-9]{16}(?![A-Z0-9])')),
]


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args], stderr=subprocess.PIPE)


def paths(data):
    return {os.fsdecode(p) for p in data.split(b'\0') if p}


def path_issue(name):
    p = Path(name)
    if any(ord(c) < 32 for c in name):
        return '文件名包含控制字符'
    if any(part in LOCAL_DIRS or part.endswith(('.app', '.dSYM'))
           or part.startswith(('build-', 'cmake-build-')) for part in p.parts[:-1]):
        return '本机资料、构建输出或下载依赖'
    if 'artifacts' in p.parts[:-1] and not (p.parent.name == 'artifacts' and p.name == 'README.md'):
        return '本机验证记录，仅允许目录 README'
    if name.startswith('reTerminalE1002/reference/') and name not in {
        'reTerminalE1002/reference/README.md', 'reTerminalE1002/reference/sources.lock.json',
    }:
        return '可重新下载的参考资料'
    if name.startswith('reTerminalE1002/toolchains/'):
        return '本机工具链'
    if p.name in LOCAL_FILES or p.name.startswith((
        '.cloud-', '.harness-', '.claude-statusline', '.data-location', '.homecraft-',
        'id_rsa.', 'id_ed25519.', 'gitleaks-report', '.gitleaks-report',
    )) or p.name.startswith('.ble-') and p.suffix == '.lock':
        return '私有设置、配对或运行缓存'
    if (p.name == '.env' or p.name.startswith('.env.')) and p.name not in {'.env.example', '.env.sample'}:
        return '本机环境变量文件'
    if p.suffix.lower() in OUTPUT_SUFFIXES or p.name.startswith('sdkconfig.old.'):
        return '编译/运行产物或私钥文件'
    return None


def inspect_content(root, name, data, symlink=False):
    if symlink:
        target = Path(os.fsdecode(data))
        if target.is_absolute() or not (root / name).parent.joinpath(target).resolve().is_relative_to(root):
            return [(name, 0, '符号链接指向仓库外部或使用本机绝对路径')]
        return []
    if b'\0' in data[:8192]:
        return []
    issues = []
    for label, pattern in SECRET_PATTERNS:
        for match in pattern.finditer(data):
            issues.append((name, data.count(b'\n', 0, match.start()) + 1, f'疑似 {label}；请检查，值不会输出'))
    return issues


def check(root, staged=False):
    root = root.resolve()
    index = {}
    for entry in git(root, 'ls-files', '--stage', '-z').split(b'\0'):
        if entry:
            metadata, name = entry.split(b'\t', 1)
            mode, oid, stage = metadata.split()
            if stage == b'0':
                index[os.fsdecode(name)] = (mode, oid.decode('ascii'))
    staged_paths = paths(git(root, 'diff', '--cached', '--name-only', '--diff-filter=ACMR', '-z'))
    candidates = staged_paths if staged else set(index) | paths(git(root, 'ls-files', '--others', '--exclude-standard', '-z'))
    forced = paths(git(root, 'ls-files', '--cached', '--ignored', '--exclude-standard', '-z'))
    issues = [(name, 0, '已由 Git 跟踪但被忽略规则排除，需从索引移除并保留本机文件') for name in forced if name in candidates]
    count = total = 0
    for name in sorted(candidates):
        reason = path_issue(name)
        if reason:
            issues.append((name, 0, reason))
            continue
        payloads = []
        file = root / name
        if not staged and (file.exists() or file.is_symlink()):
            resolved = file.resolve()
            if not resolved.is_relative_to(root):
                issues.append((name, 0, '文件或父目录的符号链接指向仓库外部'))
                continue
            if path_issue(resolved.relative_to(root).as_posix()):
                issues.append((name, 0, '文件或父目录的符号链接指向禁止上传的本机路径'))
                continue
            size = file.lstat().st_size
            if size > MAX_FILE:
                issues.append((name, 0, '单文件超过项目的 20 MiB 检查阈值'))
                continue
            payloads.append((os.fsencode(os.readlink(file)) if file.is_symlink() else file.read_bytes(), file.is_symlink()))
        # Also inspect staged bytes: a clean working file must not hide an older staged secret.
        if name in staged_paths and name in index:
            mode, oid = index[name]
            size = int(git(root, 'cat-file', '-s', oid))
            if size > MAX_FILE:
                issues.append((name, 0, '暂存文件超过项目的 20 MiB 检查阈值'))
                continue
            payloads.append((git(root, 'cat-file', 'blob', oid), mode == b'120000'))
        if payloads:
            count += 1
            total += len(payloads[0][0])
        for data, symlink in payloads:
            issues.extend(inspect_content(root, name, data, symlink))
    return count, total, sorted(set(issues))


def main():
    parser = argparse.ArgumentParser(description='只读检查待上传文件、常见密钥格式和暂存内容；不修改文件或 Git 索引。')
    parser.add_argument('--staged', action='store_true', help='只检查本次已经暂存的新增/修改文件')
    args = parser.parse_args()
    try:
        count, size, issues = check(Path(__file__).resolve().parents[2], args.staged)
    except (OSError, subprocess.CalledProcessError) as error:
        print(f'检查无法完成：{type(error).__name__}')
        return 2
    print(f'检查 {count} 个文件，合计 {size / 1024 / 1024:.2f} MiB。')
    for name, line, reason in issues:
        print(f'{json.dumps(name, ensure_ascii=False)}{":" + str(line) if line else ""}：{reason}')
    print(f'发现 {len(issues)} 项需处理。' if issues else '通过：未发现被禁止的产物/私有路径、超大文件或常见密钥格式。')
    return 1 if issues else 0


if __name__ == '__main__':
    raise SystemExit(main())
