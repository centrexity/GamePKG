#!/usr/bin/env bash
# Developer qualification only; Android players never need this script or Python.
set -euo pipefail
serial=${1:?Pass the serial of your dedicated Android test device}
target=${2:-i686-linux-android}
publication_binary=${3:?Pass the library test executable reported by Cargo}
[ -f "$publication_binary" ] && [ -x "$publication_binary" ] || { echo 'The library test executable is missing.' >&2; exit 1; }
case "$target" in
  i686-linux-android|armv7-linux-androideabi) ;;
  *) echo 'Select a 32-bit Android target.' >&2; exit 1 ;;
esac
adb=${ADB:-adb}
remote=/data/local/tmp/gamepkg-portability32
"$adb" -s "$serial" shell mkdir -p "$remote"
found=0
for kind in components portability runtime memory_budget audio_effect_assets; do
  for binary in "target/$target/debug/deps/$kind"-*; do
    [ -f "$binary" ] && [ -x "$binary" ] || continue
    name=$(basename "$binary")
    "$adb" -s "$serial" push "$binary" "$remote/$name"
    "$adb" -s "$serial" shell "TMPDIR=$remote GAMEPKG_CONFIG=$remote/settings.toml $remote/$name --test-threads=1"
    found=$((found + 1))
  done
done
[ "$found" -eq 5 ] || { echo 'Build exactly the five Android test binaries before running this script.' >&2; exit 1; }
name=$(basename "$publication_binary")
"$adb" -s "$serial" push "$publication_binary" "$remote/$name"
"$adb" -s "$serial" shell "TMPDIR=$remote $remote/$name pack::publication_tests::"
