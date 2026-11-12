#!/usr/bin/env bash
# Maintainer-only virtual IRQ instruction-state and independent CPU/PIT checks.
set -euo pipefail
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_tests="$task_root/engines/takedawn/native-gen/tests"
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-interrupts.XXXXXXXX")
task_link_only=${GAMEPKG_INTERRUPTS_LINK_ONLY:-0}
task_portable_only=${GAMEPKG_INTERRUPTS_PORTABLE_ONLY:-0}
[[ "$task_link_only" == 0 || "$task_link_only" == 1 ]]
[[ "$task_portable_only" == 0 || "$task_portable_only" == 1 ]]
[[ "$task_link_only" != 1 || "$task_portable_only" != 1 ]]
task_flags=(-std=c11 -O3 -Wall -Wextra -Werror -I"$task_runtime")
task_source="$task_runtime/gamepkg_x86_interrupt.c"
task_fixture="$task_tests/interrupt_state.c"
task_corpus="$task_work/corpus.bin"
if [[ "$task_link_only" == 0 ]]; then
    task_gcc32=${GAMEPKG_GCC32:-gcc}
    "$task_gcc32" -m32 -std=c11 -O2 -Wall -Wextra -Werror -ffreestanding -fno-pie \
        -fno-stack-protector -fno-asynchronous-unwind-tables -c "$task_tests/interrupt_ring0/kernel.c" -o "$task_work/kernel.o"
    "$task_gcc32" -m32 -c "$task_tests/flags_ring0/start.S" -o "$task_work/start.o"
    ld -m elf_i386 -T "$task_tests/flags_ring0/link.ld" "$task_work/start.o" "$task_work/kernel.o" -o "$task_work/kernel.elf"
    if timeout 30 "${GAMEPKG_QEMU_SYSTEM_I386:-qemu-system-i386}" -accel tcg -cpu pentium3 -m 16M \
        -nodefaults -display none -monitor none -serial none -no-reboot -kernel "$task_work/kernel.elf" \
        -debugcon "file:$task_corpus" -device isa-debug-exit,iobase=0xf4,iosize=4; then task_code=0; else task_code=$?; fi
    [[ "$task_code" == 1 && -s "$task_corpus" ]]
    "${GAMEPKG_HOST_CC:-cc}" "${task_flags[@]}" -fsanitize=address,undefined -fno-sanitize-recover=all \
        "$task_fixture" "$task_source" -o "$task_work/host"
    "$task_work/host" "$task_corpus"
    "$task_gcc32" -m32 "${task_flags[@]}" -fsanitize=undefined -fsanitize-undefined-trap-on-error \
        "$task_fixture" "$task_source" -o "$task_work/linux32"
    "$task_work/linux32" "$task_corpus"
    "$task_gcc32" -m32 -fno-pie -no-pie -std=c11 -O3 -Wall -Wextra -Werror \
        "$task_tests/interrupt_privilege.c" -o "$task_work/privilege32"
    "$task_work/privilege32"
    "${GAMEPKG_MIPS_CC:-mips-linux-gnu-gcc}" "${task_flags[@]}" -static -DGAMEPKG_REQUIRE_BIG_ENDIAN=1 \
        -fsanitize=undefined -fsanitize-undefined-trap-on-error "$task_fixture" "$task_source" -o "$task_work/mips32"
    "${GAMEPKG_QEMU_MIPS:-qemu-mips}" "$task_work/mips32" "$task_corpus"
    sha256sum "$task_corpus"
fi
if [[ "$task_portable_only" == 1 ]]; then
    printf 'Interrupt-state portability files: %s\n' "$task_work"
    exit 0
fi
task_mingw=${GAMEPKG_MINGW_CC:-i686-w64-mingw32-gcc}
"$task_mingw" "${task_flags[@]}" -c "$task_source" -o "$task_work/interrupt.obj"
"$task_mingw" "${task_flags[@]}" "$task_fixture" "$task_work/interrupt.obj" -o "$task_work/state-gcc.exe"
"${GAMEPKG_CLANG:-clang}" --target=i686-w64-windows-gnu --sysroot="${GAMEPKG_MINGW_SYSROOT:-/usr/i686-w64-mingw32}" \
    "${task_flags[@]}" -c "$task_fixture" -o "$task_work/fixture-clang.obj"
"$task_mingw" "$task_work/fixture-clang.obj" "$task_work/interrupt.obj" -o "$task_work/state-mixed.exe"
if [[ "$task_link_only" == 0 ]]; then
    export WINEPREFIX=${GAMEPKG_WINEPREFIX:-"$task_work/wineprefix"} WINEDEBUG=-all
    [[ "$WINEPREFIX" == /* ]]
    task_win_corpus=$(winepath -w "$task_corpus")
    for task_profile in gcc mixed; do
        "${GAMEPKG_WINE:-wine}" "$task_work/state-$task_profile.exe" "$task_win_corpus"
    done
fi
printf 'Interrupt-state qualification files: %s\n' "$task_work"
