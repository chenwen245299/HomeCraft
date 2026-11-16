#!/bin/zsh
set -euo pipefail
cd "$(dirname "$0")/.."
tag="${1:?需要版本标签，例如 v0.3.1}"
repository="${2:-$(python3 tools/release.py value repository)}"
python3 tools/release.py check
app="${HOMECRAFT_APP_INPUT:-dist/HomeCraft.app}"
stem="$(python3 tools/release.py value stem)"
mkdir -p dist/releases
[[ ! -e "dist/releases/$stem.zip" ]] || { print -u2 "本版本 ZIP 已存在，请使用新的版本/构建号。"; exit 1; }
codesign --verify --deep --strict "$app"
ditto -c -k --keepParent --norsrc "$app" "dist/releases/$stem.zip"
zsh tools/package_mac_dmg.sh
python3 tools/release.py prepare --tag "$tag" --repository "$repository" --out dist/releases
print "发布附件与更新清单已生成：$stem"
