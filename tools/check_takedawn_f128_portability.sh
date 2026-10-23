#!/usr/bin/env bash
# Maintainer-only portable wider arithmetic; no game media or host FP types.
set -euo pipefail
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_sf="$task_runtime/softfloat"
task_host_cc=${GAMEPKG_HOST_CC:-cc}
task_mips_cc=${GAMEPKG_MIPS_CC:-mips-linux-gnu-gcc}
task_qemu=${GAMEPKG_QEMU_MIPS:-qemu-mips}
for task_tool in "$task_host_cc" "$task_mips_cc" "$task_qemu"; do command -v "$task_tool" >/dev/null; done
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-f128-portability.XXXXXXXX")
task_corpus=${GAMEPKG_F128_CORPUS_SOURCE:-}
if [[ -z "$task_corpus" ]]; then
    task_corpus="$task_work/reference.bin"
    GAMEPKG_TAKEDAWN_F128_CORPUS="$task_corpus" cargo test --locked --release \
        --manifest-path "$task_root/engines/takedawn/native-gen/Cargo.toml" --test f128_math -- --nocapture
fi
[[ -f "$task_corpus" && -r "$task_corpus" ]]
task_sources=("$task_root/engines/takedawn/native-gen/tests/portable_f128.c"
    "$task_runtime/gamepkg_x86_f128.c" "$task_runtime/gamepkg_x86_ext80.c")
for task_list in sources.txt ext80-sources.txt wide-sources.txt; do
    while IFS= read -r task_source; do
        [[ "$task_source" == source/*.c && "$task_source" != *..* ]]
        task_sources+=("$task_sf/$task_source")
    done < "$task_sf/$task_list"
done
task_flags=(-std=c11 -O3 -Wall -Wextra -Werror -I"$task_runtime" -I"$task_sf"
    -I"$task_sf/source/include" -I"$task_sf/source/8086-SSE")
"$task_host_cc" "${task_flags[@]}" -fsanitize=undefined -fno-sanitize-recover=all "${task_sources[@]}" -o "$task_work/host"
"$task_work/host" "$task_corpus"
"$task_mips_cc" "${task_flags[@]}" -static -DGAMEPKG_REQUIRE_BIG_ENDIAN=1 \
    -fsanitize=undefined -fsanitize-undefined-trap-on-error "${task_sources[@]}" -o "$task_work/mips32"
"$task_qemu" "$task_work/mips32" "$task_corpus"
sha256sum "$task_corpus"
printf 'Asset-free qualification files: %s\n' "$task_work"
