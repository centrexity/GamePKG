#!/usr/bin/env bash
# Maintainer-only integer full-state replay. No disc, C++ or MPFR at runtime.
set -euo pipefail
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_sf="$task_runtime/softfloat"
task_clang=${GAMEPKG_CLANG:-clang}
task_gcc32=${GAMEPKG_GCC32:-gcc}
task_mingw=${GAMEPKG_MINGW_CC:-i686-w64-mingw32-gcc}
task_sysroot=${GAMEPKG_MINGW_SYSROOT:-/usr/i686-w64-mingw32}
task_link_only=${GAMEPKG_X87_SINCOS_LINK_ONLY:-0}
[[ "$task_link_only" == 0 || "$task_link_only" == 1 ]]
for task_tool in "$task_clang" "$task_mingw"; do command -v "$task_tool" >/dev/null; done
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-x87-sincos-targets.XXXXXXXX")
task_corpus=${GAMEPKG_X87_SINCOS_CORPUS_SOURCE:-}
if [[ "$task_link_only" == 0 && -z "$task_corpus" ]]; then
    task_corpus="$task_work/host.bin"
    GAMEPKG_TAKEDAWN_X87_SINCOS_CORPUS="$task_corpus" cargo test --locked --release \
        --manifest-path "$task_root/engines/takedawn/native-gen/Cargo.toml" --test x87_sincos -- --nocapture
fi
task_sources=("$task_runtime/gamepkg_x86_ext80.c" "$task_runtime/gamepkg_x86_x87.c"
    "$task_runtime/gamepkg_x86_x87_trans.c" "$task_runtime/gamepkg_x86_fault.c"
    "$task_runtime/gamepkg_x86_f128.c" "$task_runtime/transcendentals/gamepkg_fyl2x.c" "$task_runtime/transcendentals/gamepkg_fpatan.c" "$task_runtime/transcendentals/gamepkg_f2xm1.c" "$task_runtime/transcendentals/gamepkg_fsincos.c")
for task_list in sources.txt ext80-sources.txt wide-sources.txt; do
    while IFS= read -r task_source; do
        [[ "$task_source" == source/*.c && "$task_source" != *..* ]]
        task_sources+=("$task_sf/$task_source")
    done < "$task_sf/$task_list"
done
task_flags=(-std=c11 -O3 -Wall -Wextra -Werror
    -I"$task_runtime" -I"$task_sf" -I"$task_sf/source/include" -I"$task_sf/source/8086-SSE")
task_fixture="$task_root/engines/takedawn/native-gen/tests/portable_x87_sincos.c"
if [[ "$task_link_only" == 0 ]]; then
    [[ -f "$task_corpus" && -r "$task_corpus" ]]
    "$task_gcc32" -m32 "${task_flags[@]}" -fsanitize=undefined -fsanitize-undefined-trap-on-error \
        "$task_fixture" "${task_sources[@]}" -o "$task_work/linux32"
    "$task_work/linux32" "$task_corpus"
fi
task_objects=()
for task_source in "${task_sources[@]}"; do
    task_object="$task_work/$(basename "${task_source%.c}").obj"
    "$task_mingw" "${task_flags[@]}" -c "$task_source" -o "$task_object"
    task_objects+=("$task_object")
done
"$task_mingw" "${task_flags[@]}" "$task_fixture" "${task_objects[@]}" -o "$task_work/windows32.exe"
"$task_clang" --target=i686-w64-windows-gnu --sysroot="$task_sysroot" -femulated-tls \
    "${task_flags[@]}" -c "$task_fixture" -o "$task_work/fixture-clang.obj"
"$task_mingw" "${task_flags[@]}" "$task_work/fixture-clang.obj" "${task_objects[@]}" -o "$task_work/mixed32.exe"
if [[ "$task_link_only" == 0 ]]; then
    task_wine=${GAMEPKG_WINE:-wine}
    command -v "$task_wine" >/dev/null
    export WINEPREFIX=${GAMEPKG_WINEPREFIX:-"$task_work/wineprefix"} WINEDEBUG=-all
    [[ "$WINEPREFIX" == /* ]]
    task_windows_corpus=$(winepath -w "$task_corpus")
    for task_profile in windows32 mixed32; do
        "$task_wine" "$task_work/$task_profile.exe" "$task_windows_corpus"
    done
    sha256sum "$task_corpus"
else
    printf 'Windows GCC and Clang/GCC replay fixtures linked; execution skipped.\n'
fi
printf 'Asset-free qualification files: %s\n' "$task_work"
