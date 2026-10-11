# Windows host development preview

This archive contains `bin/gamepkg.exe` and `bin/gamepkg-gui.exe`, example source, original dependency sources, license notices and SHA-256 metadata. Extract it into a folder you control. Verify the ZIP against its `.sha256` file; executable hashes are in `BUILD.json`.

Run `bin\gamepkg.exe --help` in PowerShell, or launch `bin\gamepkg-gui.exe` for the desktop library. Inspection, creator approval and installed-library metadata use the same verified package format as Linux. The host requires a working desktop graphics driver.

**Isolated source builds on Windows are not implemented yet.** A downloaded game still needs compilation before it can play. The native host rejects unsupported compilation; this archive does not provide a complete Windows game installation/play workflow. Do not disable isolation to work around that restriction. The existing Linux preview provides the verified build/play workflow.

Keep creator keys private. Players do not enter a signing key; signed game manifests carry the creator's public key. Native Windows build and runtime regression checks do not by themselves verify actual desktop GPU playback or isolated compilation.
