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
  --recover-gameplay-paths /path/to/owned.iso --native-mxcsr \
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

### Windows preparation

The Windows preparation audit found two concrete blockers in Linux-generated
Rust. Importing `stderr` as a global fails to link on Windows, and thirteen
`INT64_MIN` declarations use Linux's 64-bit `long`, which becomes a 32-bit type
on Windows. The normalizer now uses native stream getters and native C wrappers
for `fprintf`/`fflush`, keeping each `FILE` in the runtime that owns it. `FILE`
remains an opaque pointer; by-value use, layout-dependent arrays and fixed-size
construction are rejected. The exact known `INT64_MIN` definition becomes
`i64::MIN`; a changed definition fails preparation rather than truncating it.

`tools/check_takedawn_rust_stdio.py` compares converted Rust with the platform's
native C stream identities, fixed-width minimum, variadic 32/64-bit values and
flush results. The fixture passes on Linux 64/32-bit and Windows GNU 64/32-bit
under a private maintainer Wine runner. Native Windows MSVC 64/32-bit CI runs
the same fixture directly. Both Windows architectures and Linux pass in
[run 40018020006](https://github.com/centrexity/GamePKG/actions/runs/40018020006);
Linux also passes all six normalizer tests and strict formatting/Clippy. Those
checks are separate from full game support.
The original actual game-code stub fails Windows linking before this change;
afterward it links and passes its logging and return-stack probe.

CI consumes the small prepared asset-free fixture in `tests/stdio-prepared`;
its provenance binds the C source, normalizer source and resulting Rust hashes.
To refresh it after a reviewed normalizer change, run the fixture tool with
`--c2rust /private/tools/c2rust --normalizer /path/to/gamepkg-takedawn-rust-port
--refresh-fixture`. Conversion tools and Wine remain maintainer-only tools.

Newly prepared Windows game source is a private qualification candidate. The
complete optimized 486-unit Windows archive and original native runtime now
link successfully, retaining all 24,032 Rust exports. The ISO transport includes
native file/directory APIs, startup reads and SDL streams. Actual MSVC 32/64-bit
stream/admission/file and real Rust reader tests pass. Isolated Wine tests now
cover original race/Crash controls, profile create/reload, 45-second mixed stereo
and a keyboard-driven, resizable native Vulkan window with clean closing.
The private universal v9 source candidate also passes Linux gameplay/save/audio
regressions and full source-hash preservation. Details and remaining gates are in the
[media integration notes](../media-bridge/README.md). The player source-kit pin
remains the verified Linux v8 kit while actual Windows-host hardware checks,
native app build/launch integration, process isolation and complete gameplay are
qualified. These checks do not prove a completed Windows game release.

### Linux gameplay baseline

The complete pinned C baseline reproduced its expected tree hash, 108/108
patches and both spin-wait adjustments. All 24,001 generated game functions
converted into 484 Rust units and compiled with stable Rust 1.89.0 on Linux
x86-64. All 24,030 expected native exports were retained, including upstream
manual/dispatch code and its 15 existing unresolved stubs. Those stubs are
preserved baseline behavior, not newly implemented functions.

The v7 prepared kit recovers two of those entries: `0x000966F0` is a complete
game-state transition body, and `0x00249AB6` is a floating-point error-result
body. Both have direct branches from existing generated functions. Recovery
checks the supported executable hash, those original branches, non-overlap with
existing functions, complete decoding through the return, and all local branch
boundaries. It uses the pinned original translator and adds a separate native
Rust unit; existing function boundaries remain unchanged. The resulting
inventory is 24,003 translated functions in 485 units, with the same 24,030
exports and 13 remaining upstream unresolved stubs.

Pass `--recover-gameplay-paths /path/to/owned.iso` to the maintainer conversion
command to prepare this kit. The executable stays in sealed RAM; original media
is never written to disk. `tools/check_takedawn_recovered_paths.py --source
/private/pinned/takedawn --toolkit /private/pinned/xboxrecomp --prepared
/tmp/private/new-conversion` compiles both recovered C and Rust bodies and passes
128 comparisons, with independent assertions for state branches, signed mode
values, callback arguments, return-stack balance and every floating-point stack
position. These focused checks do not establish complete campaign coverage.
The v7 kit also passes the normal signed-package native Build and integrity
checks. Its freshly linked native executable passes the original Xbox-layout
race assertions, including trigger acceleration, pause/resume and Quit > Yes
returning to the front end, with unresolved-stub logging enabled. The same
executable passes the PS2-layout Crash impact/results/Continue assertions and
the signed-package Play, remembered ISO, failed-disc recovery and history gate.
The v7 kit contains 4,238 verified files; 483 unchanged Rust unit checkpoints were
individually verified before reuse and both changed/new units compiled afresh.
No runtime/build inputs or other Rust units changed. The earlier v6 results
below remain baseline evidence.

The v8 candidate also restores the original SSE control setup at `0x001DD63F`
and `0x001DD652`. The baseline logged both instructions as reached no-ops:
the native engine never received the game's requested flush-to-zero setting.
Maintainer preparation replaces only those exact two sites with calls to a
reviewed Rust leaf unit using the real host `STMXCSR` and `LDMXCSR` instructions.
The original read, stack writes, bitwise OR and load order remain intact.
This changes the existing native OS thread's floating-point state, shared with
the original engine. It adds no software CPU or arithmetic decoder.

`tools/check_takedawn_native_mxcsr.py` passes 512 cases on actual Linux x86-64
and i686, with both Clang and GCC references. It checks all four rounding modes,
all 64 sticky-status combinations, FTZ and subnormal results, the original
read/OR/load sequence, 8,192 thread-local reads, and matching hardware faults
for reserved bits. Preparation rejects duplicate or incomplete setup pairs.
The inventory becomes 486 Rust units: all 24,030 original game/runtime exports
remain, alongside two named native-control helpers, for 24,032 exports total.
The full native kit remains qualified only for Linux x86-64; a 32-bit helper
fixture does not establish complete 32-bit game support.
The v8 signed-package native Build/integrity and Play/remembered ISO/failed-disc
recovery/history checks pass. Its actual executable passes the original Xbox
race controls, with neither former SSE no-op diagnostic present. The gameplay
gate now rejects either diagnostic; it demonstrably rejects the prior v7 log.
The same v8 executable passes the PS2-layout Crash impact, results and Continue
assertions with the new guard enabled. No remaining unresolved stub is reached.

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

Use `xbox race-complete` for the separate whole-race gate. It runs the pinned
original `race_drive.sh`: the title's own AI drives the first Silver Lake race
through its laps, finish and results screen. It does not unlock tracks, shorten
the race or inject a result. This is a wider native execution check, separate
from player-control equivalence and full campaign progression. The maximum
test duration is twenty minutes; the race/crash control checks keep their
existing five-minute bound. Both paths reject reached unresolved game stubs.
The exact signed v12 Linux executable passes this gate: all three laps, fourth
of six, 329.150 seconds, a 104.600-second best lap, the results screen and normal
test shutdown. No unresolved stub is reached. This records one complete Single
Event race, not a completed World Tour or physical-controller qualification.

A separate private check of the same native executable completes the first
World Tour Silver Lake race using the original AI: first of six over two laps,
209.833 seconds, best lap 101.817 seconds. The original results award a gold medal,
three newly unlocked events and 1,900 burnout points. Continue reaches the normal
successful autosave and award screens. A fresh process then loads a private copy
of that save: the region screen shows 1/55 USA and 1/173 overall gold medals, and
the unlocked Downtown Road Rage event remains available. The active 59,904-byte
profile and 54-byte metadata retain their post-autosave hashes through reload.
Both runs read the ISO in place and reach no unresolved game-function stub.
This qualifies the first campaign event and its persistence, not the remaining
World Tour events or player-control equivalence.

Use `xbox roadrage-complete` for the original driver's Single Event Road Rage
route. It chooses among tracks already available in the supplied private save,
drives with the original AI and waits for the natural event end and results.
The exact signed v12 Linux executable passes on US/C3_V1: seven takedowns, vehicle
totaled, the original new-record/results screen and test shutdown, with no
unresolved game-function stub reached. The observed run takes 869 wall seconds,
within the same twenty-minute complete-event bound. Its end-state log shares
the race driver's generic placing/lap fields; those are not Road Rage rankings.
The takedown count and Totaled result are verified in the renderer capture.

An extended private PS2-layout Crash probe also exercises the physical R2-to-game
B mapping during impact time, aftertouch and Crashbreaker. Captures show the ready
prompt, the effect after R2 and results with twelve vehicles totaled and $104,113.
The original control assertions pass through return to the front end; the new
high-score results table needs an additional reviewed Continue press. The ready
prompt in that v12 build still shows B instead of the configured right trigger.
The v13 host adaptation fixes the shared sheet and dedicated A/B textures using
only the runtime's own glyph drawings. It follows the actual original control
mapper, with keyboard/touch unchanged and menu/results restoring the normal
family. A native PlayStation-glyph Crash probe visibly shows R2 and exercises
that trigger. The asset-free gate compares all button positions against the
original input mapper, preserves all 15 normal glyph families, checks disconnect
and shared-controller transitions, rejects changed/in-place upstream staging,
and passes ASan/UBSan. Prepared Rust game inputs remain identical to v12; all
24,032 exports remain present. Actual mixed-controller hardware validation and
Windows original-game rendering still require qualification. The normal signed
v0.1.2 package also builds, passes integrity and incorrect-disc rejection,
remembers its ISO, and increases host play history. Its exact executable passes
Crash/R2/results/Continue and PS2 race acceleration, pause/resume, Quit and menu
assertions. A profile that already completed the driving tutorial loads the
track directly; the maintainer driver now handles that route and fresh-profile
tutorials without changing the original input/gameplay checker.

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

The v12 kit also passes normal Linux signed-package Build, integrity, invalid-ISO
rejection, Play and remembered-ISO launch in a fresh private library. The resulting
66,783,496-byte executable has SHA-256
`cf40fb68f443dff431c23bf1a1c1df95d6010b75fa2ecad926d2a0db510dda02`
and all 24,032 required exports. That exact executable passes the original Xbox
race controls and PS2-layout Crash results/Continue checks. Profile reload and
45 seconds of non-silent stereo capture pass too, but two preceding unchanged
runs missed the audio-capture deadline. Audio timing consistency remains open;
the successful repeat does not resolve those failures. No test deadline or
original gameplay assertion was relaxed.

The native Windows app session fixture passes on Windows CI: original save
retention, exclusive ownership, managed restart, play history and executable/SDL
integrity. It uses a small host fixture; original-game Vulkan rendering and input
inside the Windows AppContainer still require separate verification. Gameplay
and fidelity take priority over additional resource-policy tuning.

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

The earlier v6 private source-kit baseline also built its original native runtime
from a fresh relocated tree. Its host media adapter builds offline with an empty
Cargo home using vendored dependencies; all 4,237 shipped source files retain
their verified hashes afterward. The linked 66,788,904-byte executable retains
all 24,030 game exports, contains no debug sections, and its build commands invoke
neither Python nor generated C game bodies. It passes the native-window process
owner gate, original profile create/reload and 45-second stereo capture gate.
A profile actually updated by the Crash event reloads into another race with
passing controls, using only copied profile files and the ISO in place.

## Standalone Windows compiler path

The host now has a [native Windows prepared-source build driver](WINDOWS_BUILD.md)
and Windows checkpoint backend, with an asset-free end-to-end standalone SDK
test passing on actual Windows. It compiles Rust and links native C/Rust without
WSL or Python in the build commands. The current Linux checkpoint code and player
kit stay unchanged. The full Windows-native-compiler game build and normal app
Build/Play integration remain separate release gates.
