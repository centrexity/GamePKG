# Native Windows prepared-source build

The native Windows builder compiles prepared Rust using a standalone Windows
Rust toolchain and Windows LLVM tools. It does not invoke WSL, translation tools,
Python, or game code during compilation. The app's normal Build/Play integration
and managed SDK installer remain unfinished. This is currently an internal build
path, separate from the verified Linux player source-kit pin.

`../windows-dependencies.json` pins Rust 1.89.0's Windows GNU compiler, standard
library, Cargo and MinGW components, plus LLVM-MinGW 20260922, CMake 3.31.12
and Ninja 1.13.2. Each official
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

`build-windows-runtime.rs` supplies that original runtime link. It accepts
`--kit`, `--sdk`, `--game-archive`, `--output`, `--cpu` and `--jobs`; these are
host-owned paths to previously admitted inputs. It builds the media bridge
offline from the kit's vendored dependencies, compiles the pinned glslang source,
derives the Vulkan loader import library from the pinned headers and builds the
original C/C++ runtime through native Windows CMake/Ninja/Clang. The prepared
game and C/C++ runtime use the selected CPU target. The host media bridge keeps
release optimization/LTO with the portable x64 CPU target: CPU-specific flags in
that library regressed the original audio check, while native-built generic-CPU
and previous baseline libraries pass with the same optimized C/C++ runtime.
SDK paths containing spaces are passed as individual arguments, including
Cargo's encoded linker flags. The runtime uses Vulkan and retains audio, input,
settings and the original managed-restart connection.

The tool checks every prepared game export, copies SDL2.dll beside the executable
and records the source manifest, game/media archives, executable, companion DLL
and tool-version hashes in `runtime-qualification.json`. A failed build removes
any old qualification receipt. It does not read the ISO or run the game. The
calling app still owns kit/SDK admission, final integrity publication and launch.

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

[40054185588](https://github.com/centrexity/GamePKG/actions/runs/40054185588)
passes actual Windows checks:

- MSVC32 and MSVC64 each pass three checkpoint/hash tests and an explicitly
  invoked native MSVC BigObj fixture. The tests compile real Rust objects,
  relocate a private source tree, recover from damaged checkpoints, reject
  invalid Rust and check standard SHA-256 vectors. Linux also passes its three
  portable-cache tests.
- A standalone SDK test downloads the pinned Windows GNU Rust, LLVM, CMake and Ninja tools,
  limits PATH to those tools plus Windows system files, compiles the Rust build
  units driver, runtime driver and cache wrapper, then assembles two synthetic
  Rust modules. Both checkpoints reuse on the next build. Native CMake/Ninja and
  Clang compile and link a C++ executable with that archive, including C++ and
  Rust heap allocation; execution returns the expected 11.
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

The Rust runtime driver also builds the complete original Windows runtime under
isolated Wine, offline from the v10 source kit, with all 24,032 exports. That
comparison uses the previously qualified baseline-CPU prepared Rust game archive;
it does not stand in for the still-running native-CPU game compilation. The new
runtime passes original Crash controls/results/Continue and a sequential Xbox
race check (+9.1 m/s acceleration), but fails the unchanged save/audio deadline.
The preceding v3 executable passes that same check in the current environment.
The comparison isolates the media library: replacing only that library with the
previous one restores a pass, as does a native-built generic-CPU media library.
The current builder selects the verified x64 media target. The rebuilt v5
executable passes original profile creation/reload with unchanged save hashes,
the complete 45-second non-silent stereo capture, and the actual window/settings
restart gate. Render scale changes from 1x to 2x, persists after one managed
restart, and both processes shut down normally in the same tracked session.
The owner admits the read-only ISO twice despite an invalid legacy filename.
Renderer-owned captures confirm the applied setting has no pending-change prompt.
The restart gate accepts the authoritative parent event if the late C-stdio
diagnostic is unavailable during teardown; every final shutdown, setting and
window assertion remains. Parallel Wine runs also failed timing checks; those
results are not counted as qualified.

## Private source-kit candidate

The v10 candidate includes the native Windows build driver, portable checkpoint
wrapper, export inventory and current Windows media/restart adapters. Its archive
SHA-256 is `9a94b3e286286ab89506837cca3095b21a2f3bc844c5bbd8a2ca44d6ab42cb08`
(55,262,902 compressed bytes; 4,256 inventoried files). The actual Rust source-kit
loader accepts the archive. The installed player kit remains pinned to v8.

The prepared game/build inputs exactly match the qualified v9 Linux inputs, so
the candidate reuses that verified Rust archive. Its media bridge builds offline
with an empty Cargo home, and the original native runtime links with all 24,032
exports. Build commands contain neither Python nor generated C game bodies.
All inventoried source files remain unchanged after build and gameplay checks.
The rebuilt Linux executable passes the original race and crash control checks,
including pause/resume, quit/results/continue, plus profile creation/reload with
unchanged save hashes and non-silent stereo capture. These regressions preserve
the working Linux path while Windows app integration proceeds; full campaign,
audible fidelity and Windows player-build qualification remain separate gates.

The v12 candidate adds the fixed runtime builder and updated SDK pins to v10.
SHA-256 is
`ab30c4e7a4a86580a6fbefe7f79be941c713a0dd2b089d73fba6180729ab845b`
(55,265,670 compressed bytes; 4,257 inventoried files). The actual Rust source-kit
loader accepts it; all source hashes remain intact. Its prepared Rust and Linux
runtime sources remain identical to v10. The v5 runtime qualification used the
v10 source tree plus the current host runtime driver, whose source exactly
matches the driver packaged in v12. Normal signed app build, complete native-CPU
game compilation and player promotion remain open.
