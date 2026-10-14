# Zelda3 port work in progress

The full objective is a playable signed GamePKG using a player-selected Zelda:
A Link to the Past USA ROM directly. Neither the ROM nor its decoded resources
belong in the package. Decoding stays in memory; no extracted asset directory,
`zelda3_assets.dat`, graphics, music or intermediate YAML is written to disk.

The engine is pinned to snesrev/zelda3 revision
`45a149d8b4bb8098f53d1009f76e76e7d8b79bbf`. `upstream-lock.json` records the SHA-256
of each copied source and license. Copyright and MIT notices, including the
LakeSnes authors' notices, remain in `upstream/LICENSE.txt`.

## Verified development milestone

- The selected USA 1 MiB ROM is recognized; a 512-byte copier header is supported.
- The resource decoder constructs all 165 engine resource records in RAM.
- A Python audit hook denies actual writes/mkdir/rename/remove during decoding.
- The source ROM's checksum is unchanged after decoding.
- All selected C translation units pass Clang syntax checks before compilation.
- The full gameplay/PPU/DSP engine builds into a 16 MiB maximum-memory Wasm module.
- The resulting module has **zero imports**, including no WASI/file/network imports.
- With the user's ROM, 1,000 actual engine frames reach the title screen and
  generate audio. The local screenshot is `target/zelda3-proof/engine-title.png`.

`adapters/engine.c` replaces the SDL shell and native save/audio integration. It
retains normal SPC/DSP music, disables external MSU music, and removes desktop
replay logging from the tick path. Native libc diagnostic descriptor operations
fail closed inside Wasm. The signed `examples/ports/zelda3` GamePKG now builds and plays through the
native GamePKG runtime. Its Rust entry invokes only the checked host engine API.

The Python RAM decoder is currently a development bridge over pinned upstream
resource code. It is not package-authored Python and must not become a facility
for running arbitrary package scripts. Its temporary structures and output exist
only in memory. The final runtime integration must preserve that behavior.

## Remaining requirements before calling the port complete

1. Integrate the pinned engine with isolated GamePKG builds and integrity receipts;
   accept no arbitrary C code or unapproved engine binaries from game packages.
2. Keep package-authored Rust subject to the existing macro/alias/indirect-call
   restrictions; no general relaxation of that policy is authorized by this port.
3. Bind ROM decoding to the file explicitly selected through the existing ROM UI;
   no ambient game filesystem/network access, extracted files or bundled ROM data.
4. Add the needed action/start/select/shoulder controls without changing existing
   arrow/WASD input bits. Deliver usable native CLI/GUI entry paths.
5. Route engine pixels/audio through the checked host operations; enforce memory,
   fuel and deadline limits and roll back failed frame saves.
6. Persist original SRAM through the creator-scoped save API and verify reopening.
7. Sign/install/build the actual package; test missing/wrong/headered ROM cases,
   title, file selection, beginning gameplay, movement/actions and native rendering.
8. Document build/play requirements and runtime portability truthfully. Browser
   support must be verified separately before it is claimed for this engine.

Current additional evidence: the signed package runs 1,000 frames through the
actual runtime. Tests create a file/name slot, advance the opening dialogue,
move Link inside the house, write the 8 KiB SRAM save and reopen the slot.
The native GUI test selects the ROM through the library, loads that saved slot,
and verifies visible held-key movement. `cargo test`, formatting and all-feature
Clippy pass locally. Source publication, remote portability checks and final completion
verification are still pending; the project goal remains active.

The port currently needs Linux, Clang 18 plus a Wasm C sysroot for isolated
source builds, and host Python with Pillow/PyYAML for in-memory ROM decoding.
Set `GAMEPKG_ZELDA3_SYSROOT` to the prepared sysroot. The signed manifest requests
100,000,000 fuel; select that ceiling explicitly in Settings (the existing
10,000,000 default is preserved). The host checks this before play. Memory is
reserved as 16 MiB for each Wasm instance under the combined 32 MiB limit, and
both frame execution and engine initialization use the declared fuel/deadline.

Windows compiler exchange now transfers and checks the engine artifact and its
receipt. The actual Windows CLI and GUI have passed local Wine tests with the
player-selected ROM, including 1,000 frames/audio, ROM selection, saved-slot
loading and keyboard movement. This is Wine evidence, not a claim of real
Windows hardware coverage. An optional checksum-pinned portable Python/Pillow/
PyYAML runtime can be prepared with `tools/install_zelda3_runtime.py`; Windows
uses it from `bin/runtime/zelda3/python.exe`, an explicitly configured
`GAMEPKG_ZELDA3_PYTHON`, or a system `python.exe`. WSL is used only for source
compilation, not playback. The compile request includes no ROM or save data.
A browser ROM decoder remains outside this native port; Zelda3 web export currently gives a clear
unsupported-feature error instead of producing a broken player.
