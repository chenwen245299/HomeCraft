#!/usr/bin/env python3
"""Read Codex account limits and forward a credential-free snapshot over USB."""
from __future__ import annotations

import argparse
import glob
import json
import math
import os
import selectors
import shutil
import subprocess
import sys
import termios
import time


class BridgeError(RuntimeError):
    pass


class CodexClient:
    def __init__(self, executable: str):
        self.executable = executable
        self.process = None
        self.buffer = bytearray()
        self.sequence = 0
        self.selector = selectors.DefaultSelector()

    def start(self):
        self.process = subprocess.Popen(
            [self.executable, "app-server", "--listen", "stdio://"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        )
        self.selector.register(self.process.stdout, selectors.EVENT_READ)
        self.request("initialize", {"clientInfo": {
            "name": "epaper_usage_dashboard", "title": "EPaper Usage Dashboard", "version": "1.0.0"
        }})
        self.send({"method": "initialized", "params": {}})

    def send(self, message):
        if self.process is None or self.process.poll() is not None:
            raise BridgeError("Codex app-server is not running")
        self.process.stdin.write(json.dumps(message, separators=(",", ":")).encode() + b"\n")
        self.process.stdin.flush()

    def request(self, method, params=None, timeout=30):
        self.sequence += 1
        request_id = self.sequence
        message = {"id": request_id, "method": method}
        if params is not None:
            message["params"] = params
        self.send(message)
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            while b"\n" in self.buffer:
                line, _, rest = self.buffer.partition(b"\n")
                self.buffer = bytearray(rest)
                try:
                    reply = json.loads(line)
                except (ValueError, UnicodeError):
                    continue
                if reply.get("id") != request_id:
                    continue
                if "error" in reply:
                    error = reply["error"]
                    raise BridgeError(f"{method} failed ({error.get('code', 'unknown')})")
                return reply.get("result", {})
            events = self.selector.select(max(0, deadline - time.monotonic()))
            if not events:
                break
            block = os.read(self.process.stdout.fileno(), 65536)
            if not block:
                raise BridgeError("Codex app-server closed the connection")
            self.buffer.extend(block)
            if len(self.buffer) > 1_048_576:
                raise BridgeError("Oversized app-server message")
        raise BridgeError(f"{method} timed out")

    def limits(self):
        return self.request("account/rateLimits/read")

    def close(self):
        self.selector.close()
        if self.process is not None:
            self.process.terminate()
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()


def normalize_window(window):
    if not isinstance(window, dict):
        return {"remaining_tenths": -1, "window_mins": 0, "reset_at": 0}
    used = window.get("usedPercent")
    minutes = window.get("windowDurationMins")
    reset = window.get("resetsAt")
    valid_number = lambda x: isinstance(x, (int, float)) and not isinstance(x, bool) and math.isfinite(x)
    remaining = round(max(0, min(100, 100 - used)) * 10) if valid_number(used) else -1
    return {
        "remaining_tenths": remaining,
        "window_mins": int(minutes) if valid_number(minutes) and 0 < minutes <= 525600 else 0,
        "reset_at": int(reset) if valid_number(reset) and 0 < reset <= 4102444800 else 0,
    }


def normalize_limits(result, now=None):
    buckets = result.get("rateLimitsByLimitId")
    if isinstance(buckets, dict) and isinstance(buckets.get("codex"), dict):
        bucket = buckets["codex"]
    else:
        bucket = result.get("rateLimits")
    if not isinstance(bucket, dict):
        raise BridgeError("No Codex quota bucket returned; sign in to Codex with ChatGPT")
    return {
        "fetched_at": int(time.time() if now is None else now),
        "primary": normalize_window(bucket.get("primary")),
        "secondary": normalize_window(bucket.get("secondary")),
        "status": 0,
    }


def encode_snapshot(snapshot):
    values = ["EPAPER1", str(snapshot["fetched_at"])]
    for key in ("primary", "secondary"):
        window = snapshot[key]
        values.extend(str(window[field]) for field in ("remaining_tenths", "window_mins", "reset_at"))
    values.append(str(snapshot.get("status", 0)))
    return (" ".join(values) + "\n").encode("ascii")


class UsbLink:
    """POSIX serial transport; no DTR/RTS toggles and no third-party dependencies."""
    def __init__(self, port):
        self.fd = os.open(port, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
        try:
            attrs = termios.tcgetattr(self.fd)
            attrs[0] = 0
            attrs[1] = 0
            attrs[2] = termios.CS8 | termios.CREAD | termios.CLOCAL
            attrs[3] = 0
            attrs[4] = attrs[5] = termios.B115200
            attrs[6][termios.VMIN] = 0
            attrs[6][termios.VTIME] = 0
            termios.tcsetattr(self.fd, termios.TCSANOW, attrs)
        except BaseException:
            os.close(self.fd)
            raise

    def send(self, payload):
        offset = 0
        deadline = time.monotonic() + 3
        while offset < len(payload):
            if time.monotonic() > deadline:
                raise BridgeError("USB write timed out")
            try:
                offset += os.write(self.fd, payload[offset:])
            except BlockingIOError:
                time.sleep(0.02)

    def drain(self):
        try:
            return os.read(self.fd, 65536)
        except BlockingIOError:
            return b""

    def close(self):
        os.close(self.fd)


class DeviceMessages:
    """Preserve line boundaries across USB reads, including split refresh requests."""
    def __init__(self):
        self.buffer = bytearray()
        self.dropping = False

    def feed(self, chunk):
        lines = []
        for byte in chunk:
            if byte == 10:
                if not self.dropping:
                    lines.append(self.buffer.decode('utf-8', errors='replace').rstrip('\r'))
                self.buffer.clear()
                self.dropping = False
            elif not self.dropping:
                if len(self.buffer) < 4096:
                    self.buffer.append(byte)
                else:
                    self.buffer.clear()
                    self.dropping = True
        return lines


def find_port(explicit):
    if explicit:
        return explicit
    candidates = sorted(glob.glob("/dev/cu.usbmodem*") + glob.glob("/dev/ttyACM*"))
    if len(candidates) != 1:
        raise BridgeError("Connect one ESP32 board, or specify --port /dev/cu.usbmodem...")
    return candidates[0]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", help="USB port; automatically detected when exactly one is connected")
    parser.add_argument("--interval", type=int, default=60, help="Quota poll interval in seconds (minimum 15)")
    parser.add_argument("--once", action="store_true", help="Read and send a single snapshot")
    parser.add_argument("--stdout", action="store_true", help="Read real quota without opening a USB port")
    parser.add_argument("--codex", default=shutil.which("codex"), help="Codex CLI executable")
    args = parser.parse_args(argv)
    if not args.codex:
        parser.error("Codex CLI is not installed or is not on PATH")
    client = None
    link = None
    snapshot = None
    next_poll = 0.0
    next_send = 0.0
    last_poll = 0.0
    device_messages = DeviceMessages()
    try:
        while True:
            now = time.monotonic()
            if now >= next_poll:
                try:
                    if client is None:
                        client = CodexClient(args.codex)
                        client.start()
                    snapshot = normalize_limits(client.limits())
                    print(json.dumps(snapshot, ensure_ascii=False), flush=True)
                except (BridgeError, OSError, BrokenPipeError) as error:
                    print(f"Quota unavailable: {error}", file=sys.stderr, flush=True)
                    if client is not None:
                        client.close()
                        client = None
                    if args.once:
                        return 1
                    if snapshot is not None:
                        snapshot = {**snapshot, "status": 1}
                last_poll = time.monotonic()
                next_poll = last_poll + max(15, args.interval)
                next_send = 0
            if not args.stdout and snapshot is not None:
                try:
                    if link is None:
                        port = find_port(args.port)
                        link = UsbLink(port)
                        device_messages = DeviceMessages()
                        print(f"USB connected: {port}", file=sys.stderr, flush=True)
                    if time.monotonic() >= next_send:
                        link.send(encode_snapshot(snapshot))
                        next_send = time.monotonic() + 10  # Re-send after a device reboot; no API request.
                    log = link.drain()
                    if log:
                        for line in device_messages.feed(log):
                            if "EPAPER_" in line:
                                print(line, file=sys.stderr, flush=True)
                            if line == 'EPAPER_REFRESH':
                                next_poll = min(next_poll, last_poll + 15)
                except (BridgeError, OSError, termios.error) as error:
                    if link is not None:
                        link.close()
                        link = None
                    print(f"USB waiting: {error}", file=sys.stderr, flush=True)
                    if args.once:
                        return 1
                    time.sleep(2)
            if args.once:
                return 0
            time.sleep(0.2)
    except KeyboardInterrupt:
        return 0
    finally:
        if link is not None:
            link.close()
        if client is not None:
            client.close()


if __name__ == "__main__":
    raise SystemExit(main())
