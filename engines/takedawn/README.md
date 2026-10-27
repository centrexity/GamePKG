# Xbox Burnout 3 / Takedawn integration

The delivery path reuses Takedawn's native C/C++ runtime and converts its
complete generated C game functions into prepared Rust before players receive
them. Players compile that Rust locally; the selected supported ISO supplies
assets in place. Python and C2Rust are maintainer preparation tools.

The current unreleased recipe is 0.1.3/v14 and requires the 0.1.18 development
app. It retains 24,003 prepared Rust game functions across 487 units and 24,035
native exports. Linux x64 and standalone Windows x64 tools compile and link the
native game; players need neither Python nor WSL. The normal signed Linux
package passes Build, remembered ISO-in-place Play, history and integrity repair.
The exact repaired executable also finishes an original World Tour race, saves
its unlocks and reloads the earned profile in a fresh process. Race, Crash and
Road Rage routes have scoped checks; this does not prove a complete campaign.

Windows-under-Wine checks cover original Vulkan gameplay, saves, controls,
window resize/close and a full three-lap race. Actual Windows host tests verify
owned restarts, saves, history and basic real-window access inside the production
confinement. Normal v14 signed Build, readiness and integrity repair also pass on
actual Windows, preserving all 487 Rust objects through repair. Physical
original-game Vulkan, sound and controllers remain qualification gates. The signed
profile is offline; upstream System Link/online modes remain future work.

See [preparation and native archive attachment](rust-port/README.md),
[standalone Windows setup](rust-port/WINDOWS_BUILD.md), and the
[current gameplay validation scope](../../docs/native-windows-gameplay-validation.md).

The historical `native-gen/` replacement CPU/runtime experiments below are
preserved as development evidence; they are not this port's delivery path.

## Staged development records

The component measurements and experiments below retain their original scope.
Statements about unfinished integration describe those earlier stages; use the
current delivery summary and linked gameplay validation for player requirements.

## Verified source and disc

`upstream-lock.json` pins Takedawn 0.2.0 and its matching Xbox recompilation
toolkit. `ntsc-u-retail.manifest` is upstream's public list of 716 file names,
lengths and SHA-256 hashes. Its MIT notice is in `Takedawn-LICENSE.txt`.
The manifest contains no game assets or executable code.

The native GamePKG command

```sh
gamepkg check-xbox-disc "/path/to/Burnout 3.xiso.iso" --json
```

opens the selected image read-only, indexes its XDVDFS directories, and hashes
every required file through a fixed 64-KiB buffer. XISO and known full-disc
partition layouts are recognized. Lengths use explicit 64-bit arithmetic;
numeric disc fields are little-endian. Directory cycles, overlapping entries,
duplicate case-insensitive paths, invalid links, unsafe names and truncated
file extents fail before a file is exposed. Image names never become host paths.
Nothing is extracted, installed or executed by this command. A successful
inspection does not grant an immutable-media guarantee for a future running
engine. The separate verified stream API below supplies owned, checked reads;
connecting it to the engine remains part of that integration.

On 2026-10-20 the user's selected XISO passed all 716 hashes and 2,476,107,072
file bytes, with no extra files. Its XBE matches the pinned North American
retail executable. The existing ISO is sufficient; its accompanying 7z is not
needed. No game files are copied into this repository.

The same complete verification also passes in an actual Intel 80386 PE32 CLI
under a private Wine prefix, including reads beyond the 2-GiB boundary. It
returns the identical file count, total, manifest and executable hash. This
qualifies this reader's Windows 32-bit path, not 32-bit Takedawn gameplay or
physical Windows hardware. The three synthetic reader tests, full GamePKG
release suite (757 passes, zero failures, 159 explicit ignores), strict lint
and formatting pass locally.

## Owned verified disc streams

`xbox_disc::VerifiedDisc::open` admits the complete pinned disc and retains its
read-only file handle, file metadata and SHA-256 hashes for 64 KiB file blocks.
The hashes are collected from the same bytes used for each full manifest hash.
Only manifest files and their directories are exposed; no disc name becomes a
host filesystem path. Separate streams have independent seek positions and
explicit-offset reads, including concurrent Windows/Linux reads beyond 2 GiB.
Seeking outside one file, unsafe paths and unlisted files fail explicitly.

Every cache miss reads a complete block into owned memory and checks its hash
before returning requested bytes. Cross-block reads check both blocks before
changing the caller's buffer or the stream position. Two cached blocks keep
their original checked bytes: repeated small reads avoid disk/hash work, and
can safely return those original bytes even if the underlying file was later
changed or truncated. A changed uncached block fails before exposing bytes.
Replacing the selected pathname cannot replace the retained open file handle.
Fresh admission still checks the whole selected image.

Reservations come from a caller-supplied game budget. Opening reserves 32 MiB
for bounded untrusted indexing; the final index/hash/metadata owner retains 4 MiB.
Each stream reserves two 64 KiB buffers plus 4 KiB for ownership/metadata. Failed
admission/allocation releases its reservations; failed reads retain their
stream's buffers for retry. Streams keep the shared index charge alive after
the outer media object is dropped. No existing game
memory allowance or execution deadline is increased by this API.

Seven new synthetic stream tests exercise all cache-page transitions,
same-size/same-timestamp tampering, truncation, atomic cross-block failure,
concurrent reads, budget exhaustion/lifetime, directory scope and path
replacement. They pass locally; the Unix path-replacement case is separate
from the nine total reader/stream tests executed in an actual PE32 Windows
process under Wine. The full private-image test streams all 716 files and
2,476,107,072 bytes again and matches every full manifest hash on both hosts.
All 56 directories and 716 files match the independently enumerated original
reader; that disc has no empty directories. The first local measured admission
and full stream comparison each take about 2 seconds, retaining 4 MiB afterward;
these are media checks, not end-to-end game performance claims.

After this component, the full local release suite passes 764 tests with zero
failures and 160 explicit ignores across 80 groups. Strict all-target/all-feature
lint and formatting pass. The private disc gate is run separately, not counted
as passed merely because it is ignored by the ordinary suite.

Reproduce the private full-image gate with `GAMEPKG_TAKEDAWN_DISC` set to the
selected ISO and `cargo test --release --test xbox_disc_media -- --ignored
--nocapture`. The ISO is opened read-only. No Python is used by this native
reader or stream API. The private engine bridge below now exercises these
streams in the real engine. Runtime deadlines, complete device semantics,
production isolation and playable signed package integration remain open.

## Native engine media bridge

`media-bridge/` builds a small host-only Rust static library from the same
reviewed reader modules, without linking the full GamePKG runtime. Its C ABI
supports bounded file/directory handles, metadata, independent file positions,
checked reads, directory cursors and explicit close/shutdown. At most 128
handles are admitted. IDs never recycle during the process lifetime, including
after shutdown/reinitialization. Bad paths, invalid handles, insufficient
directory output buffers and writes do not silently succeed. Panics cannot
unwind across C. Every caller pointer must satisfy the documented C contract;
this trusted-host interface is not an API for untrusted guest pointers.

The Linux adapter forwards the pinned engine's ordinary file/stdio/directory
calls for two explicit disc roots into this ABI. Reads larger than 64 KiB are
assembled from checked stream reads, preserving ordinary regular-file behavior.
SDL's file reader also uses the verified stdio path, covering original font
assets. Files outside those roots keep their original host behavior: this
transport alone is not a filesystem sandbox. CMake attaches the adapter to the
private pinned build without editing upstream sources and emits a separate
`burnout3-gamepkg-media` executable, preserving the original reference binary.
The private development entry currently reserves a separate 64 MiB media budget;
the final launcher must charge this against the signed game's actual shared
ceiling. No signed package or player-facing engine is registered yet.

Fifteen release Rust tests and strict lint pass for this standalone library.
The real C integration gate admits the ISO, enumerates all 716 files and 56
directories, checks metadata, bounded handles and stale IDs, and performs four
concurrent readers. Its POSIX variant also exercises normal fopen/fread/fseek,
an entire XBE read exceeding 64 KiB, file/directory calls, read-only errors,
traversal rejection and original host-path forwarding. The root GamePKG reader
tests pass after making the shared module's source path explicit.

The actual pinned engine boots from the read-only ISO inside a private Linux
namespace with networking disabled, writable temporary saves/data/cache, and
no real user library or assets extracted. It loads the 4,308,992-byte XBE,
initializes the NVIDIA Vulkan device, loads original fonts and reads the intro
movie streams. A separate 20-second run produces nonblank 640x480 framebuffer
dumps: the Criterion intro logo and the original loading screen are visually
confirmed. These copyrighted diagnostic frames stay in the private temporary
directory and are not published. The tests use a dummy audio device; audible
sound is not verified by them. Headless --seconds does not close this runtime,
so the external 20/45-second bounds terminate with expected timeout status 124.

The initial boot exposed upstream mapping general Xbox hard-drive Partition1
cache paths under its game-data folder. `adapt_paths.cmake` now stages the exact
pinned path source with a dedicated writable `save_dir/Partition1` mapping and
preexisting `Partition1/Cache` directory. UDATA/TDATA rules retain precedence;
DVD paths remain read-only. It refuses changed source and never overwrites the
original. A separate CMake gate checks valid staging, changed-source rejection,
output preservation on rejection and refusal to overwrite the upstream file.
The source SHA-256 remains unchanged after compilation and actual gameplay.

Fresh private boots now create both LocalCache00.bin and LocalCache08.bin
successfully in that HDD cache folder. They reach the original title screen,
new-profile screens, main menu, Single Event mode selection, car selection,
training movie and Silver Lake race. A bounded 180-second Vulkan run, with
normal scripted controller input, drives the actual race from ISO resources.
Private frames visually confirm the car, original world/HUD and crash/Impact
Time sequence. The original drive-log hook records 6,611 race frames, a maximum
62.7622 m/s, 3,778 frames of full throttle, 120 of full brake, and 1,799 with
boost-button and throttle input together. Both stick directions are exercised by the pad
script; the drive log does not record stick axes, so it alone does not prove
steering equivalence. The final frame continues racing after crash recovery.
This run terminates at its external bound with expected timeout status 124.

The initial race test creates original title/save-image metadata but continues
with autosave disabled. The separate persistence gate below now verifies a
normal saved profile and reload. Physical test audio output remains dummy;
mixed PCM is captured and checked separately below. Disc fstatvfs/mapping/other
device operations still need an explicit semantics audit. A driven first race
does not prove full modes/progression, console fidelity, runtime resource
enforcement, platform coverage or measured optimization gains. All completion
gates below still apply to the full requested port and every current port.
Reproduction commands and the remaining bridge work are in media-bridge/README.md.

After the HDD adaptation, all 29 upstream subsystem tests in the 22 actual test
directories are rerun: 27 pass, one is skipped and the same Vulkan backend smoke
test fails with the baseline clipping, partial-clear occlusion and four
render-target ownership discrepancies. No additional subsystem failure appears;
the existing rendering failure remains an open fidelity issue, not a waived gate.

## Rendering reference correction

Investigation of that baseline finds a faulty smoke fixture: its fixed-function
screen-space scenes never initialize the composite transform matrix. The runtime
starts that context at zero; inline quads collapse rather than drawing. Two
equally empty CPU/GPU frames passed the first four whole-frame comparisons,
while later clipping, occlusion and render-target checks reported failures from
their missing draws. This was not evidence that all those Vulkan features were
broken. The original source remains untouched.

`media-bridge/adapt_smoke.cmake` stages the exact pinned test source with explicit
identity-matrix setup. It also adds independent known-color probes for the solid
quad, background and fractional quad in all four basic scenes. All original
whole-frame equality checks and expected pixels remain, with no tolerance change.
The `GAMEPKG_SMOKE_OMIT_COMPOSITE=1` negative control restores the broken setup
and fails the new solid-quad probes on both paths, plus the old clipping,
occlusion and ownership cases. The existing disabled-alias-check negative
control still catches stale render targets after real draws are restored.

With correct setup, clipping probes, exact 20,000-sample GPU occlusion across a
partial clear, and every render-target ownership check pass. CPU occlusion is
20,200 rather than 20,000, a separately documented shared-triangle-edge coverage
gap in the reference rasterizer. The shader scenes now actually draw and expose
three remaining exact comparisons: checker, YUY2 and blend. They stay failing.
The complete subsystem run remains 27 passes, one failure and one skip.

The independent `check_checker.c` oracle computes the fixture's ideal affine
nearest sample using integer rational arithmetic, without calling either
renderer. In the 180x180 checker region the CPU matches every ideal pixel;
Vulkan differs at 1,392 pixels, at columns 462/507/552/597 and rows
62/107/152/197, where ideal coordinates land exactly on a texel boundary. This
localizes the discrepancy; it does not prove original NV2A hardware behavior.
The oracle has a nonzero exit until both paths match. Nearest filtering's
integer-coordinate operation is specified in the official
[Vulkan sampling source](https://raw.githubusercontent.com/KhronosGroup/Vulkan-Docs/main/chapters/textures.adoc).
Interpolation precision and original hardware comparison still need review.

A temporary `precise gl_Position` experiment makes no change to those errors
and is removed. No sampling-coordinate bias, filter change, image tolerance or
runtime-default workaround is introduced. The final private game executable
is byte-identical to the one used for the driven race:
`ed5d8a82bb5976eec7d38a36e109e13e6c6f3ad8053eb49a2e12e5c6eb16eae1`.
Only the stronger staged test and analysis tool change this round.

## Original profile persistence and mixed audio

The original profile-save flow now passes in the private native build. The
earlier test pressed START again after skipping the film, advancing the profile
screens before the following button sequence and continuing without autosave.
Using the actual menu sequence reaches the save prompt, selects Yes and a
profile slot, and creates Profile 1 through the original code. The game writes
59,904 profile bytes and 54 SaveMeta.xbx bytes via its existing atomic save guard,
then visibly reports Save successful. No engine save function is bypassed,
forced to succeed or replaced with host-generated profile data.

A new process discovers the saved slot, displays its original name/date/play
time/unlock/medal fields, reads all 59,904 bytes successfully and returns through
the original load flow to the main menu. Full profile/metadata hashes remain
unchanged after this reload. Only fresh private test scopes are used; the real
GamePKG library, user saves and disc image remain untouched.

`tools/check_takedawn_session.sh` makes this a repeatable maintainer gate, with
no Python dependency. It creates a new /tmp evidence directory, launches each
session inside a network-disabled namespace with a cleared environment, exposes
the selected ISO read-only, and gives the engine its own writable save/cache
scope. It waits for actual file events, consumed input and game-frame progress
before pressing the next button. Two separate processes create and reload the
profile; abnormal early exit, event/frame stalls and the external per-process
deadline fail the gate. Completed sessions are deliberately stopped and reaped.
Private frames remain available for checking the actual screens; log assertions
alone are not claimed as a universal visual or progression oracle.

The same gate captures exactly 45 seconds of the engine's mixed audio before
the dummy output device: 48,000-Hz signed 16-bit stereo, 2,160,000 frames and an
8,640,044-byte WAV. Both channels and the overall mix have finite non-silent RMS.
The first manual recording measures channel RMS approximately -27.38/-28.15 dBFS
and peaks -10.29/-10.82 dBFS, without sample clipping. The automated gate repeats
the format/length/non-silence checks successfully in a second fresh scope. This
proves mixed output exists, not physical speaker playback, timing equivalence,
surround routing, or full original audio/music/film fidelity. All recordings,
profile files and diagnostic frames stay private and are never package assets.

The game executable remains the same previously race-tested binary. Default
profile save/reload and bounded mixed stereo capture are verified components;
campaign progression/autosave/restore and broader save compatibility still need
coverage. The signed GamePKG, native generation without Python for players,
remaining fidelity/platform work and measured heavy optimizations across all
current ports remain part of the complete goal.

## Private reference generation

The new native Rust frontend is under native-gen/. It reads the verified XBE
in memory, validates its layout and ports the instruction/operand model,
linear/resynchronized decoding, reachable CFGs and basic-block construction.
All24,000 original reference candidates are accounted for:23,999 backed ranges
match903,343 instructions/178,231 blocks in linear mode and810,177/161,530 in CFG
mode. One DSOUND alias range crosses into BSS and is explicitly rejected, not
counted as generated or silently ignored. Full reports match byte-for-byte on
Linux64 and actual PE32 Windows under Wine. Six frontend tests pass on both;
strict lint/formatting and the media library's15 tests pass. The native tool
links no Python runtime. Its full scope, limits, oracle caveats, unresolved
candidate and reproduction commands are in native-gen/README.md.

The native tool now includes a typed data/stack instruction-to-C emitter using
the original runtime's TLS registers, local EBP and logical memory interface.
Twenty-one architectural cases pass against the pinned runtime header on
Linux64, actual ELF32 and actual PE32 under Wine, including partial-register
aliasing, FS,16bit addresses and ESP-dependent stack operations. The full
reference CFG inventory accepts375536 instruction forms and explicitly rejects
434641 unsupported forms; this is coverage, not whole-function correctness or a
game-performance result. No game-derived C is saved by that diagnostic.

The next emitter stage adds scalar integer arithmetic, comparisons, logical
operations and carry/borrow flags. Its synthetic hardware oracle passes1408000
actual-x86 comparisons plus5 memory/alias cases on Linux64, actual ELF32 and
actual PE32 under Wine, using the unchanged pinned runtime header. The extended
inventory diagnostic now accepts500913 forms and rejects309264 unsupported
forms, preserving the same1 rejected reference extent. Eager-to-original-lazy
flag transitions, full control flow/ABI, remaining instructions and runtime
fault/SEH integration are still required; neither these counts nor the oracle
prove a complete native game generator or a game speed gain.

Closed scalar leaf-function emission now connects those flags to all16
conditions, SETcc/CMOVcc, checked internal branches and loop counters, local
EBP frames and near returns under the original C-call/guest-stack convention.
Its8192 condition/branch/counter cases and additional frame/memory cases pass
on Linux64, actual ELF32 and actual PE32 under Wine against the pinned runtime
header. The complete private inventory yields1174 accepted closed functions
and22825 rejected functions, plus the same1 invalid extent; generated C is
discarded immediately. This is emission coverage, not game-equivalence proof.
Calls, indirect/external transfers, full ABI/SEH/flags interoperability, remaining
instructions and original recovery/patches still prevent complete generation.

Direct calls and tail transfers between explicitly bound eager-flag native
functions now preserve guest return addresses, ESP/EBP and callee cleanup.
A closed-program builder rejects unresolved or invalid callees before exposing
successful source. Its13-function synthetic program passes on Linux64; actual
ELF32 and PE32 additionally match130 original synthetic x86 native-stack
call/return comparisons. The dependency-only private inventory report finds3851
provisional functions/6109 edges,20148 emission rejections and2451 transitive
dependency rejections, leaving1400 dependency-closed functions. Linux64/PE32
reports match exactly, but game compile/link, alias recovery, remaining
instructions/patches and full ABI/runtime integration are still pending. The
diagnostic discards generated C and does not claim a playable port or speed gain.

The exact-bit SSE1 data stage adds scalar/packed/half-register transfers,
bitwise operations, shuffles/unpacks and sign masks through the original TLS
XMM representation.44800 actual-hardware comparisons,4 failed-load cases and
a complete vector-carrying CALL/RET program pass on Linux64, actual ELF32 and
actual PE32 under Wine against the pinned header. Legacy aligned memory forms
require proven constant alignment until a guest-GP bridge exists; no permissive
unaligned substitute or floating arithmetic is used. The private dependency
report now has12602 provisional functions and9880 dependency-closed functions;
the remaining11397 emission failures/2722 dependency failures and1 invalid
extent are explicit. Linux/PE32 reports match. Original patch/recovery, full
memory faults, arithmetic/ABI/runtime/platform and playable-package gates remain
open, as do measured heavy optimizations across all current ports.

Scalar shift/rotate lowering now passes4205981 actual-hardware comparisons on
Linux64, actual ELF32 and actual PE32 under Wine, including count masks, carry
rings, preserved flags and memory read/modify/write. Linux also verifies the
zero-count read-only memory fault. Undefined flag images are retained rather
than claimed as PIII-specific results; dynamic/oversized word double shifts
still require range analysis. The private dependency report now accounts for
13300 provisional functions,10699 emission failures and2992 dependency failures,
leaving10308 dependency-closed functions plus the same1 invalid extent. Reports
match on Linux64/PE32. Counts remain emission evidence; complete streamed
generation, recovery/patches, runtime integration, gameplay/platform fidelity
and measured heavy optimization across every port remain unfinished.

The supported10308-function selection now also streams into private diagnostic
C without retaining all functions. Its111 shared-tail entries are checked for
identical shared bytes and instruction boundaries; ambiguous crossing ranges
still fail. Two-pass input/source hashes detect changed input, and failed
stages are discarded. Linux64 and actual Windows32 tools emit byte-identical
source; the full partial module compiles at O3 with strict warnings into ELF64,
ELF32 and i386 Windows COFF objects. Each has10308 function definitions and no
unresolved generated-function calls. This does not link or execute the complete
game. One Linux generation including disc verification measured3.15seconds and
16732KiB peak RSS; this is a tool measurement, not a gameplay optimization.
All27 host and21 Windows-tool tests pass. Private output restrictions,
reproduction commands, hashes and remaining requirements are in
native-gen/README.md.

Integer multiplication now adds another175 dependency-closed functions, bringing
the supported selection to10483. All implicit MUL/IMUL widths and word/dword
truncated IMUL forms pass1760104 actual-hardware product/defined-flag comparisons
plus15 alias/address/load-failure checks on Linux64, actual ELF32 and actual
PE32 under Wine. Signed products avoid C overflow/narrowing/shift assumptions;
undefined flags retain their prior image rather than claiming original PIII
values. Linux/Windows32 produce identical expanded source, compiling at O3 with
strict warnings into all three object targets with10483 definitions and no
unresolved generated-function symbols. All30host/23Windows-tool tests and
strict lint/formatting pass. This remains a partial module: complete guest
exception handling and the remaining complete-game generation/integration gates
are unfinished. Reproduction, scope and hashes are in native-gen/README.md.

DIV/IDIV now use checked unsigned arithmetic, explicit signed magnitudes and
range validation, preserving guest registers on divide errors without invoking
undefined host division. A concrete nested, thread-local integer fault boundary
reports faulting EIP, current local EBP and integer state; it exits generated
call frames without executing subsequent instructions. All three execution
targets pass12633635 actual-hardware successful result comparisons and21133789
checked guest-fault cases. Linux64/ELF32 also prove15 representative physical
SIGFPE cases; Windows does not run that oracle. Nested generated calls verify
the original guest return slot/ESP and exact fault location/state, and8threads
preserve1000 independent contexts each. This does not implement guest SEH
dispatch/recovery or memory/FP exception context. The supported module now has
10629 functions and116 validated shared-tail entries, emits identical source on
Linux64/Windows32, and compiles/relocatably links its real fault service into
ELF64/ELF32/i386 COFF with no unresolved generated-function or fault-service
symbols. All33host/25Windows-tool tests and strict checks pass. Complete native
generation, signed integration, broader fidelity/platform gates and measured
heavy optimization across every current port remain unfinished.

SSE1 scalar comparisons and MXCSR control now have portable integer-based
services. Guest control state uses the Pentium III mask rather than inheriting
host DAZ/AMD-MM extensions. NaNs, denormals, comparison flags, masks, sticky
status and failed-operation state pass537088 hardware comparisons,32768 control
round trips and6400 mask cases per execution target. Linux64/ELF32 additionally
capture21 physical fault contexts on this AMD host, selecting reserved-bit
tests from its actual FXSAVE mask while keeping guest rejection strict.
Windows32 arithmetic/control and8thread isolation gates pass; Windows does not
run the physical signal-context oracle. The partial supported module now has
10660 functions,117 checked shared tails and concrete comparison/control/fault
services linked into ELF64/ELF32/i386 COFF, with matching Linux/Windows32 source.
All36host/27Windows-tool tests pass. This does not complete SSE arithmetic,
conversion, control-register mode selection, full SIMD/x87 exception context,
guest recovery, signed player integration or all-port optimization.

Portable binary32 add/subtract/multiply/divide/square root now use a reviewed,
pinned SoftFloat3e subset with integer operations and explicit SSE exception
staging. All718080 direct/218880 emitted comparisons,2640 physical Linux
outcomes and8000 threaded comparisons pass at the qualified targets. A720720-case
actual-x86 corpus, including faults, also matches a real big-endian MIPS32
executable under QEMU and actual PE32 Wine. This proves the component's portable
behavior, not a full MIPS game or console fidelity. The supported selection
increases5681 functions to16341 with146 checked shared tails; source matches
Linux/Windows32 and compiles/links concrete services into all three object
targets. All40host/30Windows-tool tests pass. Exact pins, BSD3 attribution,
source adaptations and repeatable asset-free CI/manual gates are included.
Complete discovery/recovery, remaining SSE/MMX/x87, full exception/ABI/patch/
dispatch/source-kit integration and heavy measured optimization remain open.

It does not yet generate the complete game or replace the original
generator below. Function discovery/recovery, ABI analysis, remaining semantic
lifting (including flags, control flow, SIMD and floating point), all configured
patches/spin waits, source/C/dispatch emission and player build integration are
still required for the complete no-Python local build path.

`tools/check_takedawn_regen.py` is a maintainer reference tool, not a player
dependency. It verifies both pinned checkouts and the complete disc, reads the
XBE independently through upstream's image reader, checks its hash again, and
places only that executable in a sealed RAM file while the original generator
runs. The private checkout receives a temporary descriptor link, removed when
the process finishes. Generated game-derived C remains private and must never
be published with the source kit or packages.

The first reference run translates 24,001 functions, applies 108/108 configured
patches and two spin-wait sites. It reports 15 unresolved call targets; these
must be classified and, where reachable, implemented and tested. A generated
stub is not proof that the corresponding game behavior works.

Players must not require or run Python. The original generator is Python-based,
so a native generation path with reference comparisons is an open requirement.
The public AppImage is useful for upstream comparison; wrapping it as the
GamePKG port would not satisfy local source compilation or resource isolation.

## Reference native compilation

The pinned game and toolkit compile locally with Clang/LLD 18 at the unchanged
upstream Release settings. Vulkan, the full overlay, alternate controller glyphs
and all runtime/game test targets are enabled. A standalone temporary dependency
prefix supplies glslang 15.1.0, SPIR-V Tools 2025.1 and librsvg 2.58.0; no global
packages were installed. The build needs that prefix in `CMAKE_PREFIX_PATH`, its
library directory in the link search path, and the SVG tool on `PATH`. LLD is
required for the current Linux static-library dependency cycles; GNU ld leaves
texture-replacement references unresolved in a test target.

The built reference executable is 32,729,224 bytes. Its `--version` reports the
pinned revisions, disc manifest, generated-tree hash and all 108 patches. This
is a private baseline executable, not a GamePKG-installed or isolated game.
Neither this binary nor the generated source is a public package artifact.

The upstream top-level build exposes no tests through a root `ctest` invocation;
each generated `CTestTestfile.cmake` directory must be run explicitly. On the
local NVIDIA Vulkan device, 29 tests in 22 directories give 27 passes, one skip
and one failure. The failing `nv2a_backend_smoke` reports clip-draw boundary,
partial-clear occlusion and render-target ownership discrepancies. These are
recorded failures at the pinned baseline, not waived or hidden by changing test
tolerances. Successful compilation and these subsystem tests do not prove boot,
racing, campaign, console equivalence or full port fidelity.

## Completion gates

- A pinned, reviewed source kit and a native local code generator, retaining
  every required function and configured patch; clear unsupported-build errors.
- ISO data read in place through owned bounded reads, including concurrent
  streams and directory enumeration. Preserve the generic ROM size checks;
  this multi-gigabyte disc needs an explicit reviewed resource profile.
- A signed game recipe, creator verification, remembered disc selection,
  integrity/build receipts and game updates, delivered through GamePKG_Ports.
- A separate native game process and Vulkan window, GamePKG play history,
  input/controllers/touch, complete audio/music/films and original settings.
- Host filesystem/network/process isolation; creator/game-scoped saves,
  configuration and caches; denied or unavailable operations report failure.
  Original settings, files and save data outside the selected scope stay private.
- Original boot/menus, races, takedowns, Road Rage, Crash mode, split screen,
  progression, saves/imports and all supported content verified against the
  pinned upstream and, where required, the console. Classify original network
  functionality and upstream gaps rather than silently replacing them.
- Linux gameplay first, Windows next, then other requested platforms and
  architecture qualification. Upstream currently targets 64-bit hosts; a 32-bit
  disc inspector does not prove 32-bit game support.
- Timed repeated baseline/candidate runs for every current port, covering
  startup, simulation, rendering, audio, memory and representative dense scenes.
  Compare original behavior and pixels/audio where applicable. Retain existing
  execution limits and source permissions. Do not claim gains from compiler
  flags alone or trade missing features for speed.

The existing native pipeline already uses Rust level 3, native CPU tuning,
thin LTO and one codegen unit; native C/C++ uses level 3 and native CPU tuning.
Several portable Wasm paths use level 2. Further compiler changes, profile-based
optimization and renderer/algorithm work require benchmarks and fidelity checks.

The native generator now handles scalar signed32/binary32 conversions using
an integer-only runtime service, including rounding and staged SIMD faults.
The supported private selection is16,739 functions, up96 from the dynamic SIMD
alignment stage. Hardware and generated-code evidence, exact hashes and the
remaining full-port gates are recorded in native-gen/README.md. This remains
integration work rather than a complete playable GamePKG; no speed gain is
claimed for instruction coverage.

The next native runtime stage adds a shared physical80bit x87/MMX register
file, initial MMX integer/data operations and non-waiting x87 control/status
instructions. Hardware checks cover register aliasing across all TOP positions,
status clearing and deferred exceptions, rather than assuming independent
MMX/double files are equivalent. The supported private selection is16,776
functions. Full x87 arithmetic and remaining MMX operations, complete ABI/
exception/dispatch and signed GamePKG launch integration remain required;
native-gen/README.md records the exact qualified scope and open gates.

The extended-precision arithmetic foundation now compares five basic operations
against actual raw80bit x87 hardware across all supported precision/rounding
settings. Its1,023,600-case corpus also matches on big-endian MIPS32; thread
and32bit Linux/Windows component checks pass. This non-committing backend
preserves precision without host double/long double conversion. Architectural
stack/rounding-status/unmasked-exception/metadata integration is still required
before enabling x87 arithmetic instructions. No additional game function or
playability is claimed from the math-only gate; native-gen/README.md records
reproduction and remaining work.

Basic x87 register arithmetic is now connected to shared architectural state,
including pop variants, empty-stack responses, C1 and masked/unmasked deferred
exceptions. The generated-code oracle passes1,351,680 hardware comparisons and
585,174 next-wait fault checks on Linux64, actual ELF32 and PE32 Wine. Complete-
function selection stays16,776: newly accepted arithmetic sites still occur
alongside unsupported operations, especially FLD/FSTP and memory arithmetic.
The full-instruction diagnostic identifies those remaining barriers; exact
scope, masks, portable corpus and runtime-link evidence are recorded in
native-gen/README.md. Full signed GamePKG launch/playability remains unfinished.


Exact x87 floating/integer/register/constant loads now use the shared physical
stack and staged memory reads.1,736,704 hardware cases verify values, masks,
stack faults and metadata, with deferred faults, threads and real Linux fault
priority checks.21 additional MMX saturation, multiply, pack and unpack
operations pass an expanded1,433,720-case hardware fixture. Low unpack reads
exactly 32 bits, including immediately before a protected page. Linux64, actual
ELF32 and PE32 Wine component checks pass; full suites pass56 host/40 Windows
tests and strict lint/formatting.

The combined private selection grows to16,793 functions/154 shared tails.
Linux and Windows generators emit identical private source and reports; fresh
strict O3 ELF64/ELF32/i386 COFF modules link all instruction services. Other
host/kernel symbols remain external. Remaining MMX shifts and other operations,
x87 stores/memory arithmetic and the rest of the instruction/control/environment
set, full memory/SEH/kernel/thread/old-new ABI and signed GamePKG integration
still prevent a complete playable package. Readiness remains false and the
original private game prototype is unchanged. Exact scope/hashes/reproduction
are recorded in native-gen/README.md; no game assets or derived source are
published and no player Python dependency or game speed gain is introduced.


MMX immediate/register/memory shifts are now implemented with full 64-bit
counts and shared-state transitions even at count zero. All 455 current shift
sites emit. The 2,368-form fixture passes 1,343,488 hardware comparisons,
deferred/read-failure/thread checks and real Linux memory-fault priority
checks. Linux64, actual ELF32 and PE32 Wine produce identical synthetic result
corpora; all rows replay exactly on a big-endian 32-bit MIPS CPU under QEMU.
The expanded 16,814-function partial module and current instruction services
compile/relink on all three object targets; suites pass 59 host / 42 Windows
tests and strict lint/formatting. Remaining instructions and full runtime/ABI/
signed launch/game/platform qualification remain required; readiness is false.
The original private prototype is unchanged, with no assets or derived source
published and no player Python requirement. See native-gen/README.md for
reproduction, hashes, specific supported forms and remaining gates.


Eleven further MMX operations now cover average/min/max, absolute byte
differences, word shuffle/insert/extract and byte sign masks. All 82 current
sites for those operations emit. A 4,200-form fixture passes 1,848,320 real
hardware comparisons, including independent register/lane mappings, exhaustive
byte pairs, word boundaries, precise memory width, deferred faults and thread
isolation. Linux64, actual ELF32 and PE32 Wine produce identical result corpora;
all rows replay on big-endian MIPS32/QEMU with UB traps. Full suites pass
62 host / 44 Windows tests and strict lint/formatting.

The private partial module grows to 16,824 functions and links the new lanes
service on ELF64/ELF32/i386 COFF. Linux/Windows generators produce identical
source/reports. Streaming/masked memory, fences, mixed MMX/SSE conversions,
x87 memory arithmetic/stores and other instructions, full runtime/ABI/dispatch
and signed GamePKG/game/platform qualification remain required; readiness is
false. The original private prototype remains unchanged. Reproduction and
exact evidence are in native-gen/README.md, without publishing game-derived
source/assets or requiring player Python.


Mixed MMX/SSE conversions now preserve operand-specific state and exception
ordering. CVTPI2PS memory leaves x87 state alone; MMX forms enter after complete
reads and retain that transition if a SIMD exception suppresses the result.
417,792 hardware-based checks, 3,072 real Linux exception contexts, precise
memory-priority, pending-state and thread tests pass. Linux64, actual ELF32 and
PE32 Wine numeric corpora match and replay on big-endian MIPS32/QEMU; scalar
regression and full 65 host / 46 Windows test suites pass.

All 56 mixed-conversion sites emit, but remaining instructions/dependencies keep
the private complete-function selection at 16,824. The game body is unchanged;
current services relink across all three object targets and dedicated emitted
fixtures execute the new instructions. Streaming/masked memory, fences, x87
memory/remaining instructions, full runtime/ABI/dispatch, signed launch and
complete game/platform qualification remain open. Readiness remains false and
the earlier private prototype is unchanged. No game assets or derived source
are published and no player Python is required. See native-gen/README.md for
exact evidence and scope.


Streaming stores, masked stores, store fences and prefetch now use a concrete
checked RAM transaction layer. Complete-span admission prevents partial writes
on faults; MMX state changes follow measured ordering. Page-fault results carry
an address/error code. The 425,984-case hardware suite and Linux fault contexts
pass on native 64/32, with Windows 32-bit Wine value checks. Sanitizers, threaded
lifetime/transaction tests and a big-endian MIPS32 RAM suite also pass.

All 252 current streaming/fence/prefetch sites emit and private selection grows
to 16,852 functions/158 tails. All three object targets link current services.
The Windows retry uses Clang's emulated-TLS profile with GCC helpers; a reusable
cross-compiler executable check verifies 8,000 threaded fault contexts. Full
suites pass 70 host / 49 Windows tests. Exact forms, evidence, hashes and required
compiler flags are recorded in native-gen/README.md.

Host memory-map/device hooks and thread bindings, ordinary-wide-store/legacy
memory migration, remaining x87 instructions, full runtime/ABI and signed
launch/game/platform qualification remain open. Readiness is false and the
earlier private prototype is unchanged; no game assets or derived source are
published and no player Python dependency or game FPS gain is introduced.

Native x87 memory arithmetic now includes binary32/64 and signed16/32 forms
through checked whole-operand RAM reads. The 144-form synthetic suite passes
2,211,840 hardware cases, Linux64/ELF32 real fault-priority checks, Windows32
Wine compiler-boundary tests and big-endian MIPS32 saved-state replay. The
private closed module grows to 16,858 functions and links instruction services
on three targets. Remaining x87 stores/compare/control/transcendentals and full
engine memory/device/thread/ABI/package integration still prevent a complete
playable GamePKG release. See native-gen/README.md for exact evidence, limits
and reproduction; no game-derived source is shipped.

Native x87 comparisons now include ordered/unordered, EFLAGS, integer memory,
zero-test and stack-pop forms. The 115-form suite passes 7,247,872 hardware state/
flag cases, 369 Linux FPU fault contexts, both Windows32 Wine compiler profiles
and big-endian MIPS32 saved-state replay. The generator admits 486 more sites
and links 16,862 closed functions on ELF64/ELF32/i386 COFF. The precise measured
flag profile and physical Xbox qualification gap are recorded in the generator
README. Stores/conversions and remaining x87/engine/ABI/signed-package work still
prevent claiming a complete playable GamePKG release; original assets stay local.

Native x87 stores/conversions now cover register and floating32/64/raw80 plus
signed16/32/64 formats, including precise rounded-result/pop versus suppressed-
write exception behavior. The 76-form suite passes 4,487,168 hardware state/memory
cases, 412 Linux fault contexts, both Windows32 Wine compiler profiles and
big-endian MIPS32 replay. The checked RAM adapter supports complete ten-byte
admission/stores, with 74,112 portable access cases. Another 194 closed functions
can be generated, bringing the linked partial module to 17,056 functions.
Remaining x87 stack/control/environment/transcendentals and full engine/ABI/
signed-package integration still prevent claiming a complete playable release.

Native x87 stack operations now include exchange/free/TOP motion, sign changes,
classification, conditional moves, integral rounding and exponent extraction.
The 88-form suite passes 6,447,104 hardware state cases, 243 Linux FPU fault contexts,
both Windows32 Wine compiler profiles and big-endian MIPS32 full-state replay.
Another 105 closed functions become available, bringing the linked partial module
to 17,161. Remaining x87 control/environment/scale/remainder/transcendentals and
full engine/ABI/signed-package integration still prevent claiming a complete
playable release. Exact scope, corpus hashes and reproduction are in the native
generator README; original assets and derived sources remain local/private.

Native x87 control-word and legacy protected-mode context instructions now use
explicit checked save images.286,720 hardware cases match across Linux64/ELF32
and Windows32 Wine compiler profiles, with big-endian replay and whole-memory
rollback tests. Reserved PC01 arithmetic follows the measured extended profile.
The freshly compiled partial module grows to17,168 closed functions; Linux and
Windows generation agree byte-for-byte. Remaining numerical instructions, full
engine/CPU/SEH/ABI integration and a signed playable GamePKG still require work.
The native-generator README records the exact tested scope and reproduction.

Native FSCALE support now matches2,089,999 hardware state cases, including
pending exceptions and every integer scale−65,537..65,537. Linux64/ELF32,
Windows32 Wine compiler profiles and big-endian full-state replay agree.
All13 current game sites emit; their other dependencies still keep the closed
selection at17,168 functions. Runtime helpers relink across all three object
targets. This remains partial native-generation work; remainder/transcendentals,
full engine/CPU/ABI integration and signed playable launch remain required.

Native FPREM/FPREM1 support now matches4,136,989 hardware states, including
repeated partial reduction, quotient flags and exact tiny-result exceptions.
Linux64/ELF32, Windows32 Wine compiler profiles and complete big-endian replay
agree. Older hardware-oracle FXRSTOR clobbers are corrected, with prior numerical
corpora unchanged. The current game inventory has no remainder instructions,
so its17,168-function closed selection remains unchanged; runtime services relink
on three targets. Transcendentals, full engine/CPU/ABI integration and a signed
playable GamePKG remain open. Exact scope and reproduction are documented in the
native-generator README.

The native math foundation now includes portable wider intermediate arithmetic
for the remaining transcendental instructions.660,646 independent GCC reference
cases match across Linux64/ELF32 and integer-only big-endian/Windows replay.
Big-endian NaN payload accesses and a signed-shift issue are corrected and pinned.
This is internal numerical workspace; transcendental handlers and full
engine/CPU/ABI/signed playable integration still require work. The current
17,168-function partial selection remains unchanged.


F2XM1 now has a portable fixed-profile evaluator and shared-state runtime
service.146,304 state cases,44,064 queued faults and279,808 monotonicity checks
pass, with independent512-bit accuracy and pinned-profile checks for approximate
numerical results. Linux32/Windows32 Wine/big-endian32 replay agrees. This admits
five more closed functions (17,173 total); freshly compiled partial modules link
on all three object targets. Original CPU bit-for-bit approximation and full
engine/CPU/ABI/signed playable integration remain open. Licensing, reproduction,
exact scope and evidence are in the native-generator README.


FYL2X now supplies portable log2/product evaluation and the original two-input
stack/pop behavior.1,585,260 state cases,650,192 deferred faults and133,116
monotonicity pairs pass, alongside independent accuracy and reference checks.
All30 game sites emit and the partial selection reaches17,191 functions.
Linux/Windows generation agrees and fresh three-target modules resolve the
numerical services. Full engine/CPU/ABI integration, remaining transcendental
instructions and signed playable launch remain required; exact scope is in the
native-generator README.


FPATAN now has portable quadrant-aware evaluation and shared stack/fault
integration. A tiny-ratio correction preserves directed rounding and underflow
at the minimum-normal boundary.2,409,580 state cases,1,085,424 deferred faults
and264,192 monotonicity pairs pass independent profile/accuracy/physical checks;
full replay agrees on Linux32, Windows32 Wine and big-endian32. All57 game sites
emit and the partial selection reaches17,199 functions, with freshly compiled
three-target runtime modules. This remains component progress: remaining
instructions, complete engine/CPU/ABI integration and signed playable launch
are required. Exact tested scope is in the native-generator README.


Standalone FSIN/FCOS now have portable legacy-pi reduction, numerical evaluation
and shared unary stack/fault integration. Tiny directed rounding and wrapped
underflow are corrected;1,077,056 states/447,552 deferred faults and8,704 local
monotonic pairs pass. Accuracy checks separate approximation error from the
known large-argument phase drift. Full replay agrees across tested width/
compiler/endian profiles. All128 game sites emit and fresh partial modules
reach17,231 functions. Combined FSINCOS/FPTAN and complete runtime/CPU/ABI/signed
playable integration remain open; exact evidence is in the native-generator
README.


Combined FSINCOS/FPTAN now use separate numerical paths and checked stack pushes,
including masked/unmasked faults and wrapped results.1,077,056 states/452,576
deferred faults and13,056 local monotonic checks pass, with complete32-bit and
big-endian replay. Tiny-tangent approximation differences are explicitly tracked
and bounded. All9 game sites emit; the fresh partial module reaches17,233
functions. Full runtime/CPU/ABI integration and signed playable launch remain
required. The native-generator README records exact scope and evidence.


FYL2XP1 now preserves tiny log1p inputs and shares binary stack/fault integration.
Constructed rounding regressions also fix a lost closing-stage flag in FYL2X and
tiny F2XM1.1,912,940 new-operation states/780,384 deferred faults and expanded
prior-family corpora pass width/compiler/endian replay and independent accuracy
checks. The game has no FYL2XP1 sites, so the verified17,233-function body is
unchanged; current runtime services relink on three targets. Full CPU/context/
ABI/engine integration and signed playable launch remain required. Exact scope
is recorded in the native-generator README.
