# Portable game data and 32-bit hosts

GamePKG compiles every guest for `wasm32-unknown-unknown`. A game has the same
32-bit address model and little-endian WebAssembly memory on every supported
native host. The host rejects memory64 modules in both runtime profiles.
Wasmtime selects its Pulley interpreter automatically on 32-bit x86 and ARM;
games still receive the same scoped APIs, memory limits, fuel, and deadlines.

The portable source policy runs during packing, verification, installation,
and compilation, including for signed packages and unused source files.
It requires explicit fixed-width byte conversions:

```rust
let value: u32 = 0x01020304;
let bytes = u32::to_le_bytes(value);
let restored = u32::from_le_bytes(bytes);
let _ = gamepkg::save_write(&bytes);
```

Little- and big-endian encodings are both allowed when named explicitly. Native
endian conversions and `NativeEndian` are forbidden. Method-form conversion
such as `value.to_le_bytes()` is rejected because a syntax scan cannot establish
the receiver's type; use the qualified fixed-width primitive instead. `usize`
remains available for in-memory lengths and indexes, but cannot select a
serialized integer's size. Convert explicitly to a fixed-width type when
encoding a length. Guest asset, ROM, and save counts use `u32` in both Rust
binding profiles and the WIT interface.

Raw pointers, pointer extraction, raw slice construction, transmutation, layout
inspection, `isize`, platform-specific intrinsics, and primitive-name shadowing
are rejected. Existing bans on macros, attributes, aliases, callbacks, build
scripts, and unapproved crates prevent alternative architecture-specific paths.
Host-generated bindings and reviewed host-owned dependencies are trusted code;
they are not subject to the guest restrictions.

The qualified save example and adapted port keep their existing save byte
formats. They now spell out `u32` conversions and have newer recipe versions.
Earlier signed packages remain subject to the stricter source policy; creators
can supply higher-version signed updates with portable source.

## Qualification

Local tests executed 22 checks per platform: 32-bit Linux x86, 32-bit Windows
x86 under Wine, and actual 32-bit Android x86 userspace in a separate emulator.
The Android ARMv7 CLI cross-compiles with NDK 27.0.12077973 and API 23. Linux and
Windows 32-bit GUI sources also undergo architecture-specific build checks.
Golden tests check a value larger than `u32::MAX`, little-endian memory bytes,
core/component boundaries, memory and execution limits, and signed source
rejection. Arithmetic NaN results use canonical bits, and relaxed SIMD is
disabled to avoid architecture-dependent instruction results. Fixed big-endian
encodings are checked separately. This does not
claim that the full native host has been executed on big-endian hardware.

The `32-bit game portability` GitHub workflow repeats the Linux/Windows tests,
builds CLI and desktop binaries, and runs Android tests in an x86 emulator.
Release packaging reads executable ELF/PE architecture rather than the build
machine's word size, and refuses mixed CLI/GUI architectures.

Android qualification currently covers the CLI and runtime. It is not an
Android GUI APK or an Android-local sandboxed compiler. Windows 32-bit playback
can consume validated Linux compiler exchanges; 32-bit Windows itself does not
provide WSL 2. Keep source compilation isolated rather than disabling the
compiler boundary to accommodate a new platform. The downloadable v0.1.6
release remains the previously qualified 64-bit Linux/Windows preview.

## Developer commands

```sh
cargo test --locked --test portability --test runtime --test components
cargo test --locked --target i686-unknown-linux-gnu --test portability --test runtime --test components
cargo test --locked --target i686-pc-windows-msvc --test portability --test runtime --test components
```

Android builders need the matching Rust target and NDK compiler/linker configured.
After building the three test executables, run them on a dedicated test device:

```sh
cargo test --locked --target i686-linux-android --test portability --test runtime --test components --no-run
bash tools/test_android_runtime.sh emulator-5554 i686-linux-android
```

These are developer checks. Players do not need Python or signing keys.
