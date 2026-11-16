#!/bin/zsh
set -euo pipefail
cd "$(dirname "$0")/.."
python3 tools/release.py check
export MACOSX_DEPLOYMENT_TARGET="$(python3 tools/release.py value minimum_macos)"
cargo build --release --locked --offline --manifest-path mac-app/Cargo.toml
app="${HOMECRAFT_APP_OUTPUT:-dist/HomeCraft.app}"
[[ "$app" == */HomeCraft.app ]] || { print -u2 "输出必须是 HomeCraft.app。"; exit 1; }
[[ "$(lipo -archs mac-app/target/release/epaper-ble)" == "arm64" ]] || { print -u2 "需要 Apple 芯片的 arm64 构建。"; exit 1; }
mkdir -p "$(dirname "$app")"
stage=$(mktemp -d "$(dirname "$app")/.homecraft-bundle.XXXXXXXX")
trap 'rm -rf -- "$stage"' EXIT
bundle="$stage/HomeCraft.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp mac-app/target/release/epaper-ble "$bundle/Contents/MacOS/HomeCraft"
python3 tools/release.py plist "$bundle/Contents/Info.plist"
cp assets/app-icon/HomeCraft.icns "$bundle/Contents/Resources/HomeCraft.icns"
cp assets/poems/poems.json "$bundle/Contents/Resources/poems.json"
codesign --force --deep --sign - --identifier local.epaper.bluetooth "$bundle"
codesign --verify --deep --strict "$bundle"
plutil -lint "$bundle/Contents/Info.plist"
# Start from a clean allowlist and retain the previous local output.
if [[ -e "$app" ]]; then
  previous="$app.previous.$(date -u +%Y%m%dT%H%M%SZ).$(uuidgen)"
  mv -- "$app" "$previous"
fi
mv -- "$bundle" "$app"
print "已生成：$PWD/$app"
