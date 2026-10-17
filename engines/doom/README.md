# Pinned DOOM engine for GamePKG

The dependency-free recipe lives in [GamePKG_Ports/doom](https://github.com/centrexity/GamePKG_Ports/tree/main/doom); its setup and controls are in [DOOM.md](https://github.com/centrexity/GamePKG_Ports/blob/main/DOOM.md). Use host 0.1.10 or newer for direct native builds.

The host owns the C engine and adapter. Packages can request only the exact `doom-e257a095` profile with core-v1, one player-supplied IWAD, save/audio permissions and at least 80 MiB total memory. The recipe sets a 96 MiB cap. No package C, build script, replacement library or Cargo dependency is accepted.

## Compiler setup

Native Linux Build needs Clang 18 and lld-18, and does not use a WASI sysroot. The separate Build Wasm needs Clang 18, wasi-libc and libclang-rt-18-dev-wasm32. The engine uses `/usr` when the WASI headers are installed there. For a separate sysroot set `GAMEPKG_DOOM_SYSROOT` (the existing `GAMEPKG_ZELDA3_SYSROOT` is also accepted). Windows uses the matching Linux build worker through the existing opt-in WSL 2 exchange. Players need no Python.

Both builds check every pinned upstream source hash. The optional Wasm build parses the complete C source set with Clang, then compiles it in bubblewrap with no home directory or network, the existing compiler resource limits, and a 64 MiB maximum Wasm memory. Only host-embedded source is staged. The result is verified to have zero imports; no WASI runtime is linked by GamePKG.

## Runtime boundaries

The WAD is selected explicitly, snapshotted read-only and validated for its IWAD header, bounded directory and lump ranges. It is not copied to the game package or unpacked onto disk. The native worker runs the same pinned engine behind OS namespaces, a syscall allowlist, address-space limits and tick deadlines. The optional Wasmtime profile runs the engine with no host imports, a private 64 MiB memory and epoch deadlines. Engine and recipe memory together stay within the lower of the signed cap and player ceiling. Rendering snapshots 320×200 pixels through the bounded host renderer. Audio supplies 800 stereo PCM frames at 48 kHz per 60 Hz host tick. The engine advances at its original 35 Hz tic rate.

The engine's C stdio adapter is a private virtual filesystem: only the selected WAD and one bounded save slot exist. No path can reach the host filesystem. Configuration files, environment access and process execution are denied. Saves commit through the existing per-game/per-creator save blob after successful ticks. The one persistent slot is limited to 512 KiB and can be restored on launch. Engine memory mutations after a trap are not rolled back; close and relaunch a stopped game.

Sound effects use a bounded 16-channel DMX mixer. Original WAD music uses the
[in-memory OPL music backend](music/README.md), with instruments read from that
same selected WAD. Multiplayer, mod loading, music backend selection and browser
export remain unfinished. No microphone, socket, SDL device or native library is
exposed to the engine. Controller/touch input comes through the same host button
snapshot as keyboard input.

The mixer respects world-position separation and changing volume using the
[original DOOM quadratic panning and PCM gain](https://github.com/id-Software/DOOM/blob/master/linuxdoom-1.10/i_sound.c).
Separate channels survive native packets and Wasm reads through host output.
Legacy mono Wasm caches remain readable; rebuild to enable stereo. A 64-bit
playback counter prevents long sound effects from wrapping to earlier samples.
`tests/doom_audio.c` exercises the production mixer without assets or devices.

## Source and licensing

Upstream: https://github.com/ozkl/doomgeneric at `e257a09527062bfb986b6e5e0780839b6cbd5b71`.

`upstream-lock.json` records the complete included upstream file hashes and compilation source list. `upstream/` is unchanged from that revision. The adapter is in `adapters/engine.c`. The engine and adapter are GPL-2.0-or-later; see `upstream/LICENSE` and individual source headers. GamePKG packages and release source bundles include the corresponding sources and notices. No commercial game data, WAD files or extracted assets are included.

## Verification

`cargo test --all-features` includes malformed WAD, memory/import boundary and manifest/import policy checks. For actual gameplay, first pack/install/build the recipe in a private test library, then run:

```sh
GAMEPKG_DOOM_TEST_ROOT=/path/to/library \
GAMEPKG_DOOM_TEST_WAD=/path/to/DOOM.WAD \
cargo test --all-features --test doom_play -- --ignored
```

That test works in a private copy, starts a level from the menus, checks movement and firing audio, quicksaves, reloads the saved position, checks a nonblank gameplay frame and verifies the WAD was unchanged. `tests/doom_gui.py WAD --library LIBRARY` checks actual ROM selection, visible gameplay, movement and quicksave in the desktop app on a private display. Both tests require a player-owned compatible WAD and never publish it.

The same test file also runs the original attract demo and requires actual
left/right sample differences. Both tests pass on native and Wasm; select one
explicitly with `GAMEPKG_DOOM_TEST_TARGET=native` or `wasm`.
It also requires title music before any attract-demo sound effects. A separate
optional `tests/doom_music_wad.c` proof reads the original WAD in place and checks
the first three seconds of every music lump. All 45 songs in the local selected
DOOM WAD passed with address and undefined-behavior sanitizers. This does not
establish full-song or full-campaign coverage.

For a separate WASI installation, use **Settings → C/WASI build tools folder**, check the tools and save settings. This persists across app restarts without requiring a launch-time environment variable. Engine-specific sysroot environment overrides take precedence.
