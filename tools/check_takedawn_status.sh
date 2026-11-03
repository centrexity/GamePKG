#!/usr/bin/env bash
# Maintainer portable/native-ntdll status conversion and independent corpus replay.
set -euo pipefail
ulimit -c 0
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-status.XXXXXXXX")
task_link_only=${GAMEPKG_STATUS_LINK_ONLY:-0}
task_portable_only=${GAMEPKG_STATUS_PORTABLE_ONLY:-0}
[[ "$task_link_only" == 0 || "$task_link_only" == 1 ]]
[[ "$task_portable_only" == 0 || "$task_portable_only" == 1 ]]
[[ "$task_link_only" != 1 || "$task_portable_only" != 1 ]]
node "$task_root/tools/generate_takedawn_status.mjs" --check
cp "$task_root/engines/takedawn/native-gen/tests/status_fixture.c" "$task_work/fixture.c"
task_flags=(-std=c11 -O3 -Wall -Wextra -Werror -I"$task_runtime" -I"$task_runtime/status/reference")
task_sources=("$task_work/fixture.c" "$task_runtime/gamepkg_x86_status.c" "$task_runtime/gamepkg_x86_fault.c" "$task_runtime/gamepkg_x86_x87.c" "$task_runtime/status/reference/reference.c")
if [[ "$task_link_only" == 0 ]]; then
    "${GAMEPKG_HOST_CC:-cc}" "${task_flags[@]}" -fsanitize=address,undefined -fno-sanitize-recover=all "${task_sources[@]}" -o "$task_work/status64"
    GAMEPKG_STATUS_CORPUS="$task_work/reference.bin" "$task_work/status64"
    "${GAMEPKG_GCC32:-gcc}" -m32 -fno-pie -no-pie "${task_flags[@]}" -fsanitize=undefined -fsanitize-undefined-trap-on-error "${task_sources[@]}" -o "$task_work/status32"
    GAMEPKG_STATUS_REPLAY="$task_work/reference.bin" "$task_work/status32"
    "${GAMEPKG_MIPS_CC:-mips-linux-gnu-gcc}" "${task_flags[@]}" -static -DGAMEPKG_REQUIRE_BIG_ENDIAN=1 -fsanitize=undefined -fsanitize-undefined-trap-on-error "${task_sources[@]}" -o "$task_work/status-mips"
    GAMEPKG_STATUS_REPLAY="$task_work/reference.bin" "${GAMEPKG_QEMU_MIPS:-qemu-mips}" "$task_work/status-mips"
fi
if [[ "$task_portable_only" == 1 ]]; then
    printf 'NTSTATUS portable qualification files: %s\n' "$task_work"
    exit 0
fi
task_mingw=${GAMEPKG_MINGW_CC:-i686-w64-mingw32-gcc}
"$task_mingw" "${task_flags[@]}" "${task_sources[@]}" -o "$task_work/status-gcc.exe"
task_objects=()
for task_source in "${task_sources[@]:1}"; do
    task_object="$task_work/$(basename "${task_source%.c}").obj"
    "$task_mingw" "${task_flags[@]}" -c "$task_source" -o "$task_object"
    task_objects+=("$task_object")
done
"${GAMEPKG_CLANG:-clang}" --target=i686-w64-windows-gnu --sysroot="${GAMEPKG_MINGW_SYSROOT:-/usr/i686-w64-mingw32}" -femulated-tls "${task_flags[@]}" -c "$task_work/fixture.c" -o "$task_work/fixture-clang.obj"
"$task_mingw" "${task_flags[@]}" "$task_work/fixture-clang.obj" "${task_objects[@]}" -o "$task_work/status-mixed.exe"
if [[ "$task_link_only" == 0 ]]; then
    export WINEPREFIX=${GAMEPKG_WINEPREFIX:-"$task_work/wineprefix"} WINEDEBUG=-all
    [[ "$WINEPREFIX" == /* ]]
    task_win="Z:${task_work//\//\\}"
    for task_profile in gcc mixed; do
        GAMEPKG_STATUS_NATIVE=1 GAMEPKG_STATUS_CORPUS="$task_win\\native-$task_profile.bin" "${GAMEPKG_WINE:-wine}" "$task_work/status-$task_profile.exe"
        cmp "$task_work/reference.bin" "$task_work/native-$task_profile.bin"
    done
    GAMEPKG_STATUS_REPLAY="$task_work/native-gcc.bin" "$task_work/status64"
    GAMEPKG_STATUS_REPLAY="$task_work/native-gcc.bin" "$task_work/status32"
    GAMEPKG_STATUS_REPLAY="$task_work/native-gcc.bin" "${GAMEPKG_QEMU_MIPS:-qemu-mips}" "$task_work/status-mips"
    for task_profile in gcc mixed; do
        GAMEPKG_STATUS_REPLAY="$task_win\\native-gcc.bin" "${GAMEPKG_WINE:-wine}" "$task_work/status-$task_profile.exe"
    done
fi
printf 'NTSTATUS qualification files: %s\n' "$task_work"
