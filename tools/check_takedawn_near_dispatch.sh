#!/usr/bin/env bash
# Independent native ELF32 near-control fault oracle and portable replay.
set -euo pipefail
ulimit -c 0
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_tests="$task_root/engines/takedawn/native-gen/tests"
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-near-dispatch.XXXXXXXX")
task_portable=${GAMEPKG_NEAR_DISPATCH_PORTABLE_ONLY:-0}
task_link=${GAMEPKG_NEAR_DISPATCH_LINK_ONLY:-0}
[[ "$task_portable" == 0 || "$task_portable" == 1 ]]
[[ "$task_link" == 0 || "$task_link" == 1 ]]
[[ "$task_link" != 1 || "$task_portable" != 1 ]]
GAMEPKG_NEAR_DISPATCH_FIXTURE="$task_work/near.c" cargo test --locked --release --manifest-path "$task_root/engines/takedawn/native-gen/Cargo.toml" --test near_dispatch export_near_dispatch_fixture
task_flags=(-std=c11 -O3 -Wall -Wextra -Werror -pthread -I"$task_runtime")
task_sources=()
for task_name in dispatch cpu interrupt cpu_dispatch table_store far_memory far_jump memory fault x87; do task_sources+=("$task_runtime/gamepkg_x86_$task_name.c"); done
if [[ "$task_link" == 0 ]]; then
 task_gcc32=${GAMEPKG_GCC32:-gcc}
 task_physical_flags=();if [[ -n "${GAMEPKG_PHYSICAL_INCLUDE:-}" ]]; then task_physical_flags+=("-I$GAMEPKG_PHYSICAL_INCLUDE");fi
 "$task_gcc32" -m32 -std=c11 -O3 -Wall -Wextra -Werror -fno-pie -no-pie "${task_physical_flags[@]}" "$task_tests/near_physical.c" "$task_tests/near_physical.S" -o "$task_work/physical32"
 "$task_work/physical32" "$task_work/corpus.bin"
 "${GAMEPKG_HOST_CC:-cc}" "${task_flags[@]}" -fsanitize=address,undefined -fno-sanitize-recover=all "$task_work/near.c" "${task_sources[@]}" -o "$task_work/host"
 "$task_work/host" "$task_work/corpus.bin"
 "$task_gcc32" -m32 "${task_flags[@]}" -fsanitize=undefined -fsanitize-undefined-trap-on-error "$task_work/near.c" "${task_sources[@]}" -o "$task_work/linux32"
 "$task_work/linux32" "$task_work/corpus.bin"
 "${GAMEPKG_MIPS_CC:-mips-linux-gnu-gcc}" "${task_flags[@]}" -static -DGAMEPKG_REQUIRE_BIG_ENDIAN=1 -fsanitize=undefined -fsanitize-undefined-trap-on-error "$task_work/near.c" "${task_sources[@]}" -o "$task_work/mips32"
 "${GAMEPKG_QEMU_MIPS:-qemu-mips}" "$task_work/mips32" "$task_work/corpus.bin"
 sha256sum "$task_work/corpus.bin"
fi
if [[ "$task_portable" == 0 ]]; then
 task_mingw=${GAMEPKG_MINGW_CC:-i686-w64-mingw32-gcc}
 task_objects=()
 for task_source in "${task_sources[@]}";do
  task_object="$task_work/$(basename "${task_source%.c}").obj"
  "$task_mingw" "${task_flags[@]}" -c "$task_source" -o "$task_object";task_objects+=("$task_object")
 done
 "$task_mingw" "${task_flags[@]}" "$task_work/near.c" "${task_objects[@]}" -o "$task_work/gcc.exe"
 "${GAMEPKG_CLANG:-clang}" --target=i686-w64-windows-gnu --sysroot="${GAMEPKG_MINGW_SYSROOT:-/usr/i686-w64-mingw32}" -femulated-tls "${task_flags[@]}" -c "$task_work/near.c" -o "$task_work/clang.obj"
 "$task_mingw" -pthread "$task_work/clang.obj" "${task_objects[@]}" -o "$task_work/mixed.exe"
 if [[ "$task_link" == 0 ]];then
  export WINEPREFIX=${GAMEPKG_WINEPREFIX:-"$task_work/wineprefix"} WINEDEBUG=-all
  [[ "$WINEPREFIX" == /* ]]
  task_corpus=$(winepath -w "$task_work/corpus.bin")
  for task_profile in gcc mixed; do "${GAMEPKG_WINE:-wine}" "$task_work/$task_profile.exe" "$task_corpus"; done
 fi
fi
printf 'Near-dispatch qualification files: %s\n' "$task_work"
