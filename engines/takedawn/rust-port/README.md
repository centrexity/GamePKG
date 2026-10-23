# Takedawn game code in Rust

The integration direction is to retain Takedawn's working native runtime and
port its generated game functions from C to Rust. The experimental replacement
CPU/runtime in `../native-gen` is not the delivery path for this port.

Generation and conversion happen during release preparation. Python and C2Rust
are maintainer tools. Players receive prepared Rust game source, compile it to
native code locally and launch it through GamePKG. Original assets are read
from their selected ISO in place through `../media-bridge`; no extracted media
folder is required. The existing renderer/system runtime remains C/C++ initially.
This does not mean the whole Takedawn engine has been rewritten in Rust.

## Conversion

First generate the complete C baseline using `tools/check_takedawn_regen.py`.
This uses a sealed RAM-only XBE and checks the source pins and supported disc.
The generated stamp must record all 108 patches and both spin-wait adjustments.
Build this directory's maintainer tool with Cargo, then run:

```sh
python3 tools/port_takedawn_rust.py \
  --source /private/pinned/takedawn --toolkit /private/pinned/xboxrecomp \
  --c2rust /private/tools/c2rust --normalizer /path/to/gamepkg-takedawn-rust-port \
  --output /tmp/private-mode-700-directory/new-conversion
```

The conversion uses small translation units because large generated C units
can crash C2Rust's AST exporter. Every game function's complete C body is
retained. Conversion errors fail the build; exported function inventories are
checked before the final conversion manifest is written. The manifest records
the original C hashes, generation stamp and all resulting Rust source hashes.

The normalizer parses Rust syntax, replaces unstable foreign TLS declarations
with typed accessors into Takedawn's existing per-thread state, and updates two
renamed pointer-provenance APIs for stable Rust. Reviewed 32-bit atomic fetch-add
and compare-exchange calls use stable sequentially consistent atomics, preserving
old values and success flags. Incomplete C types remain distinct pointer-only
markers. It preserves native export
names and ordinary C ABI calls. It introduces no instruction interpreter or
replacement game behavior. The generated code remains unsafe host-engine code;
conversion alone does not establish memory safety or process isolation.

The tested C2Rust source is commit
`b99495ce28b408348d2409156fc1769ab9c6cbc2`. Its dominated-node hoisting
optimization can reference nodes outside the current recursive graph.
`c2rust-relooper.patch` limits moves to nodes actually present in that region.
It also avoids restarting internal-name suffix searches from zero on every CFG
conversion. Native export naming is unchanged. These maintainer tool adjustments
require behavior checks as well as successful conversion.

Large goto-heavy functions use a direct lowering of the complete typed control
flow graph into Rust branches over native basic blocks. Local declarations are
lifted while their initializer effects stay at the original sites. Explicit
value returns and implicit void returns are preserved. The preparation command
enables this path with `GAMEPKG_C2RUST_FLAT_CFG=1`, disables incremental
restructuring, and bounds each converter process to 90 seconds. This changes
compile-time control-flow translation; it adds no runtime instruction decoder.

`tools/check_takedawn_rust_cfg.py` compares compiled C and Rust results on a
synthetic graph with 300 labels, repeated local names, initializer effects,
callbacks into native code, TLS changes, and both kinds of return. It does not
require the game or its disc. The actual-game comparison and mixed C/Rust
save/reload checks are separate private integration gates.

## Validation

The complete pinned C baseline reproduced its expected tree hash, 108/108
patches and both spin-wait adjustments. All 24,001 generated game functions
converted into 484 Rust units and compiled with stable Rust 1.89.0 on Linux
x86-64. All 24,030 expected native exports were retained, including upstream
manual/dispatch code and its 15 existing unresolved stubs. Those stubs are
preserved baseline behavior, not newly implemented functions.

The full native runtime built through `attach.cmake` with the prepared Rust
archive. Its actual build commands contain neither Python nor generated C game
compilation. Repeating the preparation command reproduced every tested Rust
game unit byte for byte. The full Rust build renders original menus and creates
the original profile save. A bounded first-race run recorded 4,270 race frames,
maximum speed 72.2353 m/s, 2,499 throttle frames, 120 brake frames and 1,076
boost-plus-throttle frames. `tools/check_takedawn_drive_log.sh` checks those
original telemetry records. Both steering directions were sent; steering
accuracy is not established by that log.

The unchanged save/reload/audio gate passes against the complete native runtime
build: the original 59,904-byte profile and 54-byte metadata reload with unchanged
hashes, and 45 seconds of non-silent stereo PCM at 48 kHz are captured. Earlier
runs against the mixed baseline and a reused-runtime full Rust binary missed the
capture deadline; those failures remain historical evidence. The passing gate
uses a fresh private save directory and never touches the user's library.

The original control assertions also pass for both Xbox and PS2-style layouts
in a single race: acceleration, menu button handling, pause with the accelerator
held, resume, a second pause, and Quit > Yes returning to the front end. These
are native game inputs, with Start skipping the original tutorial video.
Crash mode passes the original impact/results/Continue-to-front-end assertions
on the source-kit runtime with both layouts. One earlier PS2-style run failed
to observe the player-results state, despite rendering the results screen.
Subsequent unchanged runs passed. The pinned C reference, complete Rust game
on the original runtime, and C game on the source-kit runtime also pass.
The earlier intermittent failure remains unexplained; successful repeats do
not establish full-event reliability or progression correctness.

Repeat the control checks with the maintainer-only wrapper:

```sh
bash tools/check_takedawn_gameplay.sh /private/native/game /private/game.iso \
  /private/pinned/takedawn /private/reference-saves ps2 race
```

The optional layout is `xbox` or `ps2`; the event is `race` or `crash`. The
wrapper checks the upstream driver hashes, copies reference saves into a fresh
private directory, reads the ISO in place, and retains logs and periodic frames.
It preserves every upstream input/state assertion. Backend-specific golden
screenshot timing is disabled. Frame-relative waits also consult a bounded
tail of the original log: its asynchronous filtered log can lag the direct
control-state log by a second and send menu inputs before their intended wait
has elapsed. This corrects test observation, without changing game input handling.
The release-sized optimized build passes both race layouts and the PS2-style
Crash gate with that observation fix. Each run uses a 4 GiB physical-memory scope;
gameplay qualification takes priority over further memory tuning.
The wrapper also enables upstream unresolved-function logging and rejects a run
that reaches one of those stubs. The final source-kit Crash run passes that guard;
it does not establish that all modes avoid the remaining upstream stubs.

## GamePKG library integration

`ports/takedawn` selects the fixed, host-reviewed native source kit. Its game
source has no Cargo dependencies. Package verification rejects replacement
launchers, additional source, build scripts, assets and unrecognized source kits.
The pinned kit includes prepared Rust game code and the original C/C++ host
runtime, with its host dependencies vendored for offline builds. Players need
neither Python nor C2Rust.

The normal native Build action verifies the signed package and builds the kit in
a private compiler directory and its own compiler cache, with no ISO, saves,
other library contents, home or network access.
Stable Rust, Clang 18, CMake, Ninja and the native SDL2/epoxy/OpenSSL/Vulkan/
glslang 15 development tools are required. A separate dependency prefix can be
selected with **Native tools folder** in Settings, or
`GAMEPKG_TAKEDAWN_NATIVE_SYSROOT`. The default optimized build targets the local
CPU. This is a substantial first build: the complete prepared Cargo recipe took
76 minutes 54 seconds on the test machine. The isolated standard library Build
now passes with its integrity check. Its first run was terminated after 456 of
484 units had compiled. After checking every source-kit file and completed
object, the resumed test reused those 456 units, compiled the remaining 28 and
freshly built the native runtime. This records a recovered build, not an
uninterrupted cold standard Build.

Host-owned per-unit checkpoints let later builds resume without repeating the
longest compilations. Keys bind source bytes, compiler identity, local CPU, the
pinned kit and exact compilation arguments. Object checksums are checked before
reuse. Missing or corrupt checkpoints compile again. This compiler cache contains
no ROMs or saves; its size is included in game/library storage accounting.

Native Play runs the original Vulkan window in a separate process, reads the
selected supported ISO in place, and uses a save directory scoped to the game,
creator and save schema. Set **Memory ceiling** to at least 4096 MiB. Only one
session may use that save directory at a time. ROM selection is remembered after
successful startup; play time starts after the runtime is ready. GamePKG's audio
mute and volume settings reach the original runtime. The CLI supports the same
native route, including bounded headless maintainer runs through `--frames`.

Private signed-package tests have verified native cache readiness, remembered
ROM launch, play history, storage accounting and integrity checks using the
validated source-kit executable. The desktop launch entry opens the original
window without a GamePKG overlay and cleans up its process scope when stopped.
Native keyboard input creates the original profile, reloads it on a later
launch, opens World Tour and drives the first race at an observed 108 mph.
Steering, boost, pause and resume were exercised through that window. A normal
window-manager close returns successfully; an earlier forced X11 window destroy
produced a BadWindow error and is not counted as a normal close test.
These tests use temporary libraries and do not modify the player's library.

Full progression, audible fidelity, steering equivalence, physical Windows/
32-bit/other-endian gameplay, comparative performance measurements and production
isolation qualification remain open. The original ISO is still required at
launch. A local signed preview package is available; official ports-library
publication and release qualification
remain unfinished. This private native proof is not a completed release.

## Prepared native build

Preparation writes a dependency-free Cargo project with `units.txt` and a
stable-Rust `build.rs`. The build compiles the prepared units separately and
bundles their native objects into one static library. Players need no Python,
C2Rust, nightly Rust, or game crates. Same-machine builds select the native CPU;
cross builds use a generic CPU unless `GAMEPKG_TARGET_CPU` is supplied. The
current archive builder supports GNU/LLVM archivers; MSVC is not yet qualified.

The complete Cargo release recipe has now built all 484 units with optimization
level 3 and the native CPU on Linux x86-64 (76 minutes 54 seconds on this machine).
The release attachment removes non-executable debug data at link time, retaining
native symbols for diagnostics. Removing those sections from the tested binary
left all 30 allocated sections unchanged and produced a 66,677,912-byte artifact,
within the existing 64 MiB native cache boundary.

Build the prepared kit with `cargo build --offline --release`. The runtime can
then use this directory's `attach.cmake` as its top-level CMake include, passing
`GAMEPKG_TAKEDAWN_RUST_ARCHIVE` and `GAMEPKG_TAKEDAWN_RUST_TLS`. It removes all
pinned C game-function sources and links the Rust library in their place. Supply
pre-rendered upstream glyph art via `BO3_GLYPHS_ART_DIR` so runtime configuration
does not invoke Python either. The original C/C++ runtime still needs its native
compiler and libraries. The signed GamePKG package now selects this reviewed kit
through the normal native Build/Play workflow described above.

`bash tools/check_takedawn_prepared_build.sh` verifies native cross-unit calls,
exports, unsigned wrap behavior and rejection of duplicate/path-containing unit
inventories without a game or ISO. Stable atomic mappings are checked by
`tools/check_takedawn_rust_ffi.py`, including concurrent fetch-add and CAS gates.

The reviewed hot unit `recomp_0021_013` uses C2Rust's regular structured lowering.
Flattening its inner loops into a large label switch caused a substantial
slowdown in the first complete native build. `--structured-unit` can select
additional reviewed units. The preparation report records each strategy, and
raw conversions from a different strategy are not reused. This selection changes
control-flow presentation only; it does not replace game functions with native
library approximations.

`C2Rust-LICENSE.txt` preserves the upstream converter's notices for the included
maintainer patch. The normalizer and prepared build helper are host preparation
code; they do not change the ordinary package validator's dependency or unsafe
code policies.

## Native process owner

`src/takedawn_player.rs` adds the host process owner for a reviewed cached build.
It snapshots and checks the executable, binds an open read-only ISO descriptor,
and exposes one persistent data folder scoped to game, creator and save schema.
The original renderer creates its own window. The host checkpoints play time,
drains bounded diagnostics, and stops the process scope when the session closes.
It uses a 4 GiB physical-memory scope rather than counting the renderer's virtual
RAM aliases against an address-space quota.

Private real-game gates pass for headless launch and a native X11 Vulkan
zero-copy window, including play-history persistence and process shutdown.
Wayland has a socket forwarding path but has not been qualified. These gates
use a temporary library; no real player library or saves are changed. Startup
diagnostics are retained alongside a bounded recent-log tail, so verbose renderer
output cannot erase the disc-admission result or grow the host log indefinitely.
The source-kit loader and process owner are host APIs. Normal signed-package
profile registration and library Build/Play are now connected and tested.

The final private source-kit candidate has also built its original native runtime
from a fresh relocated tree. Its host media adapter builds offline with an empty
Cargo home using vendored dependencies; all 4,237 shipped source files retain
their verified hashes afterward. The linked 66,788,904-byte executable retains
all 24,030 game exports, contains no debug sections, and its build commands invoke
neither Python nor generated C game bodies. It passes the native-window process
owner gate, original profile create/reload and 45-second stereo capture gate.
A profile actually updated by the Crash event reloads into another race with
passing controls, using only copied profile files and the ISO in place.
