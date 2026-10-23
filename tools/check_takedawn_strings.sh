#!/usr/bin/env bash
# Maintainer-only string partial-progress and protected-mode instruction oracle.
set -euo pipefail
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_tests="$task_root/engines/takedawn/native-gen/tests"
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-strings.XXXXXXXX")
task_link_only=${GAMEPKG_STRINGS_LINK_ONLY:-0}
task_portable_only=${GAMEPKG_STRINGS_PORTABLE_ONLY:-0}
[[ "$task_link_only" == 0 || "$task_link_only" == 1 ]]
[[ "$task_portable_only" == 0 || "$task_portable_only" == 1 ]]
[[ "$task_link_only" != 1 || "$task_portable_only" != 1 ]]
GAMEPKG_TAKEDAWN_STRING_FIXTURE="$task_work/strings.c" GAMEPKG_TAKEDAWN_STRING_VARIANTS="$task_work/variants.inc" cargo test --locked --release \
    --manifest-path "$task_root/engines/takedawn/native-gen/Cargo.toml" --test strings export_string
task_flags=(-std=c11 -O3 -Wall -Wextra -Werror -pthread -I"$task_runtime")
task_sources=("$task_runtime/gamepkg_x86_fault.c" "$task_runtime/gamepkg_x86_memory.c" "$task_runtime/gamepkg_x86_string.c")
task_replay="$task_tests/portable_strings.c"
if [[ "$task_link_only" == 0 ]]; then
    task_gcc32=${GAMEPKG_GCC32:-gcc}
    "$task_gcc32" -m32 -std=c11 -O2 -Wall -Wextra -Werror -ffreestanding -fno-pie \
        -fno-stack-protector -fno-asynchronous-unwind-tables -I"$task_work" -c "$task_tests/string_ring0/kernel.c" -o "$task_work/kernel.o"
    "$task_gcc32" -m32 -c "$task_tests/string_ring0/start.S" -o "$task_work/start.o"
    ld -m elf_i386 -T "$task_tests/string_ring0/link.ld" "$task_work/start.o" "$task_work/kernel.o" -o "$task_work/kernel.elf"
    task_corpus="$task_work/corpus.bin"
    if timeout 90 "${GAMEPKG_QEMU_SYSTEM_I386:-qemu-system-i386}" -accel tcg -cpu pentium3 -m 16M \
        -nodefaults -display none -monitor none -serial none -no-reboot -kernel "$task_work/kernel.elf" \
        -debugcon "file:$task_corpus" -device isa-debug-exit,iobase=0xf4,iosize=4; then task_code=0; else task_code=$?; fi
    [[ "$task_code" == 1 && -s "$task_corpus" ]]
    "${GAMEPKG_HOST_CC:-cc}" "${task_flags[@]}" -fsanitize=undefined -fno-sanitize-recover=all "$task_replay" "${task_sources[@]}" -o "$task_work/replay64"
    "$task_work/replay64" "$task_corpus"
    for task_fixture in "$task_replay" "$task_work/strings.c"; do
        task_name=$(basename "${task_fixture%.c}")
        "$task_gcc32" -m32 "${task_flags[@]}" -fsanitize=undefined -fsanitize-undefined-trap-on-error \
            "$task_fixture" "${task_sources[@]}" -o "$task_work/$task_name-32"
        "$task_work/$task_name-32" "$task_corpus"
        "${GAMEPKG_MIPS_CC:-mips-linux-gnu-gcc}" "${task_flags[@]}" -static -DGAMEPKG_REQUIRE_BIG_ENDIAN=1 \
            -fsanitize=undefined -fsanitize-undefined-trap-on-error "$task_fixture" "${task_sources[@]}" -o "$task_work/$task_name-mips"
        "${GAMEPKG_QEMU_MIPS:-qemu-mips}" "$task_work/$task_name-mips" "$task_corpus"
    done
    "$task_gcc32" -m32 -std=c11 -O3 -Wall -Wextra -Werror "$task_tests/string_physical_fault.c" -o "$task_work/physical32"
    "$task_work/physical32"
    sha256sum "$task_corpus"
fi
if [[ "$task_portable_only" == 1 ]]; then
    printf 'String portability qualification files: %s\n' "$task_work"
    exit 0
fi
task_mingw=${GAMEPKG_MINGW_CC:-i686-w64-mingw32-gcc}
task_objects=()
for task_source in "${task_sources[@]}"; do
    task_object="$task_work/$(basename "${task_source%.c}").obj"
    "$task_mingw" "${task_flags[@]}" -c "$task_source" -o "$task_object"
    task_objects+=("$task_object")
done
for task_fixture in "$task_replay" "$task_work/strings.c"; do
    task_name=$(basename "${task_fixture%.c}")
    "$task_mingw" "${task_flags[@]}" "$task_fixture" "${task_objects[@]}" -o "$task_work/$task_name-gcc.exe"
    "${GAMEPKG_CLANG:-clang}" --target=i686-w64-windows-gnu --sysroot="${GAMEPKG_MINGW_SYSROOT:-/usr/i686-w64-mingw32}" \
        -femulated-tls "${task_flags[@]}" -c "$task_fixture" -o "$task_work/$task_name-clang.obj"
    "$task_mingw" "${task_flags[@]}" "$task_work/$task_name-clang.obj" "${task_objects[@]}" -o "$task_work/$task_name-mixed.exe"
done
if [[ "$task_link_only" == 0 ]]; then
    export WINEPREFIX=${GAMEPKG_WINEPREFIX:-"$task_work/wineprefix"} WINEDEBUG=-all
    [[ "$WINEPREFIX" == /* ]]
    task_win_corpus=$(winepath -w "$task_corpus")
    for task_profile in gcc mixed; do
        "${GAMEPKG_WINE:-wine}" "$task_work/portable_strings-$task_profile.exe" "$task_win_corpus"
        "${GAMEPKG_WINE:-wine}" "$task_work/strings-$task_profile.exe"
    done
fi
printf 'String qualification files: %s\n' "$task_work"
