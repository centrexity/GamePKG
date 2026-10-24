# Native Windows prepared-source build

The standalone Windows pipeline compiles every prepared Rust game unit, builds
the original C/C++ runtime and produces a native Windows executable. It invokes
no WSL, translation tool, Python or game code during compilation. Python and Wine
are maintainer diagnostics. The app's Windows Build action now dispatches this
pipeline for the reviewed v12 kit. Normal Windows Play integration and player
source-kit promotion remain unfinished; the installed Linux player pin remains v8.

## Managed SDK setup

On 64-bit Windows, run `gamepkg setup-native-tools`, or choose **Prepare native
build tools** in Settings. Setup downloads about 485 MB and uses about 2.4 GB
including cached archives. No vendor installer, Python, WSL or administrator
installation is required. The downloader selects Windows' system curl directly,
so an executable in the current library directory cannot shadow it. Setup also
compiles the three host-owned build helpers with the downloaded native Rust
compiler and its bundled linker. Verified helpers are cached separately from
the SDK, with source/SDK identity and stable paths for resumable unit builds.

`src/takedawn_windows_sdk.rs` installs the official standalone tools. Rust 1.89.0
GNU components have gzip pins from Rust's published channel metadata; LLVM-MinGW
20260922, CMake 3.31.12, Ninja 1.13.2, SDL2, Vulkan headers and glslang are pinned in
`../windows-dependencies.json`. Extraction verifies each archive before reading
its payload. Rust component notices are retained; no install script executes.

The expected payload inventory is independently derived from those official
archives: **21,459 files, 1,858,144,785 bytes**, canonical inventory SHA-256
`c26f09324eda2a9f8091bf7330d5363bd22f4cee9a98fb93fe58bbe3eb02ec67`.
Every cached file is checked on reuse. Editing both a binary and its local cache
manifest still fails the host's inventory pin. Bad caches are preserved under a
quarantine name and replaced atomically. Links, special files, path escapes,
Windows device names and case aliases are rejected. GitHub's PAX commit comments
are metadata only; path overrides are not accepted.

Six local fixture tests pass, including cache reuse/repair, forged cache metadata,
archive conflicts, links and path aliases. The real installer also passes with
all official archives and the pinned full inventory. The resulting SDK builds
the complete native game/runtime under Wine. Actual Windows app CI separately
checks the CLI installation, reuse, every payload hash and native tool startup
before its legacy WSL fixtures; that latest app-level gate is still running.

## Build tools

`build-windows-units.rs` accepts host-supplied paths to an admitted prepared tree,
verified SDK, output/cache directories and the CPU choice (`native` or `x86-64`).
Inputs are `units.txt`, `exports.txt` and `src/`. It compiles at optimization level
3, validates resumable COFF checkpoints, assembles the native archive with Rust's
standard library and checks every required export before writing
`qualification.json`.

`build-windows-runtime.rs` accepts `--kit`, `--sdk`, `--game-archive`, `--output`,
`--cpu` and `--jobs`. It builds the media bridge offline from vendored sources,
compiles glslang, derives Vulkan loader imports from pinned headers, and builds
the original runtime through native CMake/Ninja/Clang. It retains Vulkan, audio,
input, original settings and managed restart. It copies SDL2.dll beside the game
and records source, archive, executable, companion and tool-version hashes.
A failed build removes any prior qualification receipt. The caller still owns
source-kit/SDK admission, final integrity publication and launch. The app now
performs admission and publication through `src/takedawn/windows_build.rs`:
it verifies a signed package snapshot, rejects the older Linux-only kit on
Windows, prepares tools, invokes both builders and rechecks the installed package
before publishing. SDL2 must match the admitted SDK; executable and companion
integrity receipts are checked when inspecting the native cache. Cache identity
includes the Windows tool recipe, helper sources, CPU model/features and OS AVX
support. Windows PE artifacts are checked for architecture and executable type.
The previous Linux recipe/cache behavior is preserved, and its source loader
also admits the reviewed v12 kit. Compiler children do not open console windows.

The game and renderer retain native CPU optimization. The host I/O bridge uses
the portable x64 CPU target with release/LTO optimization: CPU-specific flags in
that library reproducibly missed the original audio deadline. The preceding
library, a replacement of that library alone, and a native-built generic-CPU
library passed. The corrected full native build passes the unchanged check.

Checkpoint identity includes compiler/CPU features, model/vendor, wrapper bytes,
export inventory, source bytes and compiler options. Windows SHA-256 uses BCrypt.
ELF and normal/BigObj COFF objects are validated. Explicit path remapping permits
verified reuse across private source directories. Linux retains its byte-identical
`rustc-cache-linux-v1.rs` so this work does not invalidate existing Linux cache
identity.

## Qualification

[40054185588](https://github.com/centrexity/GamePKG/actions/runs/40054185588)
passes actual MSVC32/MSVC64 cache/hash/BigObj, media, file/stream/kernel and strict
process/restart checks. Its x64 standalone SDK gate compiles both Rust builders,
reuses real Rust checkpoints, and builds/runs C++/Rust heap allocation through
native CMake/Ninja/Clang. Linux checks also pass.

The complete **486-unit, 24,032-export** prepared game archive now compiles with
standalone native Windows Rust and `target-cpu=native` under isolated Wine. The
resumed build preserved its verified checkpoints and completed in 5,806 seconds.
Archive SHA-256 is
`2b240179185bfcc3490decbf711e188480b0a9440fd68689482eba59bbf46c21`.

Using GamePKG's managed SDK and that archive, the native runtime builder produces
the v6 executable: **60,064,768 bytes**, SHA-256
`c46120afab1ed10989ea0b9013cc30d45a9b6e98e7ac9b530948d48dd8576fc0`.
All game and runtime compilation uses Windows tools; no Linux cross compiler
supplies this build. It passes these original-engine checks under Wine:

- Xbox race controls, acceleration (+10.4 m/s), pause/resume and Quit.
- PS2-layout Crash controls, impact, results and Continue.
- Original profile creation/reload with unchanged 59,904-byte profile and 54-byte
  metadata, plus 45 seconds of non-silent 48 kHz stereo capture.
- Original settings UI changes render scale from 1x to 2x, cancels then confirms
  restart, verifies the persisted setting without a pending-change prompt and
  quits normally. One Rust session admits the read-only ISO twice despite an
  invalid legacy filename, opens two Vulkan windows and completes both original
  shutdowns. Renderer-owned captures are retained. The owner event is authoritative
  when a late C-stdio teardown diagnostic is unavailable.

Earlier baseline-CPU v5 Crash testing hit its diagnostic deadline; it is not
counted as a pass. The final native-CPU v6 build passes the original gate without
weakening its input, result or timeout assertions. Parallel Wine timing failures
are likewise retained as unqualified diagnostics.

## Source-kit candidate and remaining gates

Private v12 contains the fixed runtime builder and prepared Rust. Archive SHA-256:
`ab30c4e7a4a86580a6fbefe7f79be941c713a0dd2b089d73fba6180729ab845b`
(55,265,670 compressed bytes; 4,257 inventoried files). The actual Rust loader
accepts it, all source hashes remain intact after the full Windows build/tests,
and its prepared Rust/Linux runtime sources match the earlier qualified Linux
candidate. The v6 build uses this exact source tree and its complete native-CPU
archive. Older SDK metadata in this frozen kit is informational; the app admits
managed tools against its current host-pinned inventory.

The new app Build dispatch passes Windows cross-target compilation and local
path/artifact fixtures; the complete signed-package Windows build still needs
an app-level run. Windows Play, save/history integration, production
filesystem/network confinement and player-kit promotion remain open. Actual
Windows hardware gameplay, complete campaign progression, physical controllers
and audible/steering fidelity still need evidence. Wine and narrow API fixtures
do not establish those scopes. No stable release is promoted by these checks.
