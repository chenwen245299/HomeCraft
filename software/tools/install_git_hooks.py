#!/usr/bin/env python3
"""Enable this repository's tracked hooks without replacing existing custom hooks."""
from pathlib import Path
import subprocess

from check_secrets import gitleaks_ready

ROOT = Path(__file__).resolve().parents[2]
HOOKS = 'software/tools/git-hooks'


def install(root=ROOT):
    if not gitleaks_ready():
        return 2
    configured = subprocess.run(
        ['git', '-C', str(root), 'config', '--get', 'core.hooksPath'],
        capture_output=True, text=True,
    )
    if configured.returncode not in (0, 1):
        raise RuntimeError('无法读取 hooksPath')
    if configured.returncode == 0 and configured.stdout.strip() != HOOKS:
        print('检测到已有自定义 Git hooks 路径；未覆盖，请先合并现有检查。')
        return 1
    if configured.returncode == 1:
        git_dir = subprocess.check_output(
            ['git', '-C', str(root), 'rev-parse', '--absolute-git-dir'], text=True,
        ).strip()
        if any((Path(git_dir) / 'hooks' / name).exists() for name in ('pre-commit', 'pre-push')):
            print('检测到已有 pre-commit/pre-push；未覆盖，请先合并现有检查。')
            return 1
    for name in ('pre-commit', 'pre-push'):
        file = root / HOOKS / name
        file.chmod(file.stat().st_mode | 0o111)
    subprocess.run(
        ['git', '-C', str(root), 'config', '--local', 'core.hooksPath', HOOKS], check=True,
    )
    print('已启用本仓库的提交前、推送前敏感信息检查。')
    return 0


if __name__ == '__main__':
    try:
        raise SystemExit(install())
    except (OSError, RuntimeError, subprocess.CalledProcessError):
        print('未能安装 Git hooks；请检查本仓库的 Git 配置写入权限。')
        raise SystemExit(2)
