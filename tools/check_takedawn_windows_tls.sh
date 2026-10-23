#!/usr/bin/env bash
# Maintainer-only MinGW mixed-compiler ABI check; no game media or player setup.
set -euo pipefail
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_clang=${GAMEPKG_CLANG:-clang-18}
task_gcc=${GAMEPKG_MINGW_CC:-i686-w64-mingw32-gcc}
task_wine=${GAMEPKG_WINE:-wine}
task_sysroot=${GAMEPKG_MINGW_SYSROOT:-/usr/i686-w64-mingw32}
task_link_only=${GAMEPKG_WINDOWS_TLS_LINK_ONLY:-0}
[[ "$task_link_only" == 0 || "$task_link_only" == 1 ]]
for task_tool in "$task_clang" "$task_gcc"; do command -v "$task_tool" >/dev/null; done
if [[ "$task_link_only" == 0 ]]; then command -v "$task_wine" >/dev/null; fi
[[ -d "$task_sysroot/include" ]]
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-windows-tls.XXXXXXXX")
task_prefix=${GAMEPKG_WINEPREFIX:-$task_work/wine}
task_fixture=${GAMEPKG_STREAMING_FIXTURE_SOURCE:-$task_work/streaming.c}
if [[ -z ${GAMEPKG_STREAMING_FIXTURE_SOURCE:-} ]]; then
    GAMEPKG_TAKEDAWN_STREAMING_FIXTURE="$task_fixture" cargo test --release --locked \
        --manifest-path "$task_root/engines/takedawn/native-gen/Cargo.toml" \
        --test streaming export_streaming_fixture
fi
[[ -f "$task_fixture" ]]
task_flags=(-std=c11 -O3 -msse2 -mfpmath=sse -Wall -Wextra -Werror -I"$task_runtime")
for task_source in memory x87 fault; do
    "$task_gcc" "${task_flags[@]}" -c "$task_runtime/gamepkg_x86_$task_source.c" -o "$task_work/$task_source.obj"
done
# Clang defaults to native Windows TLS, while GCC MinGW uses emulated TLS.
# Every Clang translation unit that shares GCC TLS MUST select this mode.
task_cross=(--target=i686-w64-windows-gnu --sysroot="$task_sysroot" -femulated-tls)
"$task_clang" "${task_cross[@]}" "${task_flags[@]}" -c "$task_fixture" -o "$task_work/streaming.obj"
"$task_gcc" "$task_work/streaming.obj" "$task_work/memory.obj" "$task_work/x87.obj" "$task_work/fault.obj" -o "$task_work/streaming.exe"
if [[ "$task_link_only" == 0 ]]; then WINEPREFIX="$task_prefix" WINEDEBUG=-all "$task_wine" "$task_work/streaming.exe"; fi
"$task_clang" "${task_cross[@]}" "${task_flags[@]}" -c "$task_root/engines/takedawn/native-gen/tests/windows_tls.c" -o "$task_work/tls.obj"
"$task_gcc" "$task_work/tls.obj" "$task_work/fault.obj" -o "$task_work/tls.exe"
if [[ "$task_link_only" == 0 ]]; then WINEPREFIX="$task_prefix" WINEDEBUG=-all "$task_wine" "$task_work/tls.exe"; fi
printf 'Windows Clang/GCC ABI compile/link complete (link-only=%s)\n' "$task_link_only"
printf 'Asset-free qualification files: %s\n' "$task_work"
