#!/usr/bin/env bash
# Maintainer-only SSE1 predicate oracle/replay, no game media or host FP casts.
set -euo pipefail
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-predicate.XXXXXXXX")
task_link_only=${GAMEPKG_PREDICATE_LINK_ONLY:-0}
task_portable_only=${GAMEPKG_PREDICATE_PORTABLE_ONLY:-0}
[[ "$task_link_only" == 0 || "$task_link_only" == 1 ]]
[[ "$task_portable_only" == 0 || "$task_portable_only" == 1 ]]
[[ "$task_link_only" != 1 || "$task_portable_only" != 1 ]]
task_corpus=${GAMEPKG_PREDICATE_CORPUS_SOURCE:-}
GAMEPKG_TAKEDAWN_PREDICATE_FIXTURE="$task_work/oracle.c" cargo test --locked --release \
    --manifest-path "$task_root/engines/takedawn/native-gen/Cargo.toml" \
    --test sse_predicate synthetic_software_predicate_fixture_can_be_cross_compiled
if [[ "$task_link_only" == 0 && -z "$task_corpus" ]]; then
    task_corpus="$task_work/corpus.bin"
    GAMEPKG_TAKEDAWN_PREDICATE_CORPUS="$task_corpus" cargo test --locked --release \
        --manifest-path "$task_root/engines/takedawn/native-gen/Cargo.toml" \
        --test sse_predicate software_sse_predicate_matches_hardware
fi
task_flags=(-std=c11 -O3 -Wall -Wextra -Werror -I"$task_runtime")
task_sources=("$task_runtime/gamepkg_x86_fault.c" "$task_runtime/gamepkg_x86_sse.c")
task_replay="$task_root/engines/takedawn/native-gen/tests/portable_predicate.c"
if [[ "$task_link_only" == 0 ]]; then
    [[ -r "$task_corpus" && -f "$task_corpus" ]]
    "${GAMEPKG_GCC32:-gcc}" -m32 "${task_flags[@]}" -fsanitize=undefined \
        -fsanitize-undefined-trap-on-error "$task_replay" "${task_sources[@]}" -o "$task_work/linux32"
    "$task_work/linux32" "$task_corpus"
    "${GAMEPKG_MIPS_CC:-mips-linux-gnu-gcc}" "${task_flags[@]}" -static \
        -DGAMEPKG_REQUIRE_BIG_ENDIAN=1 -fsanitize=undefined -fsanitize-undefined-trap-on-error \
        "$task_replay" "${task_sources[@]}" -o "$task_work/mips32"
    "${GAMEPKG_QEMU_MIPS:-qemu-mips}" "$task_work/mips32" "$task_corpus"
fi
if [[ "$task_portable_only" == 1 ]]; then
    printf 'Predicate portability files: %s\n' "$task_work"
    exit 0
fi
task_mingw=${GAMEPKG_MINGW_CC:-i686-w64-mingw32-gcc}
task_objects=()
for task_source in "${task_sources[@]}"; do
    task_object="$task_work/$(basename "${task_source%.c}").obj"
    "$task_mingw" "${task_flags[@]}" -c "$task_source" -o "$task_object"
    task_objects+=("$task_object")
done
for task_fixture in "$task_replay" "$task_work/oracle.c"; do
    task_name=$(basename "${task_fixture%.c}")
    "$task_mingw" "${task_flags[@]}" -msse "$task_fixture" "${task_objects[@]}" -o "$task_work/$task_name-gcc.exe"
    "${GAMEPKG_CLANG:-clang}" --target=i686-w64-windows-gnu \
        --sysroot="${GAMEPKG_MINGW_SYSROOT:-/usr/i686-w64-mingw32}" -femulated-tls \
        "${task_flags[@]}" -msse -c "$task_fixture" -o "$task_work/$task_name-clang.obj"
    "$task_mingw" "${task_flags[@]}" "$task_work/$task_name-clang.obj" "${task_objects[@]}" -o "$task_work/$task_name-mixed.exe"
done
if [[ "$task_link_only" == 0 ]]; then
    export WINEPREFIX=${GAMEPKG_WINEPREFIX:-"$task_work/wineprefix"} WINEDEBUG=-all
    [[ "$WINEPREFIX" == /* ]]
    task_windows_corpus=$(winepath -w "$task_corpus")
    for task_profile in gcc mixed; do
        "${GAMEPKG_WINE:-wine}" "$task_work/portable_predicate-$task_profile.exe" "$task_windows_corpus"
        "${GAMEPKG_WINE:-wine}" "$task_work/oracle-$task_profile.exe"
    done
    sha256sum "$task_corpus"
fi
printf 'Predicate qualification files: %s\n' "$task_work"
