#!/usr/bin/env bash
# Maintainer-only independent P3 REP prefix/continuation qualification; no media.
set -euo pipefail
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_tests="$task_root/engines/takedawn/native-gen/tests"
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-string-checkpoints.XXXXXXXX")
task_link_only=${GAMEPKG_STRING_CHECKPOINTS_LINK_ONLY:-0}
task_portable_only=${GAMEPKG_STRING_CHECKPOINTS_PORTABLE_ONLY:-0}
[[ "$task_link_only" == 0 || "$task_link_only" == 1 ]]
[[ "$task_portable_only" == 0 || "$task_portable_only" == 1 ]]
[[ "$task_link_only" != 1 || "$task_portable_only" != 1 ]]
GAMEPKG_TAKEDAWN_STRING_VARIANTS="$task_work/variants.inc" cargo test --locked --release \
    --manifest-path "$task_root/engines/takedawn/native-gen/Cargo.toml" --test strings export_string_variants
task_flags=(-std=c11 -O3 -Wall -Wextra -Werror -pthread -I"$task_runtime")
task_sources=("$task_runtime/gamepkg_x86_fault.c" "$task_runtime/gamepkg_x86_memory.c" "$task_runtime/gamepkg_x86_string.c")
task_replay="$task_tests/portable_string_checkpoints.c"
if [[ "$task_link_only" == 0 ]]; then
    task_gcc32=${GAMEPKG_GCC32:-gcc}
    "$task_gcc32" -m32 -c "$task_tests/string_ring0/start.S" -o "$task_work/start.o"
    for task_kind in prefix full; do
        if [[ "$task_kind" == prefix ]]; then task_kernel=string_prefix_ring0; else task_kernel=string_ring0; fi
        "$task_gcc32" -m32 -std=c11 -O2 -Wall -Wextra -Werror -ffreestanding -fno-pie \
            -fno-stack-protector -fno-asynchronous-unwind-tables -I"$task_work" -c "$task_tests/$task_kernel/kernel.c" -o "$task_work/$task_kind.o"
        ld -m elf_i386 -T "$task_tests/string_ring0/link.ld" "$task_work/start.o" "$task_work/$task_kind.o" -o "$task_work/$task_kind.elf"
        if timeout 90 "${GAMEPKG_QEMU_SYSTEM_I386:-qemu-system-i386}" -accel tcg -cpu pentium3 -m 16M \
            -nodefaults -display none -monitor none -serial none -no-reboot -kernel "$task_work/$task_kind.elf" \
            -debugcon "file:$task_work/$task_kind.bin" -device isa-debug-exit,iobase=0xf4,iosize=4; then task_code=0; else task_code=$?; fi
        [[ "$task_code" == 1 && -s "$task_work/$task_kind.bin" ]]
    done
    "${GAMEPKG_HOST_CC:-cc}" "${task_flags[@]}" -fsanitize=address,undefined -fno-sanitize-recover=all "$task_replay" "${task_sources[@]}" -o "$task_work/host"
    "$task_work/host" "$task_work/prefix.bin" "$task_work/full.bin"
    "$task_gcc32" -m32 "${task_flags[@]}" -fsanitize=undefined -fsanitize-undefined-trap-on-error \
        "$task_replay" "${task_sources[@]}" -o "$task_work/linux32"
    "$task_work/linux32" "$task_work/prefix.bin" "$task_work/full.bin"
    "${GAMEPKG_MIPS_CC:-mips-linux-gnu-gcc}" "${task_flags[@]}" -static -DGAMEPKG_REQUIRE_BIG_ENDIAN=1 \
        -fsanitize=undefined -fsanitize-undefined-trap-on-error "$task_replay" "${task_sources[@]}" -o "$task_work/mips32"
    "${GAMEPKG_QEMU_MIPS:-qemu-mips}" "$task_work/mips32" "$task_work/prefix.bin" "$task_work/full.bin"
    task_physical_flags=()
    if [[ -n "${GAMEPKG_PHYSICAL_INCLUDE:-}" ]]; then task_physical_flags=(-isystem "$GAMEPKG_PHYSICAL_INCLUDE"); fi
    "$task_gcc32" -m32 "${task_physical_flags[@]}" -fno-pie -no-pie -std=c11 -O3 -Wall -Wextra -Werror \
        "$task_tests/string_physical_interrupt.c" -o "$task_work/physical32"
    timeout 90 "$task_work/physical32"
    sha256sum "$task_work/prefix.bin" "$task_work/full.bin"
fi
if [[ "$task_portable_only" == 1 ]]; then
    printf 'REP checkpoint portability files: %s\n' "$task_work"
    exit 0
fi
task_mingw=${GAMEPKG_MINGW_CC:-i686-w64-mingw32-gcc}
task_objects=()
for task_source in "${task_sources[@]}"; do
    task_object="$task_work/$(basename "${task_source%.c}").obj"
    "$task_mingw" "${task_flags[@]}" -c "$task_source" -o "$task_object"
    task_objects+=("$task_object")
done
"$task_mingw" "${task_flags[@]}" "$task_replay" "${task_objects[@]}" -o "$task_work/replay-gcc.exe"
"${GAMEPKG_CLANG:-clang}" --target=i686-w64-windows-gnu --sysroot="${GAMEPKG_MINGW_SYSROOT:-/usr/i686-w64-mingw32}" \
    -femulated-tls "${task_flags[@]}" -c "$task_replay" -o "$task_work/replay-clang.obj"
"$task_mingw" "${task_flags[@]}" "$task_work/replay-clang.obj" "${task_objects[@]}" -o "$task_work/replay-mixed.exe"
if [[ "$task_link_only" == 0 ]]; then
    export WINEPREFIX=${GAMEPKG_WINEPREFIX:-"$task_work/wineprefix"} WINEDEBUG=-all
    [[ "$WINEPREFIX" == /* ]]
    task_win_prefix=$(winepath -w "$task_work/prefix.bin")
    task_win_full=$(winepath -w "$task_work/full.bin")
    for task_profile in gcc mixed; do
        "${GAMEPKG_WINE:-wine}" "$task_work/replay-$task_profile.exe" "$task_win_prefix" "$task_win_full"
    done
fi
printf 'REP checkpoint qualification files: %s\n' "$task_work"
