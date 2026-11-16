#!/bin/zsh
cd "$(dirname "$0")/.." || exit 1
exec .ble-venv/bin/python tools/ble_bridge.py --stop
