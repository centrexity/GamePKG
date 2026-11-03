#!/usr/bin/env bash
# Maintainer transactional event/timer initialization qualification.
set -euo pipefail
ulimit -c 0
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_tests="$task_root/engines/takedawn/native-gen/tests"
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-dispatcher_object.XXXXXXXX")
task_link_only=${GAMEPKG_DISPATCHER_OBJECT_LINK_ONLY:-0}
task_portable_only=${GAMEPKG_DISPATCHER_OBJECT_PORTABLE_ONLY:-0}
[[ "$task_link_only" == 0 || "$task_link_only" == 1 ]]
[[ "$task_portable_only" == 0 || "$task_portable_only" == 1 ]]
[[ "$task_link_only" != 1 || "$task_portable_only" != 1 ]]
GAMEPKG_DISPATCHER_OBJECT_FIXTURE="$task_work/dispatch.c" cargo test --locked --release \
    --manifest-path "$task_root/engines/takedawn/native-gen/Cargo.toml" --test dispatcher_object export_dispatcher_object_fixture
task_flags=(-std=c11 -O3 -Wall -Wextra -Werror -pthread -I"$task_runtime")
task_sources=("$task_runtime/gamepkg_x86_dispatcher_object.c" "$task_runtime/gamepkg_x86_irql.c" "$task_runtime/gamepkg_x86_fault.c" "$task_runtime/gamepkg_x86_memory.c" "$task_runtime/gamepkg_x86_dispatch.c" "$task_runtime/gamepkg_x86_cpu.c" "$task_runtime/gamepkg_x86_interrupt.c" "$task_runtime/gamepkg_x86_cpu_dispatch.c" "$task_runtime/gamepkg_x86_table_store.c" "$task_runtime/gamepkg_x86_far_memory.c" "$task_runtime/gamepkg_x86_far_jump.c" "$task_runtime/gamepkg_x86_x87.c")
if [[ "$task_link_only" == 0 ]]; then
    "${GAMEPKG_HOST_CC:-cc}" "${task_flags[@]}" -fsanitize=address,undefined -fno-sanitize-recover=all \
        "$task_work/dispatch.c" "${task_sources[@]}" -o "$task_work/dispatch64"
    "$task_work/dispatch64"
    task_gcc32=${GAMEPKG_GCC32:-gcc}
    "$task_gcc32" -m32 -fno-pie -no-pie "${task_flags[@]}" \
        -fsanitize=undefined -fsanitize-undefined-trap-on-error "$task_work/dispatch.c" "${task_sources[@]}" -o "$task_work/dispatch32"
    "$task_work/dispatch32"
    "${GAMEPKG_MIPS_CC:-mips-linux-gnu-gcc}" "${task_flags[@]}" -static -DGAMEPKG_REQUIRE_BIG_ENDIAN=1 \
        -fsanitize=undefined -fsanitize-undefined-trap-on-error "$task_work/dispatch.c" "${task_sources[@]}" -o "$task_work/dispatch-mips"
    "${GAMEPKG_QEMU_MIPS:-qemu-mips}" "$task_work/dispatch-mips"
fi
if [[ "$task_portable_only" == 1 ]]; then
    printf 'Dispatcher object portability qualification files: %s\n' "$task_work"
    exit 0
fi
task_mingw=${GAMEPKG_MINGW_CC:-i686-w64-mingw32-gcc}
task_objects=()
for task_source in "${task_sources[@]}"; do
    task_object="$task_work/$(basename "${task_source%.c}").obj"
    "$task_mingw" "${task_flags[@]}" -c "$task_source" -o "$task_object"
    task_objects+=("$task_object")
done
"$task_mingw" "${task_flags[@]}" "$task_work/dispatch.c" "${task_objects[@]}" -o "$task_work/dispatch-gcc.exe"
"${GAMEPKG_CLANG:-clang}" --target=i686-w64-windows-gnu --sysroot="${GAMEPKG_MINGW_SYSROOT:-/usr/i686-w64-mingw32}" \
    -femulated-tls "${task_flags[@]}" -c "$task_work/dispatch.c" -o "$task_work/dispatch-clang.obj"
"$task_mingw" "${task_flags[@]}" "$task_work/dispatch-clang.obj" "${task_objects[@]}" -o "$task_work/dispatch-mixed.exe"
if [[ "$task_link_only" == 0 ]]; then
    export WINEPREFIX=${GAMEPKG_WINEPREFIX:-"$task_work/wineprefix"} WINEDEBUG=-all
    [[ "$WINEPREFIX" == /* ]]
    for task_profile in gcc mixed; do
        "${GAMEPKG_WINE:-wine}" "$task_work/dispatch-$task_profile.exe"
    done
fi
printf 'Dispatcher object qualification files: %s\n' "$task_work"
