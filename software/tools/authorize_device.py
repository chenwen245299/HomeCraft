#!/usr/bin/env python3
"""Create an independent Codex login for the board, once; never run a bridge."""
import argparse
import base64
import datetime
import json
import os
from pathlib import Path
from project_paths import ROOT
import secrets
import shutil
import subprocess


def export_codex(auth):
    tokens = auth.get('tokens') or {}
    access = tokens.get('access_token')
    refresh = tokens.get('refresh_token')
    account = tokens.get('account_id')
    if not all(isinstance(x, str) and x for x in (access, refresh, account)):
        raise ValueError('独立登录没有返回完整 Codex 凭据。')
    payload = access.split('.')[1]
    claims = json.loads(base64.urlsafe_b64decode(payload + '=' * (-len(payload) % 4)))
    client = claims.get('client_id')
    if not isinstance(client, str) or not client:
        raise ValueError('登录令牌缺少 OAuth client_id，无法配置自动续期。')
    return {'codex': {'access_token': access, 'refresh_token': refresh,
                      'account_id': account, 'client_id': client}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--browser', action='store_true', help='使用官方浏览器登录，代替设备码登录')
    parser.add_argument('--output', default=str(ROOT / '.device-credentials.json'))
    args = parser.parse_args()
    codex = shutil.which('codex')
    if not codex:
        parser.error('本机未找到 Codex CLI。')
    root = ROOT / '.device-auth'
    root.mkdir(mode=0o700, exist_ok=True)
    os.chmod(root, 0o700)
    folder = root / ('login-' + datetime.datetime.now().strftime('%Y%m%d-%H%M%S-') + secrets.token_hex(3))
    folder.mkdir(mode=0o700)
    print('即将通过官方 Codex CLI 为开发板创建独立登录；不会修改电脑现有登录，也不会调用模型。', flush=True)
    env = {**os.environ, 'CODEX_HOME': str(folder), 'CODEX_CLI_AUTH_STORE': 'file'}
    command = [codex, '-c', 'cli_auth_credentials_store="file"', 'login']
    if not args.browser:
        command.append('--device-auth')
    result = subprocess.run(command, env=env)
    if result.returncode:
        print('登录未完成。若设备码登录不可用，可添加 --browser 使用官方浏览器登录。')
        return result.returncode
    try:
        auth = json.loads((folder / 'auth.json').read_text())
        payload = export_codex(auth)
        output = Path(args.output).expanduser().resolve()
        fd = os.open(output, os.O_WRONLY | os.O_CREAT | os.O_TRUNC | getattr(os, 'O_NOFOLLOW', 0), 0o600)
        os.fchmod(fd, 0o600)
        with os.fdopen(fd, 'w') as handle:
            json.dump(payload, handle, ensure_ascii=False)
            handle.write('\n')
        print('授权文件已生成：' + str(output))
        print('在开发板配网页面导入该文件并保存。完成后电脑无需保持运行；不要分享此文件。')
        return 0
    except (OSError, ValueError, KeyError, IndexError) as error:
        print('导出未完成：' + str(error))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
