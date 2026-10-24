#!/usr/bin/env bash
# Maintainer-only, synthetic hardware corpus: no owned media or player tooling.
set -euo pipefail
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_host_cc=${GAMEPKG_HOST_CC:-cc}
task_mips_cc=${GAMEPKG_MIPS_CC:-mips-linux-gnu-gcc}
task_qemu=${GAMEPKG_QEMU_MIPS:-qemu-mips}
for task_tool in "$task_host_cc" "$task_mips_cc" "$task_qemu"; do command -v "$task_tool" >/dev/null; done
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-mmx-shift-portability.XXXXXXXX")
task_corpus=${GAMEPKG_MMX_SHIFT_CORPUS_SOURCE:-}
if [[ -z "$task_corpus" ]]; then
    [[ $(uname -s) == Linux && $(uname -m) == x86_64 ]]
    task_corpus="$task_work/oracle.bin"
    GAMEPKG_TAKEDAWN_MMX_SHIFT_CORPUS="$task_corpus" cargo test --locked --release \
        --manifest-path "$task_root/engines/takedawn/native-gen/Cargo.toml" --test mmx_shift -- --nocapture
fi
[[ -f "$task_corpus" && -r "$task_corpus" ]]
task_sources=("$task_root/engines/takedawn/native-gen/tests/portable_mmx_shift.c"
    "$task_runtime/gamepkg_x86_x87.c" "$task_runtime/gamepkg_x86_fault.c")
task_flags=(-std=c11 -O3 -Wall -Wextra -Werror -I"$task_runtime")
"$task_host_cc" "${task_flags[@]}" -fsanitize=undefined -fno-sanitize-recover=all "${task_sources[@]}" -o "$task_work/host"
"$task_work/host" "$task_corpus"
"$task_mips_cc" "${task_flags[@]}" -fsanitize=undefined -fsanitize-undefined-trap-on-error \
    -static -DGAMEPKG_REQUIRE_BIG_ENDIAN=1 "${task_sources[@]}" -o "$task_work/mips32"
"$task_qemu" "$task_work/mips32" "$task_corpus"
sha256sum "$task_corpus"
printf 'Asset-free qualification files: %s\n' "$task_work"
