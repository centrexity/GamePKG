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
