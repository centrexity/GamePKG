#!/usr/bin/env bash
# Maintainer-only checked-RAM/critical-section ABI qualification, no game media.
set -euo pipefail
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-memory-windows.XXXXXXXX")
task_link_only=${GAMEPKG_MEMORY_WINDOWS_LINK_ONLY:-0}
[[ "$task_link_only" == 0 || "$task_link_only" == 1 ]]
task_flags=(-std=c11 -O3 -Wall -Wextra -Werror -I"$task_runtime")
task_mingw=${GAMEPKG_MINGW_CC:-i686-w64-mingw32-gcc}
task_objects=()
for task_service in memory fault; do
    task_object="$task_work/$task_service.obj"
    "$task_mingw" "${task_flags[@]}" -c "$task_runtime/gamepkg_x86_$task_service.c" -o "$task_object"
    task_objects+=("$task_object")
done
"$task_mingw" "${task_flags[@]}" "$task_root/engines/takedawn/native-gen/tests/memory_runtime.c" "${task_objects[@]}" -o "$task_work/memory-gcc.exe"
"${GAMEPKG_CLANG:-clang}" --target=i686-w64-windows-gnu --sysroot="${GAMEPKG_MINGW_SYSROOT:-/usr/i686-w64-mingw32}" \
    -femulated-tls "${task_flags[@]}" -c "$task_root/engines/takedawn/native-gen/tests/memory_runtime.c" -o "$task_work/fixture-clang.obj"
"$task_mingw" "$task_work/fixture-clang.obj" "${task_objects[@]}" -o "$task_work/memory-mixed.exe"
if [[ "$task_link_only" == 0 ]]; then
    export WINEPREFIX=${GAMEPKG_WINEPREFIX:-"$task_work/wineprefix"} WINEDEBUG=-all
    [[ "$WINEPREFIX" == /* ]]
    for task_profile in gcc mixed; do
        "${GAMEPKG_WINE:-wine}" "$task_work/memory-$task_profile.exe"
    done
fi
printf 'Windows checked-RAM qualification files: %s\n' "$task_work"
