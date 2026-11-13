#!/usr/bin/env bash
# Developer qualification only; Android players never need this script or Python.
set -euo pipefail
serial=${1:?Pass the serial of your dedicated Android test device}
target=${2:-i686-linux-android}
artifacts=${3:?Pass the JSON test-artifact output from Cargo}
[ -f "$artifacts" ] || { echo 'The Cargo test-artifact manifest is missing.' >&2; exit 1; }
case "$target" in
  i686-linux-android|armv7-linux-androideabi) ;;
  *) echo 'Select a 32-bit Android target.' >&2; exit 1 ;;
esac
# Cargo owns its output paths. Do not infer them from a default target folder,
# glob stale builds, or depend on an Android ELF's host executable permission.
select_artifact() {
  jq -ser --arg name "$1" --arg kind "$2" '
    [.[] | select(.reason == "compiler-artifact" and .profile.test == true
      and .target.name == $name and (.target.kind | index($kind)) != null)
      | .executable // empty] | unique
    | if length == 1 then .[0] else error("expected exactly one test executable for " + $name) end
  ' "$artifacts"
}
binaries=()
for kind in components portability runtime render_resources memory_budget audio_effect_assets music_permissions game_arkanoo; do
  binary=$(select_artifact "$kind" test)
  [ -f "$binary" ] || { echo "The compiled Android suite is missing: $binary" >&2; exit 1; }
  binaries+=("$binary")
done
publication_binary=$(select_artifact gamepkg lib)
[ -f "$publication_binary" ] || { echo 'The compiled library test executable is missing.' >&2; exit 1; }
# The command below runs on Android's shell, so restrict its basename as well
# as quoting the local paths (which can contain spaces).
for binary in "${binaries[@]}" "$publication_binary"; do
  name=$(basename "$binary")
  [[ "$name" =~ ^[a-zA-Z0-9_-]+$ ]] || { echo 'Unexpected Android test executable name.' >&2; exit 1; }
done
adb=${ADB:-adb}
remote=/data/local/tmp/gamepkg-portability32
"$adb" -s "$serial" shell mkdir -p "$remote"
for binary in "${binaries[@]}"; do
  name=$(basename "$binary")
  "$adb" -s "$serial" push "$binary" "$remote/$name"
  "$adb" -s "$serial" shell chmod 700 "$remote/$name"
  "$adb" -s "$serial" shell "TMPDIR=$remote GAMEPKG_CONFIG=$remote/settings.toml $remote/$name --test-threads=1"
done
name=$(basename "$publication_binary")
"$adb" -s "$serial" push "$publication_binary" "$remote/$name"
"$adb" -s "$serial" shell chmod 700 "$remote/$name"
"$adb" -s "$serial" shell "TMPDIR=$remote $remote/$name pack::publication_tests::"
