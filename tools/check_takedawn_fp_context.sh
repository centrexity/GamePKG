#!/usr/bin/env bash
# Asset-free host-owned fault boundary ABI qualification.
set -euo pipefail
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-fp-context.XXXXXXXX")
task_link_only=${GAMEPKG_FP_CONTEXT_LINK_ONLY:-0}
task_portable_only=${GAMEPKG_FP_CONTEXT_PORTABLE_ONLY:-0}
[[ "$task_link_only" == 0 || "$task_link_only" == 1 ]]
[[ "$task_portable_only" == 0 || "$task_portable_only" == 1 ]]
[[ "$task_link_only" != 1 || "$task_portable_only" != 1 ]]
GAMEPKG_TAKEDAWN_FP_CONTEXT_FIXTURE="$task_work/context.c" cargo test --locked --release \
    --manifest-path "$task_root/engines/takedawn/native-gen/Cargo.toml" \
    --test fp_context floating_fault_fixture_can_be_cross_compiled
[[ -s "$task_work/context.c" ]]
task_flags=(-std=c11 -O3 -Wall -Wextra -Werror -I"$task_runtime")
task_sources=("$task_runtime/gamepkg_x86_fault.c" "$task_runtime/gamepkg_x86_x87.c")
if [[ "$task_link_only" == 0 ]]; then
    "${GAMEPKG_GCC32:-gcc}" -m32 "${task_flags[@]}" -pthread -fsanitize=undefined \
        -fsanitize-undefined-trap-on-error "$task_work/context.c" "${task_sources[@]}" -o "$task_work/linux32"
    "$task_work/linux32"
    "${GAMEPKG_MIPS_CC:-mips-linux-gnu-gcc}" "${task_flags[@]}" -pthread -static \
        -fsanitize=undefined -fsanitize-undefined-trap-on-error \
        "$task_work/context.c" "${task_sources[@]}" -o "$task_work/mips32"
    "${GAMEPKG_QEMU_MIPS:-qemu-mips}" "$task_work/mips32"
fi
if [[ "$task_portable_only" == 1 ]]; then
    printf 'Fault context portability files: %s\n' "$task_work"
    exit 0
fi
task_mingw=${GAMEPKG_MINGW_CC:-i686-w64-mingw32-gcc}
task_objects=()
for task_source in "${task_sources[@]}"; do
    task_object="$task_work/$(basename "${task_source%.c}").obj"
    "$task_mingw" "${task_flags[@]}" -c "$task_source" -o "$task_object"
    task_objects+=("$task_object")
done
"$task_mingw" "${task_flags[@]}" "$task_work/context.c" "${task_objects[@]}" -o "$task_work/windows32.exe"
"${GAMEPKG_CLANG:-clang}" --target=i686-w64-windows-gnu \
    --sysroot="${GAMEPKG_MINGW_SYSROOT:-/usr/i686-w64-mingw32}" -femulated-tls \
    "${task_flags[@]}" -c "$task_work/context.c" -o "$task_work/context-clang.obj"
"$task_mingw" "${task_flags[@]}" "$task_work/context-clang.obj" "${task_objects[@]}" -o "$task_work/mixed32.exe"
if [[ "$task_link_only" == 0 ]]; then
    export WINEPREFIX=${GAMEPKG_WINEPREFIX:-"$task_work/wineprefix"} WINEDEBUG=-all
    [[ "$WINEPREFIX" == /* ]]
    for task_profile in windows32 mixed32; do
        "${GAMEPKG_WINE:-wine}" "$task_work/$task_profile.exe"
    done
fi
printf 'Fault context qualification files: %s\n' "$task_work"
