#!/usr/bin/env bash
# Synthetic native x87 oracle and architecture-neutral full-state replay.
set -euo pipefail
ulimit -c 0
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-x87-bcd.XXXXXXXX")
task_portable=${GAMEPKG_BCD_PORTABLE_ONLY:-0}
task_link=${GAMEPKG_BCD_LINK_ONLY:-0}
[[ "$task_portable" == 0 || "$task_portable" == 1 ]]
[[ "$task_link" == 0 || "$task_link" == 1 ]]
[[ "$task_portable" != 1 || "$task_link" != 1 ]]
GAMEPKG_X87_BCD_FIXTURE="$task_work/bcd.c" cargo test --locked --release \
  --manifest-path "$task_root/engines/takedawn/native-gen/Cargo.toml" --test x87_bcd export_bcd_fixture
task_flags=(-std=c11 -O3 -pthread -Wall -Wextra -Werror -I"$task_runtime" -I"$task_runtime/softfloat" \
  -I"$task_runtime/softfloat/source/include" -I"$task_runtime/softfloat/source/8086-SSE")
task_sources=()
for task_name in x87 x87_load x87_store fault memory; do task_sources+=("$task_runtime/gamepkg_x86_$task_name.c"); done
if [[ "$task_link" == 0 ]]; then
  "${GAMEPKG_HOST_CC:-cc}" "${task_flags[@]}" -msse2 -mfpmath=sse -fsanitize=undefined -fno-sanitize-recover=all \
    "$task_work/bcd.c" "${task_sources[@]}" -o "$task_work/host"
  GAMEPKG_BCD_CORPUS="$task_work/host.bin" "$task_work/host"
  "${GAMEPKG_GCC32:-gcc}" -m32 "${task_flags[@]}" -msse2 -mfpmath=sse -fsanitize=undefined -fsanitize-undefined-trap-on-error \
    "$task_work/bcd.c" "${task_sources[@]}" -o "$task_work/linux32"
  GAMEPKG_BCD_CORPUS="$task_work/linux32.bin" "$task_work/linux32"
  cmp "$task_work/host.bin" "$task_work/linux32.bin"
  "${GAMEPKG_MIPS_CC:-mips-linux-gnu-gcc}" "${task_flags[@]}" -static -DGAMEPKG_BCD_REPLAY=1 \
    -fsanitize=undefined -fsanitize-undefined-trap-on-error "$task_work/bcd.c" "${task_sources[@]}" -o "$task_work/mips32"
  GAMEPKG_BCD_CORPUS="$task_work/host.bin" "${GAMEPKG_QEMU_MIPS:-qemu-mips}" "$task_work/mips32"
fi
if [[ "$task_portable" == 0 ]]; then
  task_mingw=${GAMEPKG_MINGW_CC:-i686-w64-mingw32-gcc}
  task_objects=()
  for task_source in "${task_sources[@]}"; do
    task_object="$task_work/$(basename "${task_source%.c}").obj"
    "$task_mingw" "${task_flags[@]}" -c "$task_source" -o "$task_object"
    task_objects+=("$task_object")
  done
  "$task_mingw" "${task_flags[@]}" -msse2 -mfpmath=sse "$task_work/bcd.c" "${task_objects[@]}" -o "$task_work/gcc.exe"
  "${GAMEPKG_CLANG:-clang}" --target=i686-w64-windows-gnu --sysroot="${GAMEPKG_MINGW_SYSROOT:-/usr/i686-w64-mingw32}" \
    -femulated-tls "${task_flags[@]}" -msse2 -mfpmath=sse -c "$task_work/bcd.c" -o "$task_work/clang.obj"
  "$task_mingw" "$task_work/clang.obj" "${task_objects[@]}" -o "$task_work/mixed.exe"
  if [[ "$task_link" == 0 ]]; then
    export WINEPREFIX=${GAMEPKG_WINEPREFIX:-"$task_work/wineprefix"} WINEDEBUG=-all
    [[ "$WINEPREFIX" == /* ]]
    for task_profile in gcc mixed; do
      task_native=$("${GAMEPKG_WINEPATH:-winepath}" -w "$task_work/$task_profile.bin")
      GAMEPKG_BCD_CORPUS="$task_native" "${GAMEPKG_WINE:-wine}" "$task_work/$task_profile.exe"
      cmp "$task_work/host.bin" "$task_work/$task_profile.bin"
    done
  fi
fi
if [[ "$task_link" == 0 ]]; then sha256sum "$task_work/host.bin"; fi
printf 'BCD qualification files: %s\n' "$task_work"
