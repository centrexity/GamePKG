# GamePKG — Play games safely

**Know what you run. Control what it can access.** GamePKG is building a safer way to share and play games: inspectable source, creator-signed packages, isolated compilation, and a host that is designed to grant only the capabilities a game needs.

**Current status:** this is a development preview. Package verification, catalog import, installation, update checks, and isolated Cargo Wasm and Slang shader builds on Linux work today. The experimental grid-v1 profile now plays in the GUI and terminal under Wasmtime with scoped asset reads, fuel, memory limits, and epoch deadlines. Resource-free Slang GLSL, SPIR-V, and WGSL fragment passes now render in the OpenGL GUI; versioned WIT components are supported alongside the legacy core profile; broader rendering remains pending; this is not a guarantee that a package is harmless.

Linux host distributions can be prepared with `python3 tools/package_release.py /tmp/gamepkg-distribution` after building both release entry points. The archive includes CLI/GUI executables, setup tools, examples, original resolved dependency sources and notices, and integrity hashes. See [installation instructions](docs/INSTALL.md). The public repository's **Package Linux preview** workflow produces the same artifact on Ubuntu 24.04.


## Why distribute GamePKG instead of an opaque executable?

A conventional game executable can be difficult to inspect, and launching it without a sandbox may give it the permissions of your user account. That can include reading personal files, making network connections, or starting other programs. Signed binaries and separately sandboxed applications can also provide protections; the benefit of GamePKG is making source visibility, package verification, restricted builds, and declared capabilities part of one distribution workflow.

A `.gamepkg` contains Rust source, Slang shader source, assets, and a signed manifest. You can inspect the code and declared permissions before choosing to install it. The host checks the package, compiles its supported source profile into Wasm, and keeps game assets in the archive. The runtime exposes scoped game operations through the host rather than the operating system’s full set of interfaces.

| Protection | What it changes | Available today? |
| --- | --- | --- |
| Inspectable source | Games ship source rather than a bundled native executable; review is possible before building. Source visibility alone does not prove safety. | Yes |
| Signed identity and updates | The manifest and package contents are signed. Updates must keep the installed creator key and package ID, with a higher version. | Yes; first creator approval is still required |
| Restricted package contents | Rejects traversal paths, duplicate entries, symlinks, oversized archive contents, unsupported shaders, and source outside the initial policy. | Yes |
| Isolated compilation | Supported Linux builds run inside bubblewrap with a private work area, no network, and no access to your home directory. | Yes; checked source modules with a pinned dependency allowlist |
| Limited game access | Games receive host-approved asset, input, ROM, drawing, audio, and save operations. | Available in the core and versioned WIT component profiles |
| Runtime resource limits | Memory caps, execution fuel, and deadlines constrain running games. | Enforced for both Wasmtime profiles; GPU work is not fuel-metered |

## Build locally, improve over time

Building on demand on your own machine creates opportunities to optimize games for the supported platform and runtime instead of relying on a single prebuilt executable. Shipping source also lets the host rebuild games as compiler tooling and optimization settings improve. Creators can release signed source updates, and the host can check periodically for newer versions while preserving creator-key continuity.

These are opportunities, not a promise that every local build will be faster. GamePKG supports isolated Wasm builds, manual rebuilds, user-requested update checks, and configurable periodic checks while the desktop library is open and idle. Broader platform-specific performance tuning remains planned. Wasm builds must stay within their supported target and sandbox; local compilation does not grant games extra access to your machine.

## What can a GamePKG do—and what can it access?

Today, importing a `.gamepkglist` adds catalog entries without fetching games or trusting creators. Downloading fetches the package you select. Installation verifies and stores it; it runs no game code or package-provided install scripts. Update checks stage a signed candidate for review. Supported builds compile the verified Rust source to Wasm inside an isolated Linux process.

The package policy supports checked Rust source modules and host-owned pinned dependencies. The current catalog contains byteorder-lite 0.1.0, whose approval can be changed in Settings or through the CLI. It rejects build scripts, proc-macro configuration, compile-time file includes, unsafe blocks, foreign declarations, and filesystem, network, process, or environment paths detected by the source scanner. Shader files must be Slang source. The scanner is a policy filter, not a substitute for compiler isolation or a runtime sandbox.

The runtime provides **no ambient filesystem or network access**. Games use specific capabilities such as input, audio, or saving through the host. Asset access uses virtual paths inside the game’s archive; save access is scoped by creator, game ID, and save schema. A game requiring a ROM declares that requirement, and the player supplies it. ROM bytes are shared read-only only for the selected play session.

Guest networking is currently unsupported: manifests must declare `network = "none"`. User-requested host downloads over HTTPS or Tor are separate from game permissions. A signed update URL does not give the game permission to browse the internet. Signatures establish integrity and continuity with a creator key, not a creator's real-world identity or a promise that their code is safe.

## Try it

Install Rust, then run:

```sh
cargo test --locked
cargo run -- --help
```

A complete package input is in `examples/minimal`. Generate a creator signing key with `keygen`, keeping it outside the package directory. The command uses operating-system randomness, refuses to replace an existing file, and prints only the public creator key. On Unix, the private file is readable and writable only by its owner. Keep a private backup: signed updates must use the same key. Players do not need this private key.

```sh
cargo run -- keygen -o /path/to/signing.key
cargo run -- pack examples/minimal -o minimal.gamepkg --key /path/to/signing.key
cargo run -- verify minimal.gamepkg
cargo run -- asset minimal.gamepkg hello.txt -o hello.txt
```

Packing validates before publishing and refuses to overwrite files. The output must be outside the input directory. Signing keys are never stored in packages. The public key is inserted into the signed manifest during packing. Verification checks the embedded signature automatically; first installation requires a creator trust decision. An embedded key establishes signature integrity, not independent real-world identity.

## Initial policy

- Format version 1; network must be `none`; positive fuel and memory limit up to 512 MiB.
- Required manifest, PNG-signature thumbnail, license, credits, Cargo manifest/lockfile, and `source/src/lib.rs`.
- At most 10,000 entries, 64 MiB per file, 256 MiB uncompressed total; stored and zstd entries only.
- No absolute paths, traversal, backslashes, duplicate paths, directory entries, or symlinks.
- Only UTF-8 `.slang` files under `shaders/`. Inline SPIR-V, target-code injection and macro token pasting are rejected in every shader file, including unused sources. The conservative check also inspects comments and strings; it supplements the isolated compiler and does not prove GPU code is harmless.
- Rust source files under `source/src/`, pinned allowlisted dependencies only, `package.build = false`, and restricted Cargo fields. Includes, attributes, unsafe blocks, foreign declarations, and filesystem/network/process/environment paths are rejected.
- Optional AV1/WebM and Theora/Ogg previews undergo isolated codec, size, duration, frame-count, and initial-keyframe validation. PNG package validation checks magic bytes; GUI image decoding is bounded.

The source scanner is an early policy filter, **not a security boundary**. Rust macros and compiler behavior require an OS-isolated build process, followed by a Wasm capability sandbox. Linux builds now invoke the verified Cargo compiler inside bubblewrap; game code executes only through the Wasmtime runtime when Play/run is requested. Do not relax the source profile before those boundaries exist.

## Implemented workflow and remaining work

Archive verification, signed creator continuity, source checking, Linux isolated builds, and all four requested Slang output formats are implemented. CLI and desktop workflows support installation, catalogs, updates, build/rebuild, and scoped gameplay. The desktop includes previews, Settings, and portable store imports with shared assets. Core and versioned WIT component runtimes enforce memory, fuel, and tick deadlines.

Local tests cover accepted and rejected packages, every good source example, compiler isolation, scoped runtime operations, store transfers, and desktop behavior. Hosted verification runs the same fixture integrations. The [quickstart](html/guide/index.html) explains installation and play; the [library policy](html/guide/allowlist.html) describes the pinned approval catalog.

Remaining development includes broader GPU resources and vertex/compute pipelines, native Metal/DXIL desktop rendering, and isolated source builds on other operating systems. Key rotation, automatic save migration, and shared-pool garbage collection are also unfinished. The current Linux preview supports the documented gameplay profiles; these limits remain visible to creators and players.

See [the format notes](html/spec/index.md) and [security design](html/guide/security.md). The original design brief is preserved in `DESIGN.md`. No site has been deployed.

## Creator signing and updates

Creators sign their `.gamepkg` files with their own Ed25519 private keys. Verification proves the package was signed by the holder of a trusted key and that its signed contents have not changed. A signature alone does not prove a person's real-world identity: the public key must first be associated with the creator through a trusted channel.

The installer pins the creator key to the installed package ID. Updates must keep that ID, use the same key, and have a strictly higher semantic version. Key changes are rejected even with `--trust-creator`. Rotation and lost-key recovery remain pending. `verify` checks the embedded key by default; `--trusted-key` remains available for advanced callers who require a separately known signer.


## License and attribution

GamePKG and its minimal example are licensed under [Apache 2.0](LICENSE). [NOTICE](NOTICE) identifies the project and links to https://gamepkg.com. When redistributing covered code or derivative works, preserve applicable copyright and attribution notices, include the license, and mark modified files as required by its terms. NOTICE attribution may appear in distributed notices, source/documentation, or the appropriate notice display; the license does not mandate a clickable or prominently displayed backlink on every website.

Creators choose their own licenses for their games and assets. Using the GamePKG format or host does not automatically license a game under Apache 2.0. The package verifier accepts an optional top-level `NOTICE`, includes it in the signed tree, and does not require it for independently authored games.

## CLI and desktop GUI

Build both entry points locally:

```sh
cargo build --locked --release --features gui --bins
cargo test --locked --all-features
```

The executables are `target/release/gamepkg` and `target/release/gamepkg-gui` (with `.exe` on Windows). Launch the GUI with `cargo run --locked --features gui --bin gamepkg-gui`. It lists installed packages and accepts a local package path or HTTPS URL. Inspection runs in a background worker and displays the verified creator fingerprint and permissions before installation. New creators require approval; updates use the installed creator key automatically. It compiles the supported source profile on Linux and plays core and versioned WIT component games with scoped assets, ROMs, saves, drawing, and audio. Resource-free fragment passes render through OpenGL; broader graphics remain unfinished. A desktop display is required to launch it.

The CLI remains available without desktop dependencies: `cargo build --locked --release --bin gamepkg`. See [examples](examples/README.md) for valid and deliberately rejected packages. Local integration tests exercise the actual CLI binary, every example, and signed archive tampering.

## Install, download, and update

```sh
gamepkg verify game.gamepkg
gamepkg install game.gamepkg --trust-creator
gamepkg install https://creator.example/game.gamepkg --trust-creator
gamepkg list
gamepkg update com.creator.game
```

Without `--trust-creator`, a first CLI install reports the creator fingerprint and waits for an explicit re-run accepting that creator. The GUI has a “Trust this creator and install” button. Existing packages need no new trust approval. Neither an embedded key nor a URL certifies a creator's name; obtain the first package from a source you trust.

Creators may set `update_url = "https://creator.example/latest.gamepkg"` at the top level of their input manifest. It points directly to the newest signed `.gamepkg`, not a JSON feed. The host downloads it only when the user requests an update; the downloaded ID, signer, and version must pass continuity checks. Packing inserts `creator_key` automatically; if supplied, it must match the signing key. The private signing key never goes inside the archive.

Downloads require the `curl` executable, enforce HTTPS for ordinary hosts, Tor-only routing for onion hosts, certificate verification for HTTPS, connection/transfer timeouts, and a 256 MiB transfer limit. No game code runs during installation. Host downloads are separate from guest network permissions, which remain `none`.

Installed archives reside under `GAMEPKG_HOME` when set, otherwise `$XDG_DATA_HOME/gamepkg`, `%LOCALAPPDATA%/gamepkg`, or `$HOME/.local/share/gamepkg`. Installation snapshots and verifies the bytes before replacement, uses an install lock, and leaves the old version intact on validation failure. An interrupted process may leave `.install-lock`; remove it only after confirming no install process is active.

For a local HTTPS end-to-end test (requires Python 3, OpenSSL, and curl), run `python3 tests/https_download.py` after the release build. It creates temporary certificates and packages, verifies URL installation and updates, and checks signer and ID substitution rejection. The server and temporary keys are cleaned up afterward.

## Library table, build actions, and ROM requirements

The desktop library shows each installed package's thumbnail, ID, version, ROM requirement, build status, and actions. Build becomes Rebuild once the exact installed package has a cached Wasm output. Installing a new package version resets that status. Play is enabled for built packages; ROM games require a file path in the library’s ROM field. The package must implement the grid-v1 entry point; packages without it report a clear runtime error and need a playable source implementation. Thumbnails decode with bounded dimensions and allocation; invalid icons show a fallback.

Set `requires_rom = true` at the top level of the manifest for a game requiring a ROM supplied by the end user. It defaults to false for existing packages and is covered by the creator signature. The host displays the requirement during inspection and in the library. No ROM is downloaded, uploaded, or included automatically. Select a local ROM path when launching; the guest receives bounded read-only bytes for that session.

`gamepkg build <id>` uses the same build action as the GUI. This initial build profile compiles verified `source/src/lib.rs` and approved host-owned crates through offline, locked Cargo to `wasm32-unknown-unknown` under Linux bubblewrap, without Cargo build scripts. It requires `bwrap`, `timeout`, `prlimit`, Rust, `rustup target add wasm32-unknown-unknown`, and the pinned Slang installation described below. The compiler has a private working directory, an isolated network namespace, access to system tools and its toolchain, and a 60-second deadline. It has no access to the user's home directory. Failed sandbox setup stops the build; there is no native or unsandboxed fallback. Compiler processes have a 2 GiB virtual address-space ceiling, core dumps disabled, output-file limits (32 MiB for Rust, 16 MiB for Slang), and bounded diagnostics (1 MiB per stream). These are per-process limits rather than a total process-tree memory budget. The initial approved crate is `byteorder-lite` 0.1.0 with default features disabled; broader GPU rendering remains pending.

`gamepkg check-updates <id>` checks the signed URL without changing the installed game. In the GUI, a newer update opens for review and explicit installation. Matching versions report up-to-date; older versions and changed IDs/keys are rejected. `gamepkg update <id>` remains the explicit download-and-install command.

After building the host, run `python3 tests/isolated_build.py` on a Linux machine with bubblewrap support to verify isolated compilation and rebuilding.

## Tor onion downloads and updates

`update_url` and the download field accept `http://<v3-address>.onion/latest.gamepkg` and HTTPS onion URLs. Ordinary hosts still require HTTPS. HTTP onion transport is permitted because onion-service connections are protected by Tor; package signatures and creator continuity checks still apply.

Run a local Tor SOCKS service on `127.0.0.1:9050`, or set `GAMEPKG_TOR_PROXY=127.0.0.1:9150` for a different local listener. This variable accepts a loopback IP and port, not a URL. The CLI and GUI both use it. GamePKG does not bundle or start Tor.

Onion downloads explicitly use `socks5h`, so the proxy resolves onion names. `NO_PROXY` cannot bypass this route; proxy failure stops the download with no direct fallback. Redirects are validated individually, and onion-to-ordinary-host redirects are rejected. Automatic curl configuration files are ignored. HTTPS onion URLs retain certificate verification.

Run `python3 tests/tor_download.py` after building to test remote DNS, Tor proxy routing despite `NO_PROXY=*`, signed installs/updates, and redirect rejection with a mock SOCKS5 server. This test does not contact or certify connectivity to the live Tor network.

## Bulk game lists and the not-downloaded state

A `.gamepkglist` is a UTF-8 TOML catalog, not an archive. Import adds game metadata to the library without downloading game packages, thumbnails, ROMs, or granting creator trust. Remote imports download only the list itself. See [starter.gamepkglist](examples/lists/starter.gamepkglist) for a complete example with placeholder URLs.

```toml
format_version = 1

[[games]]
id = "com.creator.game"
title = "Example Game"
download_url = "https://creator.example/game.gamepkg"
version = "1.0.0"     # optional catalog claim
requires_rom = false # optional catalog claim
```

```sh
gamepkg import-list starter.gamepkglist
gamepkg import-list https://creator.example/games.gamepkglist
gamepkg list
gamepkg download com.creator.game --trust-creator
```

The GUI's shared path/URL field has an **Import .gamepkglist** action. Imported games appear in the table as **Not downloaded**, with a placeholder icon and **Download / inspect** action. Version and ROM claims from the catalog are unverified; the signed manifest becomes authoritative after downloading. The package ID must match the requested catalog ID, and creator approval is still required before first installation. Existing installed packages appear only once and retain their verified metadata and signer continuity rules.

Lists support the same HTTPS and Tor onion URLs as packages, have a 4 MiB/10,000-game limit, reject unknown fields and duplicate IDs, and are imported atomically. Reimporting identical entries is harmless. Conflicting metadata for existing catalog IDs rejects the entire import rather than silently replacing download sources. Catalog metadata persists in `catalog.gamepkglist` under the library directory; it contains no game binaries or creator trust approvals. A crashed import may leave `.catalog-lock`; remove it only after confirming no import process is active.

## Play the first game

After building both release binaries, run `./play-demo.sh`. It signs the bundled example with a temporary local demo key, installs it in `.demo-library`, builds it in isolation, and opens the desktop player. The private demo key is deleted; this separate demo library does not change your normal installed games.

`examples/good/collect-stars` is a playable guest whose movement and scoring run inside Wasmtime. Pack and install it using your creator key, then build and play:

```sh
gamepkg pack examples/good/collect-stars -o collect-stars.gamepkg --key /path/to/creator.key
gamepkg install collect-stars.gamepkg --trust-creator
gamepkg build com.gamepkg.collect-stars
gamepkg run com.gamepkg.collect-stars
```

In the terminal, enter W/A/S/D to move and Q to quit. In the GUI, click Play and use arrows or WASD; collect yellow targets to increase your score. Restart resets the game; Back to library unloads it. The desktop executable also accepts `--library <directory> --play com.gamepkg.collect-stars`.

The legacy grid-v1 ABI uses core Wasm. The versioned WIT component profile is described below. Guest source defines `pub fn game_tick(input: u32, state: u64) -> u64`; the host compiler appends the exported wrapper, so guest export attributes are unnecessary. State packs player x/y and target x/y into four bytes, followed by a 32-bit score. Input bits are left=1, right=2, up=4, down=8. Coordinates use a 16×16 grid. Guests receive zero input unless the signed manifest grants input.

Runtime modules can import only the scoped GamePKG asset, ROM and save APIs; WASI, filesystem and socket imports are rejected. Wasmtime enforces memory, fuel and epoch limits. A trap ends gameplay. Slang compilation is available; Resource-free GLSL fragment passes render in the desktop player.

For a game declaring `requires_rom = true`, use `gamepkg run <id> --rom /path/to/file` or enter your ROM path above the desktop library table. ROM files must be regular files of 1 byte to 64 MiB and are read into a session snapshot. Guests use `gamepkg::rom_size()` and `gamepkg::rom_read(offset, &mut buffer)`, never a host path. Reads are capped at 64 KiB and consume fuel. No ROM is downloaded, installed or bundled automatically. `examples/good/rom-required` is a playable data-driven example; `python3 tests/runtime_rom.py` tests the signed build and runtime path.

Run `python3 tests/playable.py` after the release build to test the complete signed-package, isolated-build, movement and scoring path. Runtime tests separately exercise forbidden imports, missing exports, input permission denial, excessive memory, infinite loops and epoch interruption.

## Recommended memory and hard memory limit

Creators set both values in the signed manifest:

```toml
[limits]
recommended_memory_bytes = 8388608 # 8 MiB, advisory
hard_memory_bytes = 16777216       # 16 MiB, enforced
fuel = 1000000
```

Recommended memory describes the creator's suggested budget; it neither reserves memory nor stops a game from using more. Wasmtime enforces the hard limit on guest linear memory, including initial allocation and growth. It is not a cap on the entire host process, compiler, or JIT memory. The GUI displays both values during inspection and in the installed-state tooltip.

Recommended memory is optional and defaults to the hard limit. When supplied, it must be positive and no greater than the hard limit. The hard limit must be positive and at most 512 MiB. Existing `memory_bytes` remains accepted as an alias for `hard_memory_bytes`; supplying both is rejected, and newly packed manifests use the canonical hard-limit field.

## Desktop settings

Open **Settings** from the desktop library navigation. Save applies validated preferences; Cancel discards form changes, and Reset restores defaults in the form until you save.

- Library folder: choose an absolute path or leave empty for the default. Switching libraries does not move existing packages.
- Tor proxy: local loopback IP and SOCKS port; Tor remains a separately running service.
- Game ceilings: memory (1–512 MiB), fuel (1–100 million per tick), and tick deadline (10–100 ms). Memory/fuel use the lower of the saved ceiling and the signed game limit. New sessions use the settings; current games are unaffected.
- Optimize builds: enables Rust `opt-level=2` for subsequent Build/Rebuild; existing artifacts remain available.
- Appearance: dark or light theme, applied after saving.

Settings persist in `$XDG_CONFIG_HOME/gamepkg/settings.toml`, `%APPDATA%/gamepkg/settings.toml`, or `$HOME/.config/gamepkg/settings.toml`. Set `GAMEPKG_CONFIG` to override the config file. `GAMEPKG_HOME` (including GUI `--library`) and `GAMEPKG_TOR_PROXY` override the saved library and proxy respectively. The CLI shares library, proxy, build and runtime settings. Invalid configuration is reported rather than silently weakening restrictions.

## Guest asset reads

The isolated compiler supplies `gamepkg::asset_read(path, offset, &mut buffer)`.
Paths are relative to `assets/` in the signed archive. The function returns
`Some(bytes_read)`, including zero at EOF, or `None` for invalid paths, offsets,
or missing assets. Reads are limited to 64 KiB per call; archive work consumes
runtime fuel. Verified stored assets use direct archive offsets; compressed assets
stream through the prefix without allocating the entire file. Fuel charges cover
the requested bytes plus any decoded prefix. The guest receives only the requested bytes and cannot open host
files. The same safe Rust wrappers are available in the versioned WIT component
profile. Rebuild existing games to use the new wrapper.

Run `python3 tests/runtime_assets.py` after building the release CLI to verify
asset reads and path rejection through signing, isolated compilation and play.

## Scoped saves

Games declaring `permissions.save = true` can use `gamepkg::save_size()`,
`gamepkg::save_read(offset, &mut buffer)` and `gamepkg::save_write(&bytes)`.
The host keeps one blob of at most 1 MiB under the library’s `saves` folder,
scoped by the signed game ID and creator key. Same-creator updates retain the
blob; another game or creator cannot select it. Reads are capped at 64 KiB.
Denied permissions return `None` or `false` without opening save storage.
Writes consume fuel and replace the blob atomically only after a successful
tick. A trapped tick discards its pending save. Concurrent sessions use the
last successfully committed write. The version-one blob format belongs to the
game; creators should handle migration when changing their save layout.

`examples/good/save-counter` keeps its tick count across sessions. After a
release build, `python3 tests/runtime_saves.py` verifies persistence, updates,
permission denial and trap rollback.

## Pinned shader builds

Build/Rebuild now compiles the signed Slang shader source in a network-isolated
Linux process before publishing the Wasm artifact. Install the supported
compiler into a new folder:

```sh
python3 tools/install_slang.py /absolute/path/gamepkg-slang
export GAMEPKG_SLANG_ROOT=/absolute/path/gamepkg-slang
```

The installer downloads the official Slang 2026.19 Linux x86-64 release and
checks a pinned SHA-256 before unpacking it. Alternatively set the installation
folder and backend in the desktop Settings page. New configurations default to
GLSL for shader playback in the OpenGL desktop player. SPIR-V, WGSL, Metal,
and DXIL compilation outputs are also tested. DXIL requires the pinned
DXC 1.9.2609 Linux x86-64 distribution:

```sh
python3 tools/install_dxc.py /absolute/path/gamepkg-dxc
export GAMEPKG_DXC_ROOT=/absolute/path/gamepkg-dxc
```

The installer verifies the official release archive checksum. The host checks
both DXC library hashes before compilation, mounts them read-only inside the
build sandbox, and includes the pinned DXC identity in the shader cache key.
Missing or modified libraries fail the build. No backend fallback occurs. Select GLSL, SPIR-V, or WGSL and rebuild to use resource-free
fragment passes in the OpenGL desktop player. SPIR-V is parsed and validated by
pinned Naga 24.0.0, then translated to host-owned GLSL 330 before graphics setup.
Metal and DXIL rendering integration remains pending.

Legacy manifests default to `shaders/main.slang`, `fragmentMain`, fragment
stage. Declare multiple entry points explicitly using signed manifest entries:

```toml
[[shaders]]
source = "shaders/main.slang"
entry = "fragmentMain"
stage = "fragment"
```

Supported stages are vertex, fragment and compute, with up to 32 entries.
Compiler outputs and JSON reflection are cached outside the archive, keyed by
package bytes, compiler version/binaries and backend. A failed shader build
publishes no partial cache or new playable artifact. The compiler sees its
own tools, system libraries and staged shader sources; your home directory
and network are unavailable. Run `python3 tests/shaders.py` with
`GAMEPKG_SLANG_ROOT` set to verify actual backends, reflection and host include
rejection.

## Guest drawing

Guests can draw their own scene with `gamepkg::draw_clear(rgba)`,
`gamepkg::draw_rect(x, y, width, height, rgba)` and
`gamepkg::draw_circle(x, y, radius, rgba)`. These safe wrappers send bounded
commands to the desktop renderer; games never receive a graphics-device handle.
Coordinates use a 640 × 480 canvas and colors use `0xRRGGBBAA`.
Each frame keeps at most 1,024 commands. Non-finite coordinates and oversized
geometry are rejected, calls consume fuel, and trapped ticks discard the frame.
The desktop clips commands to the game viewport and scales the canvas to fit.

Input bits 0–3 report newly pressed left/right/up/down directions as before.
Bits 8–11 report held directions (`HELD_LEFT`, `HELD_RIGHT`, `HELD_UP`,
`HELD_DOWN`). Disabling input permission clears both sets of bits.

`examples/good/shape-playground` draws an animated scene and moves a square
with held arrows/WASD. Play it through the GUI or run it headlessly using
`--frames`. Interactive terminal playback remains for the grid profile.
`tests/drawing_game.py` checks the signed source/build/runtime path;
`tests/drawing_gui.py` uses a virtual display and actual screen pixels to
verify rendering and held-key movement. The latter requires Xvfb, xdotool,
ImageMagick and Pillow. Set `GAMEPKG_SLANG_ROOT` for both tests.

The host also exposes `gamepkg::draw_shader(index)`, which draws a full viewport
pass using the signed manifest's fragment entry at that index. Select GLSL, SPIR-V, or WGSL in
Settings and rebuild. The host checks reflected resources and active uniforms,
keeps graphics handles private, and bounds the per-tick command queue. Guest
Wasm fuel does not meter GPU shader execution; GPU drivers and shader compilation
remain part of the trusted computing base. This does not guarantee that an
arbitrary shader cannot stall or crash a driver.

`examples/good/shader-switch` switches two compiled fragment passes with held
Right input. `tests/shader_gui.py` verifies the actual blue and green pixels in
an isolated display. Textures, external bindings, vertex/compute pipelines,
other rendering backends remain pending.

Hosted Linux build integrations run controlled fixtures with elevated namespace
setup privileges because the runner rejects unprivileged network namespace
configuration. The actual compilers still run inside bubblewrap with a private
network and filesystem. The application continues to fail closed when sandbox
setup is unavailable; it does not use an unrestricted build fallback.

## Bounded audio output

Games declaring `permissions.audio = true` can submit signed 16-bit mono PCM
at 48 kHz with `gamepkg::audio_submit(&samples)`. A tick may submit at most
1,600 samples in total; the device queue keeps at most 100 ms. Invalid memory
ranges, excess samples and denied permissions are rejected. Calls consume
fuel, and trapped ticks discard their audio. Guests receive no device handles
and cannot access microphones. The host resamples for the selected output
rate and handles volume.

The desktop includes audio by default. **Settings → Sound** offers mute and
volume controls for new play sessions. Missing or disconnected devices produce
a visible sound error while gameplay continues. Back to Library stops the
output stream. Headless CLI runs generate and check audio without playing it;
interactive terminal builds need the `audio` feature for sound output.

On Linux, desktop/audio builds require ALSA development headers
(`libasound2-dev` on Debian/Ubuntu). `examples/good/tone-pad` generates a quiet
triangle wave and changes pitch with held Up/Down. `tests/audio_game.py` tests
its signed build and PCM generation. The ignored `audio_output_null_device`
test is run explicitly with an ALSA null device to verify the actual output
callback and mute behavior without speaker playback:

```sh
echo 'pcm.!default { type null }' > /tmp/gamepkg-null-alsa.conf
ALSA_CONFIG_PATH=/tmp/gamepkg-null-alsa.conf cargo test --all-features audio_output_null_device -- --ignored --test-threads=1
```

Hosted privileged build fixtures stage toolchains under `/tmp` so namespace
root can traverse the compiler paths without access to runner home folders.

## Engine compatibility and save schemas

Signed manifests may declare `engine_version` as a semantic-version requirement
and `save_schema` as a positive integer. Existing manifests default to
`engine_version = "^0.1.0"` and `save_schema = 1`.

```toml
engine_version = ">=0.1.0, <0.2.0"
save_schema = 1
```

Verification and runtime loading reject unsupported engine requirements with
a message showing the required and provided versions. Engine compatibility
is also checked before installation, so an incompatible update cannot replace
a working game.

Saves are scoped to the creator, game ID and declared schema. Schema 1 retains
the existing storage path. Changing schemas opens a fresh blob and preserves
previous-schema bytes; switching back restores that namespace. No conversion
or deletion happens automatically. Keep the same schema when the game can
read its existing save layout; use a new schema for a deliberately separate
format. Automatic migration between namespaces remains pending.

The library’s **Details** button verifies an installed package and shows its
engine requirement, save schema, permissions, creator fingerprint, ROM
requirement and signed lockfile inventory hash. It does not stage a download
or install anything. Creator keys normalize to their byte identity so hex
letter case cannot create different trust or save identities.


## Versioned WIT components

Set `runtime_profile = "component-v1"` at the top level of the signed manifest
with `entry_world = "game"`. Omission preserves the existing `core-v1` profile.
The authoritative contract is [`wit/game.wit`](wit/game.wit), package
`gamepkg:engine@0.1.0`, world `game`. It exports a typed `tick(input: u32,
state: u64) -> u64` and imports only `gamepkg:engine/host@0.1.0`.

Guest source still defines `game_tick` and uses the safe `gamepkg` wrappers.
The isolated compiler appends host-owned canonical ABI bindings, compiles source
into core Wasm, and encodes a validated component outside the signed archive.
No guest-supplied prebuilt component, WIT file, WASI interface, build script or
additional dependency is accepted. CLI and GUI load components through Wasmtime's
component linker with the same fuel, epoch deadline, input permission, memory
ceiling, draw/audio bounds and transactional save behavior as the core profile.
Component instances allow four internal modules and two bounded tables for the
canonical ABI; linear memory remains limited to one memory and the signed/user
ceiling. Host list copies are additional bounded host allocations, outside the
linear memory ceiling.

Assets, ROM and save reads use typed optional byte lists. Save writes and audio
use typed lists; drawing returns a boolean success value. Paths stay virtual,
ROMs remain explicitly selected, and saves remain scoped to identity/schema.
`examples/good/component-playground` is a playable component source example.
Run `tests/components_game.py` for signed build, typed list transfers, permission
denial and save rollback, and `tests/component_gui.py` for rendering and held
movement verified from actual screen pixels. The GUI test requires the same
virtual display tools as the drawing test.


## Periodic update checks

The desktop library checks installed games with signed update URLs automatically
at startup and every 24 hours while the library remains open and idle. Settings
can disable these checks or change the interval to 1–168 hours. Catalog entries
that have not been downloaded are excluded. Checks use the same HTTPS and Tor
transport, signature verification and creator/ID continuity as manual checks.

One request runs at a time. Each check downloads the complete package to verify
it, then discards that download; it never installs, builds or replaces a game.
An available version appears in the library table. Use **Check for updates** to
fetch it again for review and explicit installation. Failures appear in the table
and wait until the next interval rather than retrying continuously. Turning checks
off prevents new requests; a request already running may finish. No update service
runs after the desktop app closes. Restarting the app starts a fresh check cycle.

`tests/updates_gui.py` uses a local HTTPS server and isolated display to verify
that a signed update notice renders, the installed archive stays unchanged,
no early repeat request occurs, and disabled checks make no requests.


## Compiler resource and source-policy checks

Compiler discovery and version probes also use the process deadline and bounded
capture. Compiler stdout/stderr are drained after the capture limit so verbose
errors do not deadlock a build or grow the host's diagnostic buffer indefinitely.
No truncated or oversized artifact is published, and component encoding is checked
against the final 32 MiB artifact limit. A failed rebuild leaves earlier compiled
output intact. `tests/compiler_limits.py` exercises an oversized real Rust artifact;
ordinary Rust, component and Slang build integrations verify successful builds.

The source scanner checks identifiers inside macro token groups and import aliases,
including raw identifiers. Hidden includes, unsafe/foreign code, restricted OS
names, attributes and module declarations inside macros are refused. This initial
policy is conservative: the reserved identifiers are rejected even when used as
ordinary variable names; string literal text is allowed. Normal expression macros
remain supported. Scanning supplements compiler isolation and Wasm linking; it is
not a proof that source code is harmless.

## Game previews

Packages may include `preview.av1.webm` (AV1 in WebM) and/or `preview.ogv`
(Theora in Ogg). Previews are signed along with the game. The current profile
accepts one silent video stream, at most 16 MiB, 640×480, 30 seconds, and 900
frames, starting with a keyframe. Damaged videos, wrong codecs and oversized
previews are rejected before installation.

On Linux, install FFmpeg and bubblewrap to verify packages with previews.
Video inspection and decoding run in a separate process with no network, a
private temporary directory, a 2 GiB address-space ceiling and a 60-second
timeout. The desktop **Details** view offers **Play preview** and **Play Theora
fallback**. Playback decodes at 320×240 and five frames per second, retains at
most 150 frames, and never starts the game. A missing preview produces an
error; preview decoding is currently supported on Linux only.

`gamepkg preview game.gamepkg` checks and decodes the preferred preview;
`--theora` selects the fallback explicitly. Run `python3 tests/previews.py`
after building release binaries to exercise real AV1/Theora videos, signing,
verification, decoding, and rejected malformed videos.

`python3 tests/previews_gui.py` uses a private Xvfb display to verify that
AV1 frames advance, the explicit Theora fallback renders, and preview playback
leaves the game unbuilt. It requires Xvfb, xdotool, ImageMagick, and Pillow.

## Shared asset store tooling

`gamepkg pool-import game.gamepkg` verifies a signed, self-contained package
and adds its assets to the library's `asset-pool` directory. Identical content
from different games occupies one blob named by its SHA-256. Imports do not
install games, approve creators, or run game code. Original packages remain
self-contained and keep their signatures.

The command prints `sha256 size virtual-path` for each asset. Use
`gamepkg pool-read <sha256> --size <bytes> -o output.bin` to verify and copy a
blob. Every read checks the exact size and content hash. Writes publish without
replacing existing blobs; a corrupted blob causes an error. Symlinked blobs and
pool folders are rejected. Individual blobs are limited to 64 MiB.

Store packages can now sign external asset references and use them through the
same scoped runtime API. Ordinary sideloaded packages continue reading their
assets directly from their own archives.

To create a store package, use:

```sh
gamepkg pack /path/to/game -o store.gamepkg --key creator.key --shared-assets
```

This adds assets to your current library's pool and replaces embedded assets
with signed `shared_assets` entries in the manifest. Each entry binds an
asset-relative path to its SHA-256 and exact size. The original source folder
is unchanged. Missing blobs are never fetched automatically; provision a
recipient's pool with `gamepkg pool-put /path/to/asset` or import the original
self-contained package with `pool-import` before installing its store variant.

Signature verification works offline without the pool. Installation, building,
and playing additionally require every referenced blob to match its signed
hash and size. Runtime reads use private session snapshots and the same 64 KiB
range limit and fuel accounting as stored archive assets. A changed or deleted
pool blob cannot alter an active session. Logical package contents, including
shared references, remain bounded to 256 MiB. Conflicting sizes for one hash,
invalid paths, and assets declared both embedded and shared are rejected.

Run `python3 tests/shared_assets_game.py` to verify both core and WIT source
games using signed store packages, missing-blob refusal, deduplication, and
small reads from large shared assets. Portable store bundles support remote distribution as described below. Pool
garbage collection remains future work.

## Portable store bundles

A `.gamepkgstore` ZIP transports signed games and their shared assets together.
Each game retains its original creator signature, and identical shared content
is included once. The bundle contains `store.toml` (`format_version = 1`),
`packages/<signed-id>.gamepkg`, and `blobs/<sha256>.blob`. Creator approval
applies to the individual enclosed games.

```sh
gamepkg store-export first.gamepkg second.gamepkg -o games.gamepkgstore
gamepkg store-inspect games.gamepkgstore
gamepkg store-import games.gamepkgstore --trust-creators
```

Export reads shared blobs from your current library and refuses to overwrite
files. Inspect verifies every package and blob and lists creator keys without
installing anything. Import also accepts HTTPS and Tor onion URLs. First
installs require `--trust-creators`; existing games enforce the same signer and
a higher version, and an identical package is skipped. No game is built or run
during import.

All contents are staged and verified, and creator/version conflicts are checked
across the whole batch before library publication. Malformed packages,
unreferenced/missing/corrupt blobs, unexpected files, and unsupported versions
are rejected. Bounds are 10,000 entries, 64 MiB per entry, and 256 MiB per
bundle. Individual blobs and games publish atomically; an I/O failure or
concurrent library mutation can interrupt a batch after some games install.
Reimporting the same bundle safely skips completed games.

`tests/shared_assets_game.py` additionally transfers a bundle over local HTTPS
to a fresh recipient library, then builds and plays both enclosed runtime
profiles. `tests/store.rs` checks deduplication, approval, repeat imports,
conflicts, and deliberately malformed bundles.

## Approved guest crates

The host currently approves `byteorder-lite` 0.1.0 for endian-aware binary asset
reading, with all default features disabled:

```toml
[dependencies.byteorder-lite]
version = "=0.1.0"
default-features = false
```

The signed `source/Cargo.lock` must match the exact root package and approved
dependency graph, including the registry SHA-256. Alternate sources, aliases,
features, version ranges, extra locked packages, build scripts and procedural
macros are rejected. The host bundles the checksum-verified crate source and
its licenses; guest packages do not supply dependency source.

Builds stage a host-owned local dependency and a matching lockfile, then run
`cargo build --offline --locked --release --lib --target wasm32-unknown-unknown`
inside the existing network-isolated sandbox with a private Cargo home. The
original package lockfile remains unchanged and covered by its signed hash.
No registry access or dependency downloads occur during gameplay builds.

`examples/good/byteorder-game` is playable and reads a big-endian score from an
archived asset. `tests/approved_crate_game.py` builds, runs and rebuilds it under
both core and WIT profiles. The catalog ships with the host; local approval
is configurable through `allowlist add/remove` and desktop Settings.

## Game source modules

Games may organize source under `source/src/` using ordinary `mod name;`,
`name.rs`, `name/mod.rs`, and inline modules with separate child files. Define
`game_tick` in the root `src/lib.rs`; it can delegate to module functions. The
host appends its trusted API and export wrapper only to that root file.

Every packaged Rust file is checked, including unused files. Module declarations
must resolve to exactly one packaged file. `#[path]` and other guest attributes,
macro-generated module declarations, includes, build scripts, and non-Rust
source files are rejected. Source paths are limited to 1,024 bytes and 32
segments. Only checked files enter the private offline Cargo workspace.

`examples/good/modular-game` splits a playable game into movement, nested score
reading, and constants modules. `tests/approved_crate_game.py` builds and plays
it under both runtime profiles; `tests/modules.rs` checks missing/ambiguous
modules, path redirection, forbidden unused files, and malformed source.

Builds require a callable root `game_tick(u32, u64) -> u64`, including a
re-export from another module. Missing or incompatible entry points and
unsupported entry worlds fail before a build is published. The library checks
cached Wasm validity and the root tick signature before offering Play; older
caches without that entry point offer Build/Rebuild. The minimal, counter, and
asset-catalog examples now provide playable entry points.

`tests/build_entrypoints.py` covers successful core/WIT launches, reexports,
wrong/missing entry points, unsupported worlds, and preservation of earlier
caches after failed builds.

### Import portable stores in the desktop library

Enter a local `.gamepkgstore` path, HTTPS URL, or Tor onion URL in the Library source field, then choose **Inspect .gamepkgstore bundle**. GamePKG verifies every package signature and shared blob before showing the game versions, ROM requirements, and creator keys. Inspection does not install games. Choose **Trust listed creators and install store** to approve first installations, or **Cancel store import** to discard the review. Existing creator-key and version protections still apply. Installation uses a private verified snapshot, so changes to the original file cannot replace reviewed content. Imported games appear in the table and can be built and played.

Cargo package names use at most 64 ASCII letters, digits, underscores, or hyphens, beginning with a letter or underscore. Explicit editions must be string values `2015`, `2018`, `2021`, or `2024`; omitting the field retains Cargo's default. Invalid identity or edition metadata is rejected during package verification before it reaches the isolated compiler.

When DXIL is selected, Settings also provides a **DXC 1.9.2609 installation folder** field. The saved folder is shared by CLI and desktop builds. `GAMEPKG_DXC_ROOT` overrides the saved folder for the current launch; both paths require the same pinned library checksums. Older settings files default to an empty DXC folder.

### Host crate approvals

`gamepkg allowlist list` shows the bundled crate catalog, pinned version, checksum, and current approval. `gamepkg allowlist remove byteorder-lite` revokes approval; `gamepkg allowlist add byteorder-lite` restores it. Approval persists in the same settings file used by the desktop and CLI. The default policy approves the bundled byteorder-lite 0.1.0 library. Unknown crates cannot be added: extending the catalog requires host-reviewed, bundled source and an exact dependency graph. Revocation affects subsequent package verification, installation, and builds; it does not interrupt an already running game. Libraries containing revoked dependencies may report verification errors until approval is restored.

Desktop Settings provides the same policy under **Approved game libraries**. Changes apply only after **Save settings**; **Cancel edits** reloads the saved approval. Expand **Pinned library details** to view the catalog checksum and source restrictions. Resetting the form restores the default approval; save to apply it.

The desktop library and `gamepkg list` inspect installed files independently. A damaged archive, mismatched filename, or package rejected by current host policy appears as **Unavailable** with its filename and verification error; other verified games remain visible. Rejected files do not supply trusted game metadata or Play controls. Installation and store conflict checks continue to use strict verification and fail when the existing library cannot be verified.

If the library folder becomes unreadable, refreshing the desktop library clears stale rows and shows the folder error. `tests/library_gui.py` checks the actual rendered screen for a verified game beside a rejected archive, then verifies that no old rows remain after the folder becomes unreadable. The virtual-desktop test uses Tesseract to read the displayed labels.

### Build a creator source directory

```sh
gamepkg build examples/good/collect-stars -o /tmp/collect-stars.gamepkg --key creator.key
```

With `-o` and `--key`, `build` signs a temporary candidate, verifies it, and compiles its Rust and Slang source using the same isolated toolchain as installed games. Only a successful build publishes the signed source `.gamepkg`. The package retains source and assets; it does not embed the temporary Wasm or shader cache. This workflow does not install into your library. Missing signing keys, existing output files, or compilation failures leave no new output; existing files are never overwritten. Install the result and Build/Play it as usual. Without `-o`, `gamepkg build <id>` continues to compile an installed game. `pack` remains available to sign and validate source without requiring the compiler toolchain.

Creator directory-build outputs must be outside the source tree, including when their parent folder is a symlink. The destination parent must already exist. Existing files and symlinks are refused before compilation; publication still uses an atomic no-overwrite operation after successful compilation.

Existing explicitly saved shader backends remain selected. For desktop shader playback, choose GLSL and rebuild if an older configuration selected another backend. New installations and Reset form to defaults use GLSL so supported shader games can be built and played without an extra configuration step.

SPIR-V desktop playback accepts a single fragment entry with the bounded texture/sampler pair or no resources per cached shader. The host bounds binary input and translated output to 16 MiB, checks Slang reflection, validates SPIR-V capabilities and module structure, rejects unsupported resource bindings, and checks translated uniform/texture reflection before OpenGL setup. `tests/shader_gui.py --backend spirv` and `--backend spirv --component` verify actual shader pixels, held input, and malformed-cache rejection. Vertex/compute pipelines and other native rendering backends remain unfinished.

WGSL fragment outputs also render through the pinned Naga validation and bounded GLSL translation path. Both WGSL and SPIR-V reject unsupported bindings and non-fragment entries before graphics setup. `tests/shader_gui.py --backend wgsl` and `--backend wgsl --component` check actual pixels, input, and corrupted cache rejection. Metal and DXIL compilation remains supported, but their desktop rendering paths remain unfinished.

Desktop shader loading also requires reflection to describe exactly one entry point whose name matches the signed manifest request. Missing or mismatched entry metadata fails before playback, alongside resource-binding checks. The GUI shader tests exercise these rejection cases as well as valid rendered frames.

Fragment builds for the desktop GLSL, SPIR-V, and WGSL backends validate reflected entry points and reject unsupported resource bindings before publishing shader or game caches. SPIR-V and WGSL builds also validate their translation to the desktop renderer. A failed rebuild preserves earlier compiled artifacts.

The static site includes browser-readable [format notes](html/spec/index.html), [security model](html/guide/security.html), and [library policy](html/guide/allowlist.html). After editing the specification or security Markdown sources, run `python3 tools/render_site_docs.py` (requires Python-Markdown, packaged as `python3-markdown` on Debian/Ubuntu) and commit the generated HTML. The site serves directly from `html/` without a server-side renderer.

Games can draw RGBA sprites with `gamepkg::draw_image(x, y, width, height, pixel_width, pixel_height, &pixels)` in either runtime profile. Pixels use row-major red/green/blue/alpha bytes, with exactly four bytes per pixel. Images must be 1–512 pixels in each dimension. The host snapshots accepted pixels, charges fuel for copying, checks guest memory ranges, and limits queued sprite bytes to 8 MiB per frame within the existing 1,024-command limit. Frame resets and trapped ticks discard queued images. The desktop draws sprites with nearest-neighbor sampling and alpha transparency. Shader texture access uses the separate bounded API described below. See `examples/good/sprite-playground` and `tests/sprites_gui.py`.

The desktop reuses sprite textures when their pixel content is unchanged, updates the existing texture when pixels change, and releases textures when sprites disappear from the current frame or play ends. `tests/sprites_gui.py` verifies live pixel changes through held input in both runtime profiles.

Use `gamepkg::draw_asset_image("sprite.png", x, y, width, height)` to draw a PNG from a verified virtual asset path in either runtime. Embedded files and verified shared-pool snapshots work through the same archive API. Encoded PNGs are capped at 2 MiB, decoded dimensions at 512×512, and decoder allocations at 8 MiB. The host reserves fuel for bounded archive reads, decompression prefixes, and pixel decoding before processing the complete image. Images remain subject to the 8 MiB frame budget; missing, malformed, oversized, or traversal paths return false. These host rendering budgets are separate from guest linear-memory limits. See `examples/good/png-sprite-playground` and `tests/sprites_gui.py --png`.

Games can supply a texture to a supported fragment pass with `gamepkg::draw_shader_image(index, pixel_width, pixel_height, &rgba)`. The Slang pass may declare exactly one `Sampler2D gamepkgTexture` at binding 0. For SPIR-V, the host defines `GAMEPKG_SEPARATE_SAMPLER=1`: use `Texture2D<float4> gamepkgTexture` and `SamplerState gamepkgSampler` at bindings 0 and 1 in that branch, as the example demonstrates. Additional uniforms, samplers, arrays, storage buffers, and texture bindings remain rejected. Both core and WIT profiles snapshot pixels before returning, charge copying fuel, and share the 8 MiB frame image budget and 512×512 dimension limits with sprites. The renderer owns one reusable texture, uses nearest-neighbor sampling and clamp-to-edge, and replaces its contents before each pass; a plain `draw_shader` uses a transparent 1×1 texture so another pass's image cannot leak. The shader receives straight RGBA and must produce the renderer's expected output alpha convention. Textured passes support GLSL, SPIR-V, and WGSL. See `examples/good/shader-texture` and `tests/shader_gui.py --texture` (also `--component`).

Translated texture passes validate one non-arrayed, non-multisampled float 2D image at group 0/binding 0 and one non-comparison sampler at binding 1. Naga validates the complete module and its generated texture mapping before the renderer binds any pixels. Slang's combined WGSL sampler reflection occupies two descriptor slots; SPIR-V uses the explicit separate-sampler source branch. Input and output retain the 16 MiB translation bounds. Changing the SPIR-V compiler definition produces a new shader cache identity; rebuild affected games. The CI tests actual texture pixels, held-input changes, transparent fallback, and malformed caches in both runtime profiles for both translated backends.
