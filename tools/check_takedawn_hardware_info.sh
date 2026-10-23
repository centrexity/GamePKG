#!/usr/bin/env bash
# Maintainer-only kernel data ABI and read-only mapping tests; no game media.
set -euo pipefail
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-hardware-info.XXXXXXXX")
task_link_only=${GAMEPKG_HARDWARE_INFO_LINK_ONLY:-0}
task_portable_only=${GAMEPKG_HARDWARE_INFO_PORTABLE_ONLY:-0}
[[ "$task_link_only" == 0 || "$task_link_only" == 1 ]]
[[ "$task_portable_only" == 0 || "$task_portable_only" == 1 ]]
[[ "$task_link_only" != 1 || "$task_portable_only" != 1 ]]
GAMEPKG_TAKEDAWN_HARDWARE_FIXTURE="$task_work/hardware.c" cargo test --locked --release \
    --manifest-path "$task_root/engines/takedawn/native-gen/Cargo.toml" --test hardware_info export_hardware_info_fixture
task_flags=(-std=c11 -O3 -Wall -Wextra -Werror -pthread -I"$task_runtime")
task_sources=("$task_runtime/gamepkg_x86_fault.c" "$task_runtime/gamepkg_x86_memory.c")
if [[ "$task_link_only" == 0 ]]; then
    "${GAMEPKG_HOST_CC:-cc}" "${task_flags[@]}" -fsanitize=address,undefined -fno-sanitize-recover=all "$task_work/hardware.c" "${task_sources[@]}" -o "$task_work/host"
    "$task_work/host"
    "${GAMEPKG_GCC32:-gcc}" -m32 "${task_flags[@]}" -fsanitize=undefined -fsanitize-undefined-trap-on-error "$task_work/hardware.c" "${task_sources[@]}" -o "$task_work/linux32"
    "$task_work/linux32"
    "${GAMEPKG_MIPS_CC:-mips-linux-gnu-gcc}" "${task_flags[@]}" -static -DGAMEPKG_REQUIRE_BIG_ENDIAN=1 \
        -fsanitize=undefined -fsanitize-undefined-trap-on-error "$task_work/hardware.c" "${task_sources[@]}" -o "$task_work/mips32"
    "${GAMEPKG_QEMU_MIPS:-qemu-mips}" "$task_work/mips32"
fi
if [[ "$task_portable_only" == 1 ]]; then
    printf 'Hardware-info portability files: %s\n' "$task_work"
    exit 0
fi
task_mingw=${GAMEPKG_MINGW_CC:-i686-w64-mingw32-gcc}
task_objects=()
for task_source in "${task_sources[@]}"; do
    task_object="$task_work/$(basename "${task_source%.c}").obj"
    "$task_mingw" "${task_flags[@]}" -c "$task_source" -o "$task_object"
    task_objects+=("$task_object")
done
"$task_mingw" "${task_flags[@]}" "$task_work/hardware.c" "${task_objects[@]}" -o "$task_work/hardware-gcc.exe"
"${GAMEPKG_CLANG:-clang}" --target=i686-w64-windows-gnu --sysroot="${GAMEPKG_MINGW_SYSROOT:-/usr/i686-w64-mingw32}" \
    -femulated-tls "${task_flags[@]}" -c "$task_work/hardware.c" -o "$task_work/hardware-clang.obj"
"$task_mingw" "$task_work/hardware-clang.obj" "${task_objects[@]}" -o "$task_work/hardware-mixed.exe"
if [[ "$task_link_only" == 0 ]]; then
    export WINEPREFIX=${GAMEPKG_WINEPREFIX:-"$task_work/wineprefix"} WINEDEBUG=-all
    [[ "$WINEPREFIX" == /* ]]
    for task_profile in gcc mixed; do "${GAMEPKG_WINE:-wine}" "$task_work/hardware-$task_profile.exe"; done
fi
printf 'Hardware-info qualification files: %s\n' "$task_work"
