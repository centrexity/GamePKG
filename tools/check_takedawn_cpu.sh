#!/usr/bin/env bash
# Maintainer-only shared virtual-CPU ownership tests; no game media.
set -euo pipefail
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-cpu.XXXXXXXX")
task_link_only=${GAMEPKG_CPU_LINK_ONLY:-0}
task_portable_only=${GAMEPKG_CPU_PORTABLE_ONLY:-0}
[[ "$task_link_only" == 0 || "$task_link_only" == 1 ]]
[[ "$task_portable_only" == 0 || "$task_portable_only" == 1 ]]
[[ "$task_link_only" != 1 || "$task_portable_only" != 1 ]]
task_flags=(-std=c11 -O3 -Wall -Wextra -Werror -I"$task_runtime")
task_sources=("$task_root/engines/takedawn/native-gen/tests/cpu_domain.c" "$task_runtime/gamepkg_x86_cpu.c" "$task_runtime/gamepkg_x86_interrupt.c")
if [[ "$task_link_only" == 0 ]]; then
    "${GAMEPKG_HOST_CC:-cc}" "${task_flags[@]}" -pthread -fsanitize=address,undefined -fno-sanitize-recover=all "${task_sources[@]}" -o "$task_work/host"
    "$task_work/host"
    "${GAMEPKG_GCC32:-gcc}" -m32 "${task_flags[@]}" -pthread -fsanitize=undefined -fsanitize-undefined-trap-on-error "${task_sources[@]}" -o "$task_work/linux32"
    "$task_work/linux32"
    "${GAMEPKG_MIPS_CC:-mips-linux-gnu-gcc}" "${task_flags[@]}" -pthread -static -DGAMEPKG_REQUIRE_BIG_ENDIAN=1 -fsanitize=undefined -fsanitize-undefined-trap-on-error "${task_sources[@]}" -o "$task_work/mips32"
    "${GAMEPKG_QEMU_MIPS:-qemu-mips}" "$task_work/mips32"
fi
if [[ "$task_portable_only" == 1 ]]; then
    printf 'CPU ownership portability files: %s\n' "$task_work"
    exit 0
fi
task_mingw=${GAMEPKG_MINGW_CC:-i686-w64-mingw32-gcc}
task_objects=()
for task_service in cpu interrupt; do
    task_object="$task_work/$task_service.obj"
    "$task_mingw" "${task_flags[@]}" -c "$task_runtime/gamepkg_x86_$task_service.c" -o "$task_object"
    task_objects+=("$task_object")
done
"$task_mingw" "${task_flags[@]}" "${task_sources[0]}" "${task_objects[@]}" -o "$task_work/domain-gcc.exe"
"${GAMEPKG_CLANG:-clang}" --target=i686-w64-windows-gnu --sysroot="${GAMEPKG_MINGW_SYSROOT:-/usr/i686-w64-mingw32}" "${task_flags[@]}" -c "${task_sources[0]}" -o "$task_work/fixture-clang.obj"
"$task_mingw" "$task_work/fixture-clang.obj" "${task_objects[@]}" -o "$task_work/domain-mixed.exe"
if [[ "$task_link_only" == 0 ]]; then
    export WINEPREFIX=${GAMEPKG_WINEPREFIX:-"$task_work/wineprefix"} WINEDEBUG=-all
    [[ "$WINEPREFIX" == /* ]]
    for task_profile in gcc mixed; do "${GAMEPKG_WINE:-wine}" "$task_work/domain-$task_profile.exe"; done
fi
printf 'CPU ownership qualification files: %s\n' "$task_work"
