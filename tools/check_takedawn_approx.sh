#!/usr/bin/env bash
# Maintainer-only reciprocal/rsqrt approximation, fault and width/endian qualification.
set -euo pipefail
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_tests="$task_root/engines/takedawn/native-gen/tests"
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-approx.XXXXXXXX")
task_link_only=${GAMEPKG_APPROX_LINK_ONLY:-0}
task_portable_only=${GAMEPKG_APPROX_PORTABLE_ONLY:-0}
[[ "$task_link_only" == 0 || "$task_link_only" == 1 ]]
[[ "$task_portable_only" == 0 || "$task_portable_only" == 1 ]]
[[ "$task_link_only" != 1 || "$task_portable_only" != 1 ]]
GAMEPKG_TAKEDAWN_APPROX_FIXTURE="$task_work/approx.c" GAMEPKG_TAKEDAWN_APPROX_REFERENCE="$task_work/original.cc" GAMEPKG_TAKEDAWN_APPROX_CORPUS="$task_work/corpus.bin" \
    cargo test --locked --release --manifest-path "$task_root/engines/takedawn/native-gen/Cargo.toml" --test approx -- --nocapture
task_flags=(-std=c11 -O3 -Wall -Wextra -Werror -pthread -I"$task_runtime")
task_sources=("$task_runtime/gamepkg_x86_fault.c" "$task_runtime/gamepkg_x86_approx.c" "$task_runtime/approx/gamepkg_approx.c")
if [[ "$task_link_only" == 0 ]]; then
    task_gcc32=${GAMEPKG_GCC32:-gcc}
    task_corpus="$task_work/corpus.bin"
    [[ -s "$task_corpus" ]]
    "${GAMEPKG_HOST_CC:-cc}" "${task_flags[@]}" -fsanitize=undefined -fno-sanitize-recover=all "$task_work/approx.c" "${task_sources[@]}" -o "$task_work/approx64"
    "$task_work/approx64" "$task_corpus"
    "$task_work/approx64"
    "$task_gcc32" -m32 "${task_flags[@]}" -fsanitize=undefined -fsanitize-undefined-trap-on-error \
        "$task_work/approx.c" "${task_sources[@]}" -o "$task_work/approx32"
    "$task_work/approx32" "$task_corpus"
    "$task_work/approx32"
    "${GAMEPKG_MIPS_CC:-mips-linux-gnu-gcc}" "${task_flags[@]}" -static -DGAMEPKG_REQUIRE_BIG_ENDIAN=1 \
        -fsanitize=undefined -fsanitize-undefined-trap-on-error "$task_work/approx.c" "${task_sources[@]}" -o "$task_work/approx-mips"
    "${GAMEPKG_QEMU_MIPS:-qemu-mips}" "$task_work/approx-mips" "$task_corpus"
    "${GAMEPKG_QEMU_MIPS:-qemu-mips}" "$task_work/approx-mips"
    sha256sum "$task_corpus"
fi
if [[ "$task_portable_only" == 1 ]]; then
    printf 'Approximation portability qualification files: %s\n' "$task_work"
    exit 0
fi
task_mingw=${GAMEPKG_MINGW_CC:-i686-w64-mingw32-gcc}
task_objects=()
for task_source in "${task_sources[@]}"; do
    task_object="$task_work/$(basename "${task_source%.c}").obj"
    "$task_mingw" "${task_flags[@]}" -c "$task_source" -o "$task_object"
    task_objects+=("$task_object")
done
"$task_mingw" "${task_flags[@]}" "$task_work/approx.c" "${task_objects[@]}" -o "$task_work/approx-gcc.exe"
"${GAMEPKG_CLANG:-clang}" --target=i686-w64-windows-gnu --sysroot="${GAMEPKG_MINGW_SYSROOT:-/usr/i686-w64-mingw32}" \
    -femulated-tls "${task_flags[@]}" -c "$task_work/approx.c" -o "$task_work/approx-clang.obj"
"$task_mingw" "${task_flags[@]}" "$task_work/approx-clang.obj" "${task_objects[@]}" -o "$task_work/approx-mixed.exe"
if [[ "$task_link_only" == 0 ]]; then
    export WINEPREFIX=${GAMEPKG_WINEPREFIX:-"$task_work/wineprefix"} WINEDEBUG=-all
    [[ "$WINEPREFIX" == /* ]]
    task_win_corpus=$(winepath -w "$task_corpus")
    for task_profile in gcc mixed; do
        "${GAMEPKG_WINE:-wine}" "$task_work/approx-$task_profile.exe" "$task_win_corpus"
        "${GAMEPKG_WINE:-wine}" "$task_work/approx-$task_profile.exe"
    done
fi
printf 'Approximation qualification files: %s\n' "$task_work"
