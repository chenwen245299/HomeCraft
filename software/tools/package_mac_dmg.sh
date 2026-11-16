#!/bin/zsh
set -euo pipefail
cd "$(dirname "$0")/.."
app="${HOMECRAFT_APP_INPUT:-dist/HomeCraft.app}"
codesign --verify --deep --strict "$app"
version=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app/Contents/Info.plist")
build=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$app/Contents/Info.plist")
[[ "$version" == "$(python3 tools/release.py value version)" && "$build" == "$(python3 tools/release.py value build)" ]]
[[ "$(lipo -archs "$app/Contents/MacOS/HomeCraft")" == "arm64" ]]
mkdir -p dist/releases
output="$PWD/dist/releases/HomeCraft-${version}-build${build}-macos-arm64.dmg"
[[ ! -e "$output" ]] || { print -u2 "安装包已存在，请使用新的构建号。"; exit 1; }
# Stage an allowlist only: never include project settings, pairing files or caches.
stage=$(mktemp -d /private/tmp/homecraft-dmg.XXXXXX)
trap 'rm -rf -- "$stage"' EXIT
ditto "$app" "$stage/HomeCraft.app"
cp docs/homecraft-install.txt "$stage/安装说明.txt"
ln -s /Applications "$stage/Applications"
hdiutil create -volname "HomeCraft" -srcfolder "$stage" -format UDZO -ov "$output"
hdiutil verify "$output"
shasum -a 256 "$output"
print "已生成：$output"
