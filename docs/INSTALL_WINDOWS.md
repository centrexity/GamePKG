# Windows host development preview

This archive contains `bin/gamepkg.exe` and `bin/gamepkg-gui.exe`, example source, original dependency sources, license notices and SHA-256 metadata. Extract it into a folder you control. Verify the ZIP against its `.sha256` file; executable hashes are in `BUILD.json`.

Run `bin\gamepkg.exe --help` in PowerShell, or launch `bin\gamepkg-gui.exe` for the desktop library. Inspection, creator approval and installed-library metadata use the same verified package format as Linux. The host requires a working desktop graphics driver.

The current source has an **experimental, opt-in WSL 2 compiler connection**. End-to-end builds through WSL 2, core/component playback, integrity repair and actual native GUI shader rendering/input passed on a hosted Windows runner using software OpenGL. Hardware graphics drivers were not tested exhaustively. The existing Linux preview provides the verified build/play workflow.

To configure the experimental connection, install a WSL 2 Linux distribution and prepare its GamePKG compiler with the same version as the Windows host, Rust's `wasm32-unknown-unknown` target, bubblewrap, and the pinned Slang compiler following the Linux setup instructions. The matching 0.1.4 Linux preview includes the internal compiler worker. Download its archive and checksum, verify and extract them inside WSL, and follow its `INSTALL.md` to install the compiler prerequisites. The Linux command-line binary also needs the listed ALSA runtime library; no Linux desktop display is needed for compilation. Test a source build inside Linux first. Earlier 0.1.3 Linux archives predate this worker and cannot serve as the matching compiler.

In the Windows settings page, supply the distribution name and the absolute Linux path to its `gamepkg` executable, for example `GamePKG-Ubuntu-24.04` and `/opt/gamepkg/bin/gamepkg`. Both fields must be supplied together; blank fields disable the connection. These values can also be set as `windows_compiler_distribution` and `windows_compiler_executable` in the settings file. Slang configuration inside the Linux distribution controls its compiler installation.

Build/Rebuild sends only the verified source game and referenced shared assets through a private request store. Saves, history, user ROMs and keys are excluded. The Linux worker builds offline in the existing sandbox, then the Windows host validates the returned package identity, backend, host version, output checksums, shader declarations and playable runtime profile before accepting it. WSL 1, missing distributions, version mismatches, changed settings and invalid responses fail without a native unsandboxed build fallback. Launcher execution and output draining have deadlines.

Keep creator keys private. Players do not enter a signing key; signed game manifests carry the creator's public key. Native Windows build and runtime regression checks do not by themselves verify actual desktop GPU playback or isolated compilation.

For an actual Windows/WSL integration check after setup, build the current Windows CLI and run `python tests/windows_wsl.py --distribution YOUR-DISTRIBUTION --compiler /absolute/linux/path/gamepkg`. It signs temporary source fixtures, uses normal Windows Build, plays core/component games, damages a compiled file and repairs it through WSL. It does not substitute fixture-copy tests for the real compiler connection. Desktop GPU rendering needs separate verification.

Add `--gui` with Pillow installed and a working OpenGL driver to verify actual blue/green shader pixels and held/released keyboard input for both runtime profiles. The hosted test uses checksum-pinned Mesa 26.2.4 DLLs only in its build folder; those test DLLs are excluded from the distribution. Screenshots and diagnostics are retained as a separate test artifact.

The current 0.1.5 source requires a matching 0.1.5 compiler worker for the full Rust preflight policy. Build both hosts from the same source revision until matching release archives are published; a 0.1.4 worker is intentionally rejected by a 0.1.5 Windows host.

## Zelda3 optional engine

Zelda3 uses the same opt-in WSL 2 compiler exchange as other source games.
Install Clang 18, wasi-libc and libclang-rt-18-dev-wasm32 in that compiler's
Ubuntu 24.04 environment. The matching 0.1.6 host includes the pinned engine
sources and transfers its verified Wasm/receipt alongside the Rust entry.
No player ROM or save data is sent to the compiler.

Playback runs in the native Windows host and does not use WSL. It needs a host
Python runtime with Pillow and PyYAML. Either set GAMEPKG_ZELDA3_PYTHON to your
Python executable, or prepare the optional portable runtime beside the programs:

```powershell
python tools/install_zelda3_runtime.py bin/runtime/zelda3
```

The installer uses checksum-pinned official Python/PyPI downloads, installs into
a new folder, and preserves their license files. A system Python can run the
installer; the resulting decoder does not need pip or an installed Python.
Set the fuel ceiling to 100,000,000, import/sign/build the Zelda3 example and
select your own supported USA ROM in the library. The ROM is decoded only in RAM;
only game SRAM is persisted. See engines/zelda3/README.md for controls and tests.
