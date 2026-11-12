#!/usr/bin/env bash
# Maintainer-only synthetic instruction tests; no game media or player tooling.
set -euo pipefail
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_sf="$task_runtime/softfloat"
task_clang=${GAMEPKG_CLANG:-clang}
task_mingw=${GAMEPKG_MINGW_CC:-i686-w64-mingw32-gcc}
task_sysroot=${GAMEPKG_MINGW_SYSROOT:-/usr/i686-w64-mingw32}
task_link_only=${GAMEPKG_X87_COMPARE_LINK_ONLY:-0}
[[ "$task_link_only" == 0 || "$task_link_only" == 1 ]]
for task_tool in "$task_clang" "$task_mingw"; do command -v "$task_tool" >/dev/null; done
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-x87-compare-targets.XXXXXXXX")
task_fixture=${GAMEPKG_X87_COMPARE_FIXTURE_SOURCE:-}
task_corpus=${GAMEPKG_X87_COMPARE_CORPUS_SOURCE:-}
if [[ -z "$task_fixture" ]]; then
    task_fixture="$task_work/fixture.c"
    if [[ "$task_link_only" == 1 ]]; then
        GAMEPKG_TAKEDAWN_X87_COMPARE_FIXTURE="$task_fixture" cargo test --locked --release \
            --manifest-path "$task_root/engines/takedawn/native-gen/Cargo.toml" --test x87_compare export_comparison_fixture
    else
        task_corpus="$task_work/host.bin"
        GAMEPKG_TAKEDAWN_X87_COMPARE_FIXTURE="$task_fixture" GAMEPKG_TAKEDAWN_X87_COMPARE_CORPUS="$task_corpus" \
            cargo test --locked --release --manifest-path "$task_root/engines/takedawn/native-gen/Cargo.toml" --test x87_compare -- --nocapture
    fi
fi
[[ -f "$task_fixture" && -r "$task_fixture" ]]
task_sources=("$task_runtime/gamepkg_x86_x87.c"
    "$task_runtime/gamepkg_x86_x87_compare.c" "$task_runtime/gamepkg_x86_memory.c" "$task_runtime/gamepkg_x86_fault.c")
task_flags=(-std=c11 -O3 -msse2 -mfpmath=sse -Wall -Wextra -Werror
    -I"$task_runtime" -I"$task_sf" -I"$task_sf/source/include" -I"$task_sf/source/8086-SSE")
if [[ "$task_link_only" == 0 ]]; then
    "$task_clang" -m32 "${task_flags[@]}" -pthread -fsanitize=undefined -fsanitize-undefined-trap-on-error \
        "$task_fixture" "${task_sources[@]}" -o "$task_work/linux32"
    GAMEPKG_TAKEDAWN_X87_COMPARE_CORPUS="$task_work/linux32.bin" "$task_work/linux32"
    if [[ -n "$task_corpus" ]]; then cmp "$task_corpus" "$task_work/linux32.bin"; fi
    task_corpus="$task_work/linux32.bin"
fi
task_objects=()
for task_source in "${task_sources[@]}"; do
    task_object="$task_work/$(basename "${task_source%.c}").obj"
    "$task_mingw" "${task_flags[@]}" -c "$task_source" -o "$task_object"
    task_objects+=("$task_object")
done
"$task_mingw" "${task_flags[@]}" "$task_fixture" "${task_objects[@]}" -o "$task_work/windows32.exe"
# GCC's MinGW TLS is emulated. Clang must use the same model when sharing
# architectural state with these helpers; a partial object link is insufficient.
"$task_clang" --target=i686-w64-windows-gnu --sysroot="$task_sysroot" -femulated-tls \
    "${task_flags[@]}" -c "$task_fixture" -o "$task_work/fixture-clang.obj"
"$task_mingw" "${task_flags[@]}" "$task_work/fixture-clang.obj" "${task_objects[@]}" -o "$task_work/mixed32.exe"
if [[ "$task_link_only" == 0 ]]; then
    task_wine=${GAMEPKG_WINE:-wine}
    command -v "$task_wine" >/dev/null
    export WINEPREFIX=${GAMEPKG_WINEPREFIX:-"$task_work/wineprefix"} WINEDEBUG=-all
    [[ "$WINEPREFIX" == /* ]]
    for task_profile in windows32 mixed32; do
        task_windows_corpus=$(winepath -w "$task_work/$task_profile.bin")
        GAMEPKG_TAKEDAWN_X87_COMPARE_CORPUS="$task_windows_corpus" "$task_wine" "$task_work/$task_profile.exe"
        cmp "$task_corpus" "$task_work/$task_profile.bin"
    done
    sha256sum "$task_corpus" "$task_work/windows32.bin" "$task_work/mixed32.bin"
else
    printf 'Windows GCC and Clang/GCC fixtures compiled and linked; execution skipped.\n'
fi
printf 'Asset-free qualification files: %s\n' "$task_work"
