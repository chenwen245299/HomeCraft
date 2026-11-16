#!/usr/bin/env python3
"""One-time USB provisioning; exits after saving and never runs a quota bridge."""
import argparse
import base64
import getpass
import json
import os
from pathlib import Path
import time
from codex_bridge import UsbLink, DeviceMessages, find_port


def current_codex_access(auth):
    tokens = auth.get('tokens') or {}
    access = tokens.get('access_token')
    account = tokens.get('account_id')
    if not isinstance(access, str) or not access or not isinstance(account, str) or not account:
        raise ValueError('电脑当前没有可用的 ChatGPT/Codex 登录')
    payload = access.split('.')[1]
    claims = json.loads(base64.urlsafe_b64decode(payload + '=' * (-len(payload) % 4)))
    # Deliberately exclude the desktop refresh token, so its rotation cannot log the computer out.
    return {'access_token': access, 'account_id': account, 'client_id': claims.get('client_id', '')}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port')
    parser.add_argument('--credentials', help='一次性授权工具生成的 JSON 文件')
    parser.add_argument('--current-codex', action='store_true', help='只复制当前 Codex access token；到期后需重新授权')
    parser.add_argument('--ssid', help='Wi-Fi 名称；密码通过隐藏输入提供')
    parser.add_argument('--open-network', action='store_true')
    parser.add_argument('--deepseek-env', action='store_true', help='从 DEEPSEEK_API_KEY 环境变量读取官方 API key')
    args = parser.parse_args()
    config = {}
    if args.credentials:
        config.update(json.loads(Path(args.credentials).expanduser().read_text()))
    if args.current_codex:
        config['codex'] = current_codex_access(json.loads((Path.home()/'.codex/auth.json').read_text()))
    if args.ssid:
        config['wifi'] = {'ssid': args.ssid, 'password': '' if args.open_network else getpass.getpass('Wi-Fi 密码：'), 'open_network': args.open_network}
    if args.deepseek_env:
        key = os.environ.get('DEEPSEEK_API_KEY', '')
        if not key:
            parser.error('未设置 DEEPSEEK_API_KEY。也可以直接在配网页面中填写。')
        config['deepseek'] = {'api_key': key}
    if not config:
        parser.error('请提供 --credentials、--current-codex、--ssid 或 --deepseek-env')
    body = json.dumps(config, ensure_ascii=True, separators=(',', ':')).encode('ascii')
    if len(body) > 32768:
        parser.error('配置文件过大')
    link = UsbLink(find_port(args.port))
    messages = DeviceMessages()
    try:
        link.drain()
        link.send(b'EPAPER_CONFIG '+body+b'\n')
        deadline = time.monotonic()+30
        while time.monotonic()<deadline:
            for line in messages.feed(link.drain()):
                if line.startswith('EPAPER_CONFIG_ACK '):
                    code = int(line.split()[-1])
                    if code:
                        print('设备没有保存配置。请在网络设置页三击 BOOT 打开配网窗口，再重试。错误码：'+str(code))
                        return 1
                    print('配置已保存到开发板。此程序现在退出，后续由开发板通过 Wi-Fi 自行查询。')
                    if args.current_codex:
                        print('当前仅导入 access token；长期独立运行请使用 authorize_device.py 创建专属登录。')
                    return 0
            time.sleep(.05)
        print('未收到保存回执，请检查 USB 连接和配网窗口。')
        return 1
    finally:
        link.close()


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, IndexError) as error:
        # Do not echo configuration, credential strings or HTTP bodies.
        print('配置未完成：'+type(error).__name__)
        raise SystemExit(1)
