#!/usr/bin/env bash
# Maintainer synthetic descriptor resolution/accessed RAM qualification.
set -euo pipefail
ulimit -c 0
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_test="$task_root/engines/takedawn/native-gen/tests/portable_far_memory.c"
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-far-memory.XXXXXXXX")
task_portable=${GAMEPKG_FAR_MEMORY_PORTABLE_ONLY:-0}
task_link=${GAMEPKG_FAR_MEMORY_LINK_ONLY:-0}
[[ "$task_portable" == 0 || "$task_portable" == 1 ]]
[[ "$task_link" == 0 || "$task_link" == 1 ]]
[[ "$task_link" != 1 || "$task_portable" != 1 ]]
task_flags=(-std=c11 -O3 -Wall -Wextra -Werror -pthread -I"$task_runtime")
task_sources=()
for task_name in far_memory far_jump memory fault; do task_sources+=("$task_runtime/gamepkg_x86_$task_name.c"); done
if [[ "$task_link" == 0 ]]; then
  "${GAMEPKG_HOST_CC:-cc}" "${task_flags[@]}" -fsanitize=address,undefined -fno-sanitize-recover=all "$task_test" "${task_sources[@]}" -o "$task_work/host"
  "$task_work/host"
  "${GAMEPKG_GCC32:-gcc}" -m32 "${task_flags[@]}" -fsanitize=undefined -fsanitize-undefined-trap-on-error "$task_test" "${task_sources[@]}" -o "$task_work/linux32"
  "$task_work/linux32"
  "${GAMEPKG_MIPS_CC:-mips-linux-gnu-gcc}" "${task_flags[@]}" -static -DGAMEPKG_REQUIRE_BIG_ENDIAN=1 -fsanitize=undefined -fsanitize-undefined-trap-on-error "$task_test" "${task_sources[@]}" -o "$task_work/mips32"
  "${GAMEPKG_QEMU_MIPS:-qemu-mips}" "$task_work/mips32"
fi
if [[ "$task_portable" == 0 ]]; then
  task_mingw=${GAMEPKG_MINGW_CC:-i686-w64-mingw32-gcc}
  task_objects=()
  for task_source in "${task_sources[@]}"; do
    task_object="$task_work/$(basename "${task_source%.c}").obj"
    "$task_mingw" "${task_flags[@]}" -c "$task_source" -o "$task_object"
    task_objects+=("$task_object")
  done
  "$task_mingw" "${task_flags[@]}" "$task_test" "${task_objects[@]}" -o "$task_work/gcc.exe"
  "${GAMEPKG_CLANG:-clang}" --target=i686-w64-windows-gnu --sysroot="${GAMEPKG_MINGW_SYSROOT:-/usr/i686-w64-mingw32}" -femulated-tls "${task_flags[@]}" -c "$task_test" -o "$task_work/clang.obj"
  "$task_mingw" -pthread "$task_work/clang.obj" "${task_objects[@]}" -o "$task_work/mixed.exe"
  if [[ "$task_link" == 0 ]]; then
    export WINEPREFIX=${GAMEPKG_WINEPREFIX:-"$task_work/wineprefix"} WINEDEBUG=-all
    [[ "$WINEPREFIX" == /* ]]
    for task_profile in gcc mixed; do "${GAMEPKG_WINE:-wine}" "$task_work/$task_profile.exe"; done
  fi
fi
printf 'Far-memory qualification files: %s\n' "$task_work"
