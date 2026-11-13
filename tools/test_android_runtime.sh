#!/usr/bin/env bash
# Developer qualification only; Android players never need this script or Python.
set -euo pipefail
serial=${1:?Pass the serial of your dedicated Android test device}
target=${2:-i686-linux-android}
artifacts=${3:?Pass the Cargo JSON test artifact file}
case "$target" in
  i686-linux-android|armv7-linux-androideabi) ;;
  *) echo 'Select a 32-bit Android target.' >&2; exit 1 ;;
esac
adb=${ADB:-adb}
script_directory=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
artifact_paths=$(mktemp)
trap 'rm -f "$artifact_paths"' EXIT
python3 "$script_directory/android_test_artifacts.py" "$artifacts" "$script_directory/../Cargo.toml" > "$artifact_paths"
mapfile -d '' -t binaries < "$artifact_paths"
[ "${#binaries[@]}" -eq 9 ] || { echo 'Cargo must report eight suites and one library test executable.' >&2; exit 1; }
publication_binary=${binaries[8]}
remote=/data/local/tmp/gamepkg-portability32
"$adb" -s "$serial" shell mkdir -p "$remote"
for binary in "${binaries[@]:0:8}"; do
  name=$(basename "$binary")
  "$adb" -s "$serial" push "$binary" "$remote/$name"
  "$adb" -s "$serial" shell "chmod 700 $remote/$name"
  "$adb" -s "$serial" shell "TMPDIR=$remote GAMEPKG_CONFIG=$remote/settings.toml $remote/$name --test-threads=1"
done
name=$(basename "$publication_binary")
"$adb" -s "$serial" push "$publication_binary" "$remote/$name"
"$adb" -s "$serial" shell "chmod 700 $remote/$name"
"$adb" -s "$serial" shell "TMPDIR=$remote $remote/$name pack::publication_tests::"
