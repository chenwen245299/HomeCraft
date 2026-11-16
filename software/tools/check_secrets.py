#!/usr/bin/env python3
"""Local-only upload checks; report locations, never matched values or source lines."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

import check_git_contents as contents

ROOT = Path(__file__).resolve().parents[2]


def gitleaks_ready():
    if not shutil.which('gitleaks'):
        print('缺少 Gitleaks，已阻止提交/推送。macOS 请先运行 brew install gitleaks。')
        return False
    result = subprocess.run(['gitleaks', 'version'], capture_output=True, text=True)
    version = re.match(r'^v?(\d+)\.(\d+)\.(\d+)', result.stdout.strip())
    if result.returncode or not version or tuple(map(int, version.groups())) < (8, 30, 1):
        print('需要 Gitleaks 8.30.1 或更新版本；请升级后重试，检查不会跳过。')
        return False
    return True


def scan(root, arguments, report_dir):
    report = report_dir / 'findings.json'
    report.unlink(missing_ok=True)
    result = subprocess.run([
        'gitleaks', *arguments, '--config', str(root / '.gitleaks.toml'),
        '--redact=100', '--no-banner', '--no-color', '--log-level=error',
        '--report-format=json', '--report-path', str(report),
    ], cwd=root, capture_output=True)
    # Even error output can contain source text. Only expose selected report fields.
    findings = json.loads(report.read_text()) if report.exists() else []
    for finding in findings or []:
        name = finding.get('File', '?')
        prefix = str(report_dir / 'source') + os.sep
        if name.startswith(prefix):
            name = name[len(prefix):]
        print(json.dumps({
            'file': name, 'line': finding.get('StartLine'),
            'rule': finding.get('RuleID'), 'commit': finding.get('Commit', '')[:12],
        }, ensure_ascii=False))
    if result.returncode not in (0, 1) or (result.returncode and not findings):
        print('Gitleaks 未能完成检查；请确认安装和配置，提交/推送已阻止。')
        return 2
    return 1 if findings else 0


def check(root, staged=False):
    if not gitleaks_ready():
        return 2
    count, _, issues = contents.check(root, staged)
    if not staged:
        # Deleted private files must not remain uploadable through older commits.
        history_paths = contents.paths(contents.git(
            root, 'log', '--all', '--full-history', '--format=', '--name-only',
            '-z', '--no-renames', '--diff-filter=ACMR',
        )) if contents.git(root, 'rev-list', '--all').strip() else set()
        for name in history_paths:
            reason = contents.path_issue(name)
            if reason:
                issues.append((name, 0, '提交历史仍包含禁止上传的路径：' + reason))
    for name, line, reason in sorted(set(issues)):
        print(f'{json.dumps(name, ensure_ascii=False)}:{line}：{reason}')
    if issues:
        print('上传边界检查失败，提交/推送已阻止。')
        return 1
    print(f'上传边界检查通过（{count} 个文件）。', flush=True)
    with tempfile.TemporaryDirectory(prefix='homecraft-secret-scan-') as directory:
        private = Path(directory)  # TemporaryDirectory is owner-only (0700).
        statuses = [scan(root, ['git', '--pre-commit', '--staged', str(root)], private)]
        if not staged:
            source = private / 'source'
            source.mkdir()
            candidates = contents.paths(contents.git(
                root, 'ls-files', '--cached', '--others', '--exclude-standard', '-z',
            ))
            for name in candidates:
                original = root / name
                # Gitleaks must never follow links into private data or outside this repo.
                if original.is_symlink() or not original.is_file():
                    continue
                target = source / name
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(original, target)
            statuses.append(scan(root, ['dir', str(source)], private))
            if contents.git(root, 'rev-list', '--all').strip():
                statuses.append(scan(root, [
                    'git', '--log-opts=--all --full-history', str(root),
                ], private))
    status = max(statuses)
    print('发现疑似敏感信息，提交/推送已阻止。' if status == 1 else
          '检查未完成，提交/推送已阻止。' if status else
          '通过：Gitleaks 未发现敏感信息。')
    return status


def main():
    parser = argparse.ArgumentParser(description='本地脱敏扫描；默认检查待上传文件、暂存内容和所有本地引用的提交历史。')
    parser.add_argument('--staged', action='store_true', help='仅检查本次暂存内容，供 pre-commit 使用')
    args = parser.parse_args()
    try:
        return check(ROOT, args.staged)
    except (OSError, ValueError, subprocess.CalledProcessError):
        print('安全检查未能完成，提交/推送已阻止；请检查 Git、Python 和 Gitleaks 配置。')
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
