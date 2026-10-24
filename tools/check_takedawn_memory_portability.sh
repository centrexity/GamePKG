#!/usr/bin/env bash
# Maintainer-only concrete RAM/fault/fence test; consumes no game media.
set -euo pipefail
task_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
task_runtime="$task_root/engines/takedawn/native-gen/runtime"
task_host_cc=${GAMEPKG_HOST_CC:-cc}
task_mips_cc=${GAMEPKG_MIPS_CC:-mips-linux-gnu-gcc}
task_qemu=${GAMEPKG_QEMU_MIPS:-qemu-mips}
for task_tool in "$task_host_cc" "$task_mips_cc" "$task_qemu"; do command -v "$task_tool" >/dev/null; done
task_work=$(mktemp -d "${TMPDIR:-/tmp}/gamepkg-memory-portability.XXXXXXXX")
task_sources=("$task_root/engines/takedawn/native-gen/tests/memory_runtime.c"
    "$task_runtime/gamepkg_x86_memory.c" "$task_runtime/gamepkg_x86_fault.c")
task_flags=(-std=c11 -O2 -pthread -Wall -Wextra -Werror -I"$task_runtime")
"$task_host_cc" "${task_flags[@]}" -fsanitize=address,undefined -fno-sanitize-recover=all "${task_sources[@]}" -o "$task_work/host"
ASAN_OPTIONS=detect_leaks=1 "$task_work/host"
"$task_mips_cc" "${task_flags[@]}" -fsanitize=undefined -fsanitize-undefined-trap-on-error \
    -static -DGAMEPKG_REQUIRE_BIG_ENDIAN=1 "${task_sources[@]}" -o "$task_work/mips32"
"$task_qemu" "$task_work/mips32"
printf 'Asset-free qualification files: %s\n' "$task_work"
