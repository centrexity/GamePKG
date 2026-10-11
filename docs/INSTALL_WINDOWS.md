# Windows host development preview

This archive contains `bin/gamepkg.exe` and `bin/gamepkg-gui.exe`, example source, original dependency sources, license notices and SHA-256 metadata. Extract it into a folder you control. Verify the ZIP against its `.sha256` file; executable hashes are in `BUILD.json`.

Run `bin\gamepkg.exe --help` in PowerShell, or launch `bin\gamepkg-gui.exe` for the desktop library. Inspection, creator approval and installed-library metadata use the same verified package format as Linux. The host requires a working desktop graphics driver.

The current source has an **experimental, opt-in WSL 2 compiler connection**. End-to-end builds through WSL 2 and native desktop GPU gameplay have not been verified on Windows yet. The existing Linux preview provides the verified build/play workflow.

To configure the experimental connection, install a WSL 2 Linux distribution and prepare its GamePKG compiler with the same version as the Windows host, Rust's `wasm32-unknown-unknown` target, bubblewrap, and the pinned Slang compiler following the Linux setup instructions. The Linux compiler must include the internal `build-worker` command; the earlier published 0.1.3 Linux archive predates that command, so build the current source for this development preview. Test a source build inside Linux first.

In the Windows settings page, supply the distribution name and the absolute Linux path to its `gamepkg` executable, for example `GamePKG-Ubuntu-24.04` and `/opt/gamepkg/bin/gamepkg`. Both fields must be supplied together; blank fields disable the connection. These values can also be set as `windows_compiler_distribution` and `windows_compiler_executable` in the settings file. Slang configuration inside the Linux distribution controls its compiler installation.

Build/Rebuild sends only the verified source game and referenced shared assets through a private request store. Saves, history, user ROMs and keys are excluded. The Linux worker builds offline in the existing sandbox, then the Windows host validates the returned package identity, backend, host version, output checksums, shader declarations and playable runtime profile before accepting it. WSL 1, missing distributions, version mismatches, changed settings and invalid responses fail without a native unsandboxed build fallback. Launcher execution and output draining have deadlines.

Keep creator keys private. Players do not enter a signing key; signed game manifests carry the creator's public key. Native Windows build and runtime regression checks do not by themselves verify actual desktop GPU playback or isolated compilation.

For an actual Windows/WSL integration check after setup, build the current Windows CLI and run `python tests/windows_wsl.py --distribution YOUR-DISTRIBUTION --compiler /absolute/linux/path/gamepkg`. It signs temporary source fixtures, uses normal Windows Build, plays core/component games, damages a compiled file and repairs it through WSL. It does not substitute fixture-copy tests for the real compiler connection. Desktop GPU rendering needs separate verification.
