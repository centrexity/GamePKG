#!/usr/bin/env bash
# Independent GCC compiler reference on ELF32; integer-only replay on Windows.
set -euo pipefail
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_sf="$task_runtime/softfloat"
task_mingw=${GAMEPKG_MINGW_CC:-i686-w64-mingw32-gcc}
task_clang=${GAMEPKG_CLANG:-clang}
task_sysroot=${GAMEPKG_MINGW_SYSROOT:-/usr/i686-w64-mingw32}
task_link_only=${GAMEPKG_F128_LINK_ONLY:-0}
[[ "$task_link_only" == 0 || "$task_link_only" == 1 ]]
for task_tool in "$task_clang" "$task_mingw"; do command -v "$task_tool" >/dev/null; done
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-f128-targets.XXXXXXXX")
task_fixture=${GAMEPKG_F128_FIXTURE_SOURCE:-}
task_corpus=${GAMEPKG_F128_CORPUS_SOURCE:-}
if [[ -z "$task_fixture" && "$task_link_only" == 0 ]]; then
    task_fixture="$task_work/fixture.c"
    task_corpus="$task_work/reference.bin"
    GAMEPKG_TAKEDAWN_F128_FIXTURE="$task_fixture" GAMEPKG_TAKEDAWN_F128_CORPUS="$task_corpus" \
        cargo test --locked --release --manifest-path "$task_root/engines/takedawn/native-gen/Cargo.toml" --test f128_math -- --nocapture
fi
task_sources=("$task_runtime/gamepkg_x86_f128.c" "$task_runtime/gamepkg_x86_ext80.c")
for task_list in sources.txt ext80-sources.txt wide-sources.txt; do
    while IFS= read -r task_source; do
        [[ "$task_source" == source/*.c && "$task_source" != *..* ]]
        task_sources+=("$task_sf/$task_source")
    done < "$task_sf/$task_list"
done
task_flags=(-std=c11 -O3 -Wall -Wextra -Werror -I"$task_runtime" -I"$task_sf"
    -I"$task_sf/source/include" -I"$task_sf/source/8086-SSE")
if [[ "$task_link_only" == 0 ]]; then
    [[ -f "$task_fixture" && -r "$task_fixture" ]]
    # GCC supplies the independent __float128 reference. Production/portable
    # sources have no compiler floating type or libquadmath dependency.
    "${GAMEPKG_GCC32:-gcc}" -m32 "${task_flags[@]}" -msse2 -mfpmath=sse -frounding-math -pthread \
        -fsanitize=undefined -fsanitize-undefined-trap-on-error "$task_fixture" "${task_sources[@]}" -lm -o "$task_work/linux32"
    GAMEPKG_TAKEDAWN_F128_CORPUS="$task_work/linux32.bin" "$task_work/linux32"
    if [[ -n "$task_corpus" ]]; then cmp "$task_corpus" "$task_work/linux32.bin"; fi
    task_corpus="$task_work/linux32.bin"
fi
task_objects=()
for task_source in "${task_sources[@]}"; do
    task_object="$task_work/$(basename "${task_source%.c}").obj"
    "$task_mingw" "${task_flags[@]}" -c "$task_source" -o "$task_object"
    task_objects+=("$task_object")
done
task_replay="$task_root/engines/takedawn/native-gen/tests/portable_f128.c"
"$task_mingw" "${task_flags[@]}" "$task_replay" "${task_objects[@]}" -o "$task_work/windows32.exe"
"$task_clang" --target=i686-w64-windows-gnu --sysroot="$task_sysroot" -femulated-tls \
    "${task_flags[@]}" -c "$task_replay" -o "$task_work/replay-clang.obj"
"$task_mingw" "${task_flags[@]}" "$task_work/replay-clang.obj" "${task_objects[@]}" -o "$task_work/mixed32.exe"
if [[ "$task_link_only" == 0 ]]; then
    export WINEPREFIX=${GAMEPKG_WINEPREFIX:-"$task_work/wineprefix"} WINEDEBUG=-all
    [[ "$WINEPREFIX" == /* ]]
    task_windows_corpus=$(winepath -w "$task_corpus")
    "${GAMEPKG_WINE:-wine}" "$task_work/windows32.exe" "$task_windows_corpus"
    "${GAMEPKG_WINE:-wine}" "$task_work/mixed32.exe" "$task_windows_corpus"
    sha256sum "$task_corpus"
else
    printf 'Windows GCC and Clang/GCC integer-only replay profiles compiled and linked.\n'
fi
printf 'Asset-free qualification files: %s\n' "$task_work"
