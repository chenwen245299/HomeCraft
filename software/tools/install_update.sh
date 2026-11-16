#!/bin/sh
# Runs only after download/identity verification. All paths arrive as arguments, never code.
set -eu
pid="$1"; target="$2"; source_app="$3"; version="$4"; build="$5"; relaunch="$6"
case "$pid" in ''|*[!0-9]*) exit 1;; esac
[ "$(basename "$target")" = HomeCraft.app ] && [ ! -L "$target" ] || exit 1
stage=$(dirname "$(dirname "$source_app")")
case "$(basename "$stage")" in homecraft-update-*) ;; *) exit 1;; esac
trap 'rm -rf "$stage"' EXIT
count=0
while kill -0 "$pid" 2>/dev/null; do
  count=$((count+1)); [ "$count" -lt 1500 ] || exit 1
  sleep 0.2
done
work=$(mktemp -d "$(dirname "$target")/.homecraft-update.XXXXXXXX")
new="$work/HomeCraft.app"; previous="$work/previous.app"
cleanup() {
  # If a rename failed, restore the existing version before removing the staging area.
  if [ -d "$previous" ] && [ ! -e "$target" ]; then mv "$previous" "$target"; fi
  if [ ! -d "$previous" ]; then rm -rf "$work"; fi
  rm -rf "$stage"
}
trap cleanup EXIT HUP INT TERM
/usr/bin/ditto "$source_app" "$new"
/usr/bin/codesign --verify --deep --strict "$new"
[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$target/Contents/Info.plist")" = local.epaper.bluetooth ]
[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$new/Contents/Info.plist")" = local.epaper.bluetooth ]
[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$new/Contents/Info.plist")" = "$version" ]
[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$new/Contents/Info.plist")" = "$build" ]
mv "$target" "$previous"
if mv "$new" "$target"; then
  rm -rf "$previous"
else
  mv "$previous" "$target"
  exit 1
fi
# Settings, iCloud and device bindings live outside the App and are never touched here.
if [ "$relaunch" = 1 ]; then /usr/bin/open "$target"; fi
