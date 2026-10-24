# Native Windows prepared-source build

The native Windows builder compiles prepared Rust using a standalone Windows
Rust toolchain and Windows LLVM tools. It does not invoke WSL, translation tools,
Python, or game code during compilation. The app's normal Build/Play integration
and managed SDK installer remain unfinished. This is currently an internal build
path, separate from the verified Linux player source-kit pin.

`../windows-dependencies.json` pins Rust 1.89.0's Windows GNU compiler, standard
library, Cargo and MinGW components, plus LLVM-MinGW 20260922. Each official
archive is matched to its published SHA-256 before use. Keep the original
component notices. Rust's bundled GNU linker bootstraps Rust tools explicitly;
LLVM also has a gcc-named wrapper, which must not replace that linker on PATH.

`build-windows-units.rs` accepts only host-supplied paths to an already-admitted
prepared tree, verified tools, output/cache directories and the CPU choice
(`native` or `x86-64`). Its prepared inputs are `units.txt`, `exports.txt` and
`src/`. The source-kit packager supplies the export inventory before players
receive the kit. The caller still verifies the fixed source kit and SDK; this
internal executable does not admit arbitrary game packages.

The driver compiles every prepared unit with Rust optimization level 3, uses
`rustc-cache.rs` for verified checkpoints, builds the native object archive and
bundles it with Rust's standard library. LLVM's symbol reader checks every
required export before the driver writes `qualification.json`. Compilation or
missing-symbol errors stop the pipeline. The archive still needs the original
native runtime link and game qualification before it is playable.

Checkpoint identity includes the compiler, enabled CPU features, model/vendor,
wrapper executable, export inventory, source bytes and exact compiler options.
Windows uses its native SHA-256 provider. ELF and normal/BigObj COFF objects are
checked for supported target architecture; damaged objects or receipts compile
again. An explicit compiler path remap lets a subsequent private build directory
reuse the same object without changing `file!()` and panic locations. Cache
entries without that remap retain their actual source path in the key.

The current Linux builder retains its byte-identical `rustc-cache-linux-v1.rs`
(SHA-256 `56918a38cd209dd5899111bb873ebe7d92d2cb1e49be70a8e19d92a4a14b97c7`)
so this Windows work does not invalidate completed Linux checkpoints. The
existing Linux cache regression test passes through the unchanged embedded code.

## Qualification

[40049557634](https://github.com/centrexity/GamePKG/actions/runs/40049557634)
passes actual Windows checks:

- MSVC32 and MSVC64 each pass three checkpoint/hash tests and an explicitly
  invoked native MSVC BigObj fixture. The tests compile real Rust objects,
  relocate a private source tree, recover from damaged checkpoints, reject
  invalid Rust and check standard SHA-256 vectors. Linux also passes its three
  portable-cache tests.
- A standalone SDK test downloads the pinned Windows GNU Rust and LLVM tools,
  limits PATH to those tools plus Windows system files, compiles the Rust build
  driver/cache wrapper and assembles two synthetic Rust modules. Both checkpoints
  reuse on the next build. Native Clang links that archive into a Windows
  executable, including Rust heap allocation; execution returns the expected 11.
  This path uses no rustup, Visual Studio linker, WSL or Python build command.

The same standalone Windows compiler runs under isolated Wine as a maintainer
check and compiles actual prepared game code with `target-cpu=native`. A complete
486-unit native-compiler build is in progress. Its interrupted first attempt
preserved 58 units; the resumed build verifies and reuses those checkpoints.
An 11 MiB source unit requires more compiler memory than the game allowance;
the diagnostic compiler scope is separate from game memory settings.

Python orchestrates the asset-free CI test and Wine is a compatibility diagnostic;
neither is a player requirement. The complete native Windows game archive,
original runtime link, current-build gameplay, app integration and player-kit
promotion still require their own evidence. A native cache/SDK fixture does not
qualify full campaign progression or Windows graphics hardware.
