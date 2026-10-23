# Zelda3 native GamePKG port

The full objective is a playable signed GamePKG using a player-selected Zelda:
A Link to the Past USA ROM directly. Neither the ROM nor its decoded resources
belong in the package. Decoding stays in memory; no extracted asset directory,
`zelda3_assets.dat`, graphics, music or intermediate YAML is written to disk.

The engine is pinned to snesrev/zelda3 revision
`45a149d8b4bb8098f53d1009f76e76e7d8b79bbf`. `upstream-lock.json` records the SHA-256
of each copied source and license. Copyright and MIT notices, including the
LakeSnes authors' notices, remain in `upstream/LICENSE.txt`.

Default Build compiles the pinned engine directly for the local Linux CPU, requiring Clang 18 and lld-18. The separate Build Wasm retains the WASI toolchain and bounded-memory engine described below. Both keep ROM decoding in RAM and use scoped host operations.

## Verified behavior

- The selected USA 1 MiB ROM is recognized; a 512-byte copier header is supported.
- The resource decoder constructs all 165 engine resource records in RAM.
- The native Rust decoder has no file IO, process launch or interpreter calls.
- All 165 resources match the pinned reference loader byte for byte.
- The source ROM's checksum is unchanged after decoding.
- All selected C translation units pass Clang syntax checks before compilation.
- The full gameplay/PPU/DSP engine builds into a 16 MiB maximum-memory Wasm module.
- The resulting module has **zero imports**, including no WASI/file/network imports.
- With the user's ROM, 1,000 actual engine frames reach the title screen and
  generate audio. The local screenshot is `target/zelda3-proof/engine-title.png`.

`adapters/engine.c` replaces the SDL shell and native save/audio integration. It
retains normal SPC/DSP music, disables external MSU music, and removes desktop
replay logging from the tick path. Native libc diagnostic descriptor operations
fail closed inside Wasm. The signed `ports/zelda3` GamePKG now builds and plays through the
native GamePKG runtime. Its Rust entry invokes only the checked host engine API.

Audio preserves the original DSP's left and right channels: 800 stereo frames
at 48 kHz per host tick. Native packets and Wasm memory reads carry channel
metadata through the shared output. Rebuild earlier Wasm caches for the current
renderer and stereo output; SRAM bytes remain compatible. Both runtime targets are verified by the actual
ROM gameplay/save test and a 1,000-frame channel-separation test. Set
`GAMEPKG_ZELDA3_TEST_TARGET=native` or `wasm` to select the target explicitly.

ROM decoding runs inside the native GamePKG executable on every platform. Users
need no Python, Pillow, PyYAML, script interpreter or external decoder. Pinned
upstream Python scripts remain development references only. The native compiler
also invokes Clang directly, without a Python build script.

## Isolation and integration

The pinned engine builds in the isolated compiler and carries an integrity
receipt alongside the Rust entry. Package-authored Rust keeps the existing
macro, alias and indirect-call restrictions; packages cannot supply arbitrary C
or Python. The engine has no filesystem or network imports. Only the ROM
selected by the player is passed to the host-owned RAM decoder.

Pixels and audio pass through checked host operations. Both Wasm instances share
the host frame deadline, reserve memory under the combined hard
limit, and persist the original 8 KiB SRAM through creator-scoped storage.
Failed frame saves use the runtime's existing rollback behavior.

Local tests cover signed installation/building, invalid and headered ROMs,
1,000 runtime frames, file/name creation, opening dialogue, movement, SRAM
persistence and reopening. Native Linux GUI tests select the ROM through the
library and verify visible held-key movement. Windows CLI and GUI tests under
Wine cover compiler exchange, playback, saved-slot loading and keyboard input.
Source and signed packages are published in Centrexity/GamePKG. The 0.1.6
release includes native CLI and GUI hosts plus the four-game catalog and store.

## Build and play

Use the [GamePKG 0.1.6 release](https://github.com/centrexity/GamePKG/releases/tag/v0.1.6).
Players can download the signed ROM-free package:

```sh
gamepkg install https://raw.githubusercontent.com/centrexity/GamePKG/main/html/demos/zelda3.gamepkg --trust-creator
gamepkg build com.gamepkg.zelda3
gamepkg-gui
```

Players do not need a signing key. For creator development, the source recipe is
in `ports/zelda3`; sign it using your own private creator key before
installation.
Select your supported USA ROM in the game's ROM field, then press Play. Arrow
keys or WASD move; Z/X are B/Y, C/V are A/X, Enter is Start, Tab is Select, and
Q/E are L/R. The ROM remains in its original location. SRAM saves go to the
library's scoped save storage, not beside the ROM.

The port currently needs Linux, Clang 18 plus a Wasm C sysroot for isolated
source builds. Playback uses the built-in Rust ROM decoder.
Set `GAMEPKG_ZELDA3_SYSROOT` to the prepared sysroot. Memory is
reserved as 16 MiB for each Wasm instance under the combined 32 MiB limit, and
both frame execution and engine initialization use the host deadline.

Windows compiler exchange now transfers and checks the engine artifact and its
receipt. The actual Windows CLI and GUI have passed local Wine tests with the
player-selected ROM, including 1,000 frames/audio, ROM selection, saved-slot
loading and keyboard movement. This is Wine evidence, not a claim of real
Windows hardware coverage. No separate decoder runtime is needed. WSL is used only for source
compilation, not playback. The compile request includes no ROM or save data.
A browser ROM decoder remains outside this native port; Zelda3 web export currently gives a clear
unsupported-feature error instead of producing a broken player.

For a separate WASI installation, use **Settings → C/WASI build tools folder**, check the tools and save settings. This persists across app restarts without requiring a launch-time environment variable. Engine-specific sysroot environment overrides take precedence.

The adapter preserves the original cancellation of simultaneous Up/Down and
Left/Right presses before running a frame. The source-only maintainer check
`tools/check_zelda_input_reference.py` compares all 65,536 input combinations to
the pinned original. `tools/check_zelda_inputs.py` builds private native and Wasm
fixtures with an explicitly selected ROM and checks direction registers,
gameplay, stereo and SRAM reload. These are development tools; players need no
Python. Desktop options listed in the parity tracker still need implementation.

Signed recipes may also use `[zelda_video]` with `extra_width` (0–96 pixels per
side) and `extend_height` (224/240 lines). The original widescreen visibility
features accompany nonzero widths. Both native and Wasm preserve aspect ratio
and the existing framebuffer and memory boundaries; omitted options retain the
original 256×224 image. See the developer instructions for the manifest example.
`tools/check_zelda_video.py` checks nine private recipe configurations on both
runtimes, comparing exact sampled pixels over 1,000 frames and exercising
original gameplay, audio and SRAM reload at default and maximum widths.

Players can override the recipe's image settings on its game page under Display,
then save for the next launch. Original width, 16:9 widescreen, widest view and
extended vertical view are available. Package default removes the override and
follows the current signed recipe. Preferences stay with the creator/game across
updates and do not change signed package bytes, permissions or resource limits.
The CLI provides the same controls through `configure-display`. Current engines
apply changes without rebuilding. Saved display metadata participates in
integrity checks; malformed data is reported and preserved.

Allow extra sprites exposes the original optional renderer mode. It is off by
default; enabling it bypasses the original per-line sprite/tile clipping while
retaining the engine's fixed object storage and existing host budgets. Signed
recipes use `zelda_video.no_sprite_limits`; the CLI uses `--extra-sprites`.

The staged PPU adaptation also makes three bitplane reads explicitly unsigned
before shifting, removing a reproduced signed-shift error for high color planes.
The pinned upstream files remain unchanged. Both production builders and the
maintainer builder apply the same count-checked JSON adaptation. The full sprite
regression covers every object count from 0–128, all sixteen pixel values, both
8/16-pixel sprite sizes, original/maximum side widths, both limit settings and
excluded rows. It passes sanitizers and actual ELF32 execution. This qualifies
those evaluator paths; other renderer modes and full campaign/platform coverage
remain separate requirements.

Composed-scanline qualification also covers guarded sprite/blank output and
patterned 2bpp, 4bpp and Mode 7 mosaic backgrounds, including flips, scrolls,
clipped windows, all sixteen mosaic sizes and side widths 0/1/32/96. This exposed
and fixed an unsigned-mask issue and invalid horizontal mosaic-table indexing in
side areas. Horizontal spans now use a bounded calculation; the original
256-pixel HUD width remains intact. Both 64-bit sanitizer and actual ELF32 runs
exercise the complete scanline corpus.

Current Wasm caches must expose the checked renderer revision and carry the
current reviewed source-kit identity. The trusted builder appends a versioned
custom section with a SHA256 digest of length-framed source-kit names and bytes.
Readiness, launch and integrity checks reject missing, changed or duplicate
identities with a rebuild requirement. This is compatibility metadata, not source
attestation or a replacement for build receipts, import checks or memory bounds.
Integrity fix rebuilds from the verified package. The regression strips only the
identity section from an actual callable engine, retains a valid receipt and
current renderer revision, verifies rejection, performs the actual repair and
launches the rebuilt engine while checking the signed package remains identical.

Save-state/replay work now has a portable DMA component. The original callback
serialized 192 bytes on 64-bit hosts and 188 on 32-bit hosts, including struct
padding. The reviewed adaptation encodes the original 192-byte layout explicitly
in little-endian order, zeros padding on write, ignores historical padding on
read, preserves the live bus pointer and validates boolean/index fields before
changing DMA state. Sanitized host and actual 32-bit tests cover fixed byte
vectors, round trips, every accepted boolean/index value and 22,542 invalid
fields without mutation. This component is staged by both trusted builders;
legacy 188-byte snapshot import and replay recording remain unfinished. The
host snapshot and slot support below use this component. Existing SRAM
saves and their format are unchanged.

The DSP component now also uses an explicit 3024-byte little-endian layout.
Field descriptors address native members independently of their wire offsets;
padding is zeroed on write and ignored on read. The live APU-RAM pointer remains
owned by the current instance. Boolean representations, envelope/gain states,
BRR flags, FIR index and sample-buffer offset are checked before any DSP state
changes. Fixed byte checks, original semantic-byte agreement, exact original
stereo sample-buffer output and 86,529 rejected invalid fields pass sanitizers
and actual 32-bit execution. These component checks complement the host snapshot
loader below; replay controls and broader hardware/platform coverage remain open.

The PPU component preserves the original 67,305-byte field boundaries with
explicit little-endian VRAM, palette and tilemap-address words. Ignored regions
write as zero and remain ignored on read. All layer flags are validated before
video memory changes, while live rendering pointers, scrolls and other original
non-snapshot fields remain intact. Sanitized and actual 32-bit tests cover every
video/palette word, exact original byte boundaries, round trips and all 2,032
invalid layer flags. A separate callback test confirms malformed flags stop
before returning to game execution. Full replay and broader player
tools still require integration and whole-state verification.

The SPC player's original variable-to-RAM map also now writes native 16-bit
fields explicitly in little-endian order and decodes them back to native values.
Single-byte fields and four-byte port arrays remain byte copies. Native padding,
live pointers, channel indices, timer state and unmapped RAM are preserved.
The original source stays unchanged; both native/Wasm builders and the
maintainer builder apply checked source adaptations from the embedded kit.

The synthetic production-function fixture checks all 486 original mapped fields
across every 16-bit value, with compile-time checks that word entries really are
`uint16_t` and port entries really are byte arrays. Sanitized 64-bit and actual
32-bit little-endian builds retain exact original behavior. A real 32-bit
big-endian PowerPC executable under QEMU produces the same pinned wire digest
`4cc3066ee2043b25`; the unmodified original copy functions produce 13,447,680
field mismatches there. This qualifies the mapped audio-state conversion, not
all big-endian engine arithmetic, full gameplay or physical hardware.

Maintainers can run `tools/check_zelda_spc_state.py` on the host, add
`--32-bit --cc gcc-11` for ELF32, or use an installed Ubuntu PowerPC toolchain:

```sh
python3 tools/check_zelda_spc_state.py --big-endian --cc powerpc-linux-gnu-gcc-13 --compiler-prefix / --emulator /usr/bin/qemu-ppc --sysroot /usr/powerpc-linux-gnu
```

The full snapshot callback digest and little-endian SRAM/slot formats remain
unchanged. Fresh signed native/Wasm gameplay, audio, snapshot restart, slot keys
and source-identity repair checks pass with this adaptation. Rebuild cached
engines after updating the host; its complete source-kit identity includes the
new codec. Players need none of these maintainer tools.

Live SPC playback also uses explicit little-endian upload headers and bounded
16-bit wrapping reads of audio RAM. High-byte reads/writes operate on numeric
`uint16_t` values instead of host byte positions. This removes the original
unaligned `uint16*` loads and `WORD`/`HIBYTE` aliases from the player. Its backward
channel-reset loop stops before forming a pointer before the array. The source
adaptation retains the original control flow and checked upstream anchors.

The DSP sample decoder no longer left-shifts negative values. Multiplication
and the defined BRR overflow-range result retain the intended sample values.
An independent floor-division/clipping oracle checks 786,432 BRR blocks across
all channels, ranges, filters, header flags and nibbles, with edge histories and
audio-RAM boundary addresses. Full production-player tests also check all
65,536 high-byte values and RAM word addresses, 400 bank-upload cases spanning
odd alignments and wrapped destinations, and 64 audible stereo synthetic songs.
Their combined PCM/state digest `7ffc2b10517af1ae` is identical on sanitized
ELF64, real ELF32 and emulated big-endian PowerPC. Original reference operations
still reproduce misalignment and negative-shift sanitizer failures; original
high-byte code disagrees with 655,095 live field checks on big-endian execution.

Add `--playback` to the maintainer commands above to run this gate. It compiles
the complete player and DSP with strict warnings; no warning or sanitizer check
is disabled. Tremolo volume calculation and fractional-tick interpolation are
now staged from reviewed numeric code, with the original delay counter compared
rather than overwritten. The pinned upstream file remains unchanged.
`tools/check_zelda_tremolo_driver.py --rom /path/to/user-selected-rom.sfc` runs
1,048,576 exact comparisons between production calculations and the original USA
SPC instructions using the pinned SPC CPU emulator. It checks all phase/depth
pairs on all eight channels, both entry points, varying volume factors,
interpolation rates and fractions, dirty flags and unchanged phase counters.
The selected ROM and its sound-bank data stay in RAM; Python is used only by
this maintainer test, never by the shipped app. Another 4,194,304 comparisons
execute the original tick and background-task instructions for every delay/held
counter pair, all eight channels, four depths and both entry points. They check
phase advance, depth-255 phase saturation, counter wrapping, final volume,
dirty flags and the production DSP volume-write count.

The synthetic playback gate adds 18 enabled/disabled song pairs with depth,
rate and delay variations and the original enable/disable commands. Effective
modulation must change PCM; depth 1 (which rounds to zero modulation) and a
delay beyond the note must preserve PCM. Every enabled song must reach its
disable command. Their PCM/state digest `0273f69d973d4614` agrees on sanitized
ELF64, actual ELF32 and emulated big-endian PowerPC. Fresh native/Wasm gameplay,
audio, SRAM, snapshot restart and private Vulkan game-window checks pass with
the staged tremolo implementation. Strict release lint and formatting pass. The
new-music command stub remains unsupported. The host adapter uses direct
`LoadSongBank` uploads for the original intro/overworld, dungeon and credits
bank changes instead of the console's port-by-port transfer handshake.

Before initialization, all three compiled banks must pass a bounded structural
check: complete little-endian record headers and payloads, followed by exactly
the canonical two-byte zero marker. Truncation, missing markers and trailing
data are rejected before the original upload routine runs. Original 16-bit
destination wrapping remains supported. Synthetic tests cover every nonzero
16-bit record length, odd alignment, truncations, multiple records and marker
corruption under sanitizers at host width, real ELF32 and emulated big-endian
PowerPC. Actual Wasm tests corrupt each bank's first record and final marker,
require initialization failure, restore the bytes and then initialize and replay
gameplay successfully. This validates record bounds, not every music opcode or
the semantic contents of arbitrary replacement banks.

Complete music/effect/mod coverage, full-engine endian portability and physical
platform qualification remain separate work. These tests contain synthetic
banks, not extracted music. Fresh native/Wasm gameplay and slot tests also pass
with the corrected live player and sample decoder.

The tremolo investigation uses the annotated original
[BackgroundTasks and VolumeModulation routines](https://github.com/spannerisms/jpdasm/blob/master/spc.asm)
as a guide; the executable oracle checks the selected USA ROM's instructions
directly rather than assuming the annotated revision is identical.

The reviewed engine now has an internal bounded snapshot capture/restore
interface. Its 275,497-byte envelope contains a version, pinned-engine profile,
payload length and CRC32 around the original state callbacks with the portable
components above. Header/checksum and constrained DSP/DMA/PPU fields are checked
before the original load routine runs. CRC32 detects accidental corruption; it
does not authenticate a snapshot or validate every possible game-RAM state.
The capture/restore callbacks retain the original audio-state, HDMA and
synchronization order. Successful restoration marks SRAM dirty for the host.

Synthetic sanitizer and real 32-bit runs produce identical pinned whole-state
bytes and verify rejection before loading. A private ROM-backed Wasm fixture
restores the title after entering gameplay and gameplay after opening inventory,
then matches a fresh restored instance for 300 exact frame/audio/controller/state
observations. Invalid snapshots preserve captured state. ROM resources and
snapshots stay in RAM. Run the maintainer check with your own ROM:

```sh
python3 tools/check_zelda_snapshot_runtime.py --rom /path/to/your.sfc --sysroot /path/to/sysroot --slang-root /path/to/slang
```

The host now connects this interface through `Game::capture_zelda_snapshot` and
`Game::restore_zelda_snapshot` for both the native worker and desktop Wasm.
Snapshots retain their signed creator/game identity and the wrapper frame
counter; restoring into a fresh session therefore does not trigger its normal
first-frame reset. Successful restore clears stale host frame/audio buffers;
SRAM is committed through the ordinary save path on the following game tick.
No snapshot operation is exposed through the game SDK.

A fixed 1 MiB snapshot allowance is subtracted from the existing runtime memory
allowance, rather than added to the signed ceiling. Owned buffers, cross-session
borrows and bounded transfer scratch reserve from it. Three retained states fit
with transfer scratch; attempting a fourth fails before calling the engine.
Dropping a state releases its reservation, including after its original game
has stopped. Snapshot support is unavailable without signed save permission.
Native transfers use separate control framing between game ticks, chunks of at
most 64 KiB, exact bounded replies and one shared existing tick deadline for
the complete operation. Corrupt-state rejection keeps the worker running.

The SPC player's original RAM map omits its last-written echo-delay cache.
Restore now recovers that cache from the restored DSP register instead of a
later song. Synthetic original echo-setup checks cover all 65,536 saved/inherited
cache pairs, both unchanged and changed delay writes, with exact register
traces on sanitized 64-bit and actual 32-bit builds. Removing the correction
reproduces mismatches. The private signed native/Wasm host tests restore title
after reaching gameplay and gameplay after opening inventory, then compare
360 exact frame/PCM/controller/state observations per runtime against another
restored session. They also exercise ownership, capacity rejection, stale-output
clearing, corruption rejection and continued playback. The maintainer runner is
`tools/check_zelda_inputs.py`; it requires every named fixture to execute once.

## Snapshot slots

In the game window, **Shift+F1–F10** saves slots 1–10 and **F1–F10** loads them,
matching the original port's default bindings. In **Settings → Snapshot
shortcuts**, each load and save key can select any slot from 1–20, or **Off**.
Save Settings before launching the game. Load and save mappings are independent;
**Reset snapshot shortcuts** restores the defaults in the form, and **Cancel
edits** discards those changes. Saving a snapshot replaces that slot.
Normal in-game SRAM saves remain separate. The library game page lists these
controls; the game window still contains only the game. A successful load also
clears queued device audio and pending button edges before playback resumes.
Only real, focused, nonrepeating key presses qualify. The host uses each press's
OS modifier snapshot, including Shift held before the game gained focus.

Headless CLI runs can address all 20 slots supported by the original port's
configuration:

```sh
gamepkg run com.gamepkg.zelda3 --target native --rom /path/to/your.sfc --frames 1000 --save-slot 1
gamepkg run com.gamepkg.zelda3 --target native --rom /path/to/your.sfc --frames 120 --load-slot 1 --save-slot 2
```

Use `--target wasm` for the explicitly built desktop Wasm version. Slot flags
require `--frames`; slot numbers must be 1–20. Loading happens before the run and
saving after successful frames. A missing or damaged slot reports an error;
the desktop keeps the game running.

The host writes fixed-size `.gpstate` files beneath the library's `states`
directory, scoped by creator, game ID and save schema. A 96-byte little-endian
header binds the engine payload and wrapper counter with SHA-256. This checksum
detects corruption; it is not a signature or authentication of arbitrary state.
No game-chosen paths, snapshot imports or snapshot-file guest APIs are added.
Directory-relative capability operations reject symbolic links and hard links,
pin the folder during replacement, and replace a fully written, synced temporary
file atomically. Unix folders/files are created with modes 0700/0600. Slot reads
allocate directly into the existing snapshot budget after exact-size preflight.
Library storage totals include the current schema's slots. Snapshot RAM may
contain game data and belongs in private user storage, never a shared package.

The private host proof now saves gameplay, stops its original session, loads
the file into a restarted session and compares another 120 exact frames and PCM
observations. It verifies storage accounting and corrupt-file rejection before
changing a running game. Add `--gui-slots` to `tools/check_zelda_inputs.py` to
also drive the actual Vulkan window on private X11 displays: save with the
original keys, hold a key through OS autorepeat, restart and load the slot.
Both runtime targets and headless CLI slot save/load pass. Production slot tests
also execute as ELF32 Linux and PE32 Windows (under a disposable Wine prefix),
including an independently calculated identical wire-format vector and
hard-link rejection. Wine is a maintainer compatibility check, not a physical
Windows/hardware or full 32-bit gameplay proof.

Legacy snapshot imports, replay recording/playback, arbitrary game-RAM semantic
validation and broader campaign/platform coverage remain unfinished. Players
do not need Python, Wine or these maintainer tools.

## Original replay format foundation

The host-only `zelda_replay` module now reads the original little-endian desktop
replay framing and command stream without allocating from file-supplied counts.
It checks complete headers, exact log/base/current snapshot extents, command
boundaries, resume offsets, signed-range delay/count extensions and patch data
before returning a cursor. Snapshots remain untrusted raw payloads: this check
does not make them safe to restore into an engine.

The cursor preserves all twelve input toggles, zero-delay command ordering,
extended delays, original 17-bit RAM patch wrapping and the original single
read for a zero-frame recording. Its patch callback can deliver bytes directly
to a host engine bridge. If delivery fails, the frame does not advance and a
retry resumes the failed patch; earlier delivered commands are not rolled back.
The decoder itself calls no engine and reads no filesystem paths.

The host-only `Recorder` streams the matching original version-1 file into a
fresh host-owned output. The caller sets a log-byte quota. Each frame's input
changes and each post-frame RAM patch are written with original delays,
variable-length counts and little-endian header fields; patch payloads do not
need a second heap copy. Quota failures leave a full multi-toggle frame or
patch unwritten. The writer requires an engine snapshot at finish and never
accepts an engine snapshot from the decoder without separate validation.

`python3 tools/check_zelda_replay.py` compares the actual Rust decoder against
the pinned original C reader: 12,802 per-frame input/RAM-hash comparisons and 27
complete final RAM images. It also compares 24 synthetic recorded input/patch
sequences byte for byte and checks frame, input and delay metadata against the
pinned original C recorder. Both proofs pass as actual ELF32 programs with
`--32-bit --cc gcc-11 --linker gcc-11` on the local multilib setup. Host ASan
reproduces the original truncated-patch heap-buffer overflow; Rust framing
rejects that input. Nine ordinary Rust tests cover all command tags and their
truncations, header/resume corruption, signed extension limits, edge timing,
patch-delivery recovery, streamed wire bytes and atomic quota failures. All
data in these tests are synthetic.

Live recording/storage, embedded-snapshot validation/restoration, native/Wasm
playback and original replay shortcuts are still being connected. This module
is a verified format prerequisite, not an available replay player or importer.
It is not exposed through the guest SDK and adds no allowed guest dependency.
