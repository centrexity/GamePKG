#!/usr/bin/env bash
# No game or ISO needed: prove Cargo bundles independent prepared Rust objects.
set -euo pipefail
project=$(cd -- "$(dirname -- "$0")/.." && pwd)
task_root=$(mktemp -d /tmp/gamepkg-takedawn-prepared.XXXXXXXX)
mkdir -p "$task_root/src"
cp "$project/engines/takedawn/rust-port/prepared-build.rs" "$task_root/build.rs"
cat > "$task_root/Cargo.toml" <<'EOF'
[package]
name = "prepared-build-check"
version = "0.1.0"
edition = "2021"
[workspace]
[lib]
crate-type = ["staticlib"]
[profile.release]
opt-level = 2
panic = "abort"
EOF
printf '%s\n' '// Game functions come from the prepared objects.' > "$task_root/src/lib.rs"
printf '%s\n' first second > "$task_root/units.txt"
cat > "$task_root/src/first.rs" <<'EOF'
extern "C" { fn prepared_second(x: u32) -> u32; }
#[no_mangle]
pub unsafe extern "C" fn prepared_first(x: u32) -> u32 { prepared_second(x).wrapping_add(7) }
EOF
cat > "$task_root/src/second.rs" <<'EOF'
#[no_mangle]
pub extern "C" fn prepared_second(x: u32) -> u32 { x.wrapping_mul(3) }
EOF
cargo build --offline --release --manifest-path "$task_root/Cargo.toml"
cat > "$task_root/main.c" <<'EOF'
#include <stdint.h>
#include <stdio.h>
extern uint32_t prepared_first(uint32_t);
extern uint32_t prepared_second(uint32_t);
int main(void) {
    uint32_t values[] = {0, 1, 999, UINT32_MAX, UINT32_MAX / 3};
    for (unsigned i = 0; i < sizeof values / sizeof *values; ++i) {
        uint32_t x = values[i];
        if (prepared_first(x) != x * 3u + 7u || prepared_second(x) != x * 3u) return 1;
    }
    puts("PASS: independent prepared Rust objects are bundled, exported and callable from the native runtime");
}
EOF
cc "$task_root/main.c" "$task_root/target/release/libprepared_build_check.a" -ldl -lpthread -lm -o "$task_root/check"
"$task_root/check"
# Inventory errors must fail, never silently omit a unit.
printf '%s\n' first first > "$task_root/units.txt"
if cargo build --offline --release --manifest-path "$task_root/Cargo.toml" > "$task_root/rejected.log" 2>&1; then
    echo 'FAIL: duplicate inventory accepted' >&2
    exit 1
fi
grep -q 'duplicate prepared Rust unit' "$task_root/rejected.log"
printf '%s\n' '../outside' > "$task_root/units.txt"
if cargo build --offline --release --manifest-path "$task_root/Cargo.toml" > "$task_root/path-rejected.log" 2>&1; then
    echo 'FAIL: path escape accepted' >&2
    exit 1
fi
grep -q 'invalid prepared Rust unit' "$task_root/path-rejected.log"
echo "PASS: invalid and duplicate unit inventories rejected; evidence: $task_root"
