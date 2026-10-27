# Windows host development preview

Open **Settings → GamePKG updates** to check for a newer stable Windows release, download and verify it, and install it into a folder you choose. **Restart into updated GamePKG** launches the new copy after running games close. The updater keeps the previous portable folder for rollback and preserves your library, ROM selections, saves and settings. Update your shortcuts to the new copy. App updates use Windows curl and do not require Python, WSL or administrator access.

This archive contains `bin/gamepkg.exe` and `bin/gamepkg-gui.exe`, example source, original dependency sources, license notices and SHA-256 metadata. Extract it into a folder you control. Verify the ZIP against its `.sha256` file; executable hashes are in `BUILD.json`.

Run `bin\gamepkg.exe --help` in PowerShell, or launch `bin\gamepkg-gui.exe` for the desktop library. Inspection, creator approval and installed-library metadata use the same verified package format as Linux. The host requires a working desktop graphics driver.

## Native Takedawn source preview

The 0.1.18 development source connects native **Build**, **Play**, and integrity
repair for the host-reviewed v14 Takedawn kit on 64-bit Windows. This path uses
standalone Rust/Clang tools prepared through Settings or `gamepkg setup-native-tools`;
it needs neither WSL nor Python. The older Linux-only package must be updated
before building it on Windows. Recipe 0.1.3 selects v14; older admitted recipes
retain creator/save continuity but can stop on unsupported execution and need
an update. This is an unreleased candidate and requires the development app;
the published 0.1.17 app does not include this native engine.

In the development app, choose **Settings → Prepare native build tools**, then
save settings with **Memory ceiling** set to at least 4096 MiB. Import the signed
compatible recipe through **Add**, open its game page, choose your supported
original Xbox USA ISO, and select **Build**. The first optimized build can take
an hour or more; verified completed units are reused after interruption. **Play**
becomes available after a successful build and disc selection. Keep the whole
portable app folder together. A private hardware-test bundle uses a separate
preview library and its included recipe rather than changing the normal library.

The game owns its Vulkan window. GamePKG retains the selected ISO as a read-only
handle, saves the selection after successful admission, keeps original saves
scoped to the game/creator and tracks play time across the original restart flow.
The player uses an AppContainer with networking disabled and explicit runtime/save
access. Actual Windows tests verify real child-window creation and closure across
managed restarts under this confinement. The full native signed Build and
integrity-repair workflow also passes on Windows, preserving all 487 compiled
Rust units through repair. Original-game Vulkan, sound and physical
controllers still need Windows hardware qualification; Wine checks do not replace
that. See the [native Windows build notes](../engines/takedawn/rust-port/WINDOWS_BUILD.md).

## Optional Wasm compiler connection

The current source has an **experimental, opt-in WSL 2 compiler connection**. End-to-end builds through WSL 2, core/component playback, integrity repair and actual native GUI shader rendering/input passed on a hosted Windows runner using software OpenGL. Hardware graphics drivers were not tested exhaustively. The existing Linux preview provides the verified build/play workflow.

To configure the experimental connection, install a WSL 2 Linux distribution and prepare its GamePKG compiler with the same version as the Windows host, Rust's `wasm32-unknown-unknown` target, bubblewrap, and the pinned Slang compiler following the Linux setup instructions. Use a Linux compiler built from the same GamePKG version as the Windows host; it includes the internal compiler worker. Download its archive and checksum, verify and extract them inside WSL, and follow its `INSTALL.md` to install the compiler prerequisites. The Linux command-line binary also needs the listed ALSA runtime library; no Linux desktop display is needed for compilation. Test a source build inside Linux first. A different host/compiler version cannot serve as the matching compiler.

In the Windows settings page, supply the distribution name and the absolute Linux path to its `gamepkg` executable, for example `GamePKG-Ubuntu-24.04` and `/opt/gamepkg/bin/gamepkg`. Both fields must be supplied together; blank fields disable the connection. These values can also be set as `windows_compiler_distribution` and `windows_compiler_executable` in the settings file. Slang configuration inside the Linux distribution controls its compiler installation.

Build/Rebuild sends only the verified source game and referenced shared assets through a private request store. Saves, history, user ROMs and keys are excluded. The Linux worker builds offline in the existing sandbox, then the Windows host validates the returned package identity, backend, host version, output checksums, shader declarations and playable runtime profile before accepting it. WSL 1, missing distributions, version mismatches, changed settings and invalid responses fail without a native unsandboxed build fallback. Launcher execution and output draining have deadlines.

Keep creator keys private. Players do not enter a signing key; signed game manifests carry the creator's public key. Native Windows build and runtime regression checks do not by themselves verify actual desktop GPU playback or isolated compilation.

For an actual Windows/WSL integration check after setup, build the current Windows CLI and run `python tests/windows_wsl.py --distribution YOUR-DISTRIBUTION --compiler /absolute/linux/path/gamepkg`. It signs temporary source fixtures, uses normal Windows Build, plays core/component games, damages a compiled file and repairs it through WSL. It does not substitute fixture-copy tests for the real compiler connection. Desktop GPU rendering needs separate verification.

Add `--gui` with Pillow installed and a working OpenGL driver to verify actual blue/green shader pixels and held/released keyboard input for both runtime profiles. The hosted test uses checksum-pinned Mesa 26.2.4 DLLs only in its build folder; those test DLLs are excluded from the distribution. Screenshots and diagnostics are retained as a separate test artifact.

The optional Wasm compiler connection requires a Linux worker with exactly the same GamePKG version. Use the Linux and Windows archives from the same release for that connection. The full source preflight policy and builtin engine version are checked before compiler outputs are accepted. The standalone Takedawn native pipeline above uses its managed Windows tools instead.

## Zelda3 optional engine

Zelda3 uses the same opt-in WSL 2 compiler exchange as other source games.
Install Clang 18, wasi-libc and libclang-rt-18-dev-wasm32 in that compiler's
Ubuntu 24.04 environment. The matching host includes the pinned engine
sources and transfers its verified Wasm/receipt alongside the Rust entry.
No player ROM or save data is sent to the compiler.

Playback runs in the native Windows host and does not use WSL or Python.
ROM decoding is built into the executable; no interpreter, pip, Pillow, PyYAML
or separate runtime installation is needed. Install/build the Zelda3 package and select your own supported USA ROM in the
library. The ROM is decoded only in RAM; only game SRAM is persisted.
See engines/zelda3/README.md for controls and tests.

## DOOM source engine

The host includes the pinned DOOM Wasm engine. The Linux compiler needs the same Clang 18 and WASI tools described above. See [DOOM engine setup](../engines/doom/README.md) and the [port instructions](https://github.com/centrexity/GamePKG_Ports/blob/main/DOOM.md). Select your own DOOM.WAD/DOOM2.WAD; no WAD is included. Windows compilation uses the existing opt-in WSL 2 exchange; native Windows DOOM playback has not been tested locally.

The desktop renderer now defaults to Vulkan. Install the graphics driver’s Vulkan runtime, or launch `gamepkg-gui.exe --renderer opengl` to choose OpenGL explicitly. Other source engines on Windows use the separate **Build Wasm** and **Play Wasm** controls with the matching Linux compiler configured above. HTTPS and TCP/UDP APIs are host-mediated for Wasm gameplay as well.
