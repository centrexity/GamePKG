# GamePKG — Play games safely

**Know what you run. Control what it can access.** GamePKG is building a safer way to share and play games: inspectable source, creator-signed packages, isolated compilation, and a host that is designed to grant only the capabilities a game needs.

**Current status:** this is a development preview. Package verification, catalog import, installation, update checks, and isolated single-file Wasm and Slang shader builds on Linux work today. The experimental grid-v1 profile now plays in the GUI and terminal under Wasmtime with scoped asset reads, fuel, memory limits, and epoch deadlines. Resource-free Slang fragment passes now render in the OpenGL GUI; versioned WIT components are supported alongside the legacy core profile; broader rendering remains pending; this is not a guarantee that a package is harmless.

## Why distribute GamePKG instead of an opaque executable?

A conventional game executable can be difficult to inspect, and launching it without a sandbox may give it the permissions of your user account. That can include reading personal files, making network connections, or starting other programs. Signed binaries and separately sandboxed applications can also provide protections; the benefit of GamePKG is making source visibility, package verification, restricted builds, and declared capabilities part of one distribution workflow.

A `.gamepkg` contains Rust source, Slang shader source, assets, and a signed manifest. You can inspect the code and declared permissions before choosing to install it. The host checks the package, compiles its supported source profile into Wasm, and keeps game assets in the archive. The intended runtime will expose a small game API rather than the operating system's full set of interfaces.

| Protection | What it changes | Available today? |
| --- | --- | --- |
| Inspectable source | Games ship source rather than a bundled native executable; review is possible before building. Source visibility alone does not prove safety. | Yes |
| Signed identity and updates | The manifest and package contents are signed. Updates must keep the installed creator key and package ID, with a higher version. | Yes; first creator approval is still required |
| Restricted package contents | Rejects traversal paths, duplicate entries, symlinks, oversized archive contents, unsupported shaders, and source outside the initial policy. | Yes |
| Isolated compilation | Supported Linux builds run inside bubblewrap with a private work area, no network, and no access to your home directory. | Yes; single-file, dependency-free profile |
| Limited game access | Games will receive only host-approved asset, input, rendering, audio, and save operations. | grid-v1 supports scoped asset reads; broader host APIs planned |
| Runtime resource limits | Memory caps, execution fuel, and deadlines will constrain running games. | Enforced for grid-v1 Wasmtime execution |

## Build locally, improve over time

Building on demand on your own machine creates opportunities to optimize games for the supported platform and runtime instead of relying on a single prebuilt executable. Shipping source also lets the host rebuild games as compiler tooling and optimization settings improve. Creators can release signed source updates, and the host can check periodically for newer versions while preserving creator-key continuity.

These are opportunities, not a promise that every local build will be faster. GamePKG currently supports manual isolated Wasm builds, manual rebuilds, and user-requested update checks. Platform-specific performance tuning and automatic periodic update checks are planned. Wasm builds must stay within their supported target and sandbox; local compilation does not grant games extra access to your machine.

## What can a GamePKG do—and what can it access?

Today, importing a `.gamepkglist` adds catalog entries without fetching games or trusting creators. Downloading fetches the package you select. Installation verifies and stores it; it runs no game code or package-provided install scripts. Update checks stage a signed candidate for review. Supported builds compile the verified Rust source to Wasm inside an isolated Linux process.

The initial package policy allows one Rust source file and no dependencies. It rejects build scripts, proc-macro configuration, compile-time file includes, unsafe blocks, foreign declarations, and filesystem, network, process, or environment paths detected by the source scanner. Shader files must be Slang source. The scanner is a policy filter, not a substitute for compiler isolation or a runtime sandbox.

The runtime design starts with **no ambient filesystem or network access**. A game will request specific capabilities such as input, audio, or saving through the host. Asset access will use virtual paths inside its own archive; save access will be scoped by the host rather than exposing your folders. A game requiring a ROM declares that requirement, and the player supplies it. ROM bytes are shared read-only only for the selected play session.

Guest networking is currently unsupported: manifests must declare `network = "none"`. User-requested host downloads over HTTPS or Tor are separate from game permissions. A signed update URL does not give the game permission to browse the internet. Signatures establish integrity and continuity with a creator key, not a creator's real-world identity or a promise that their code is safe.

## Try it

Install Rust, then run:

```sh
cargo test --locked
cargo run -- --help
```

A complete package input is in `examples/minimal`. Generate a private signing seed and its public key with your Ed25519 key tooling. The private key file must contain exactly 32 bytes encoded as 64 hexadecimal characters. Keep it outside your package directory and restrict access to it.

```sh
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
- Only one Rust source file, no dependencies, `package.build = false`, and restricted Cargo fields. Includes, attributes, external modules, unsafe blocks, foreign declarations, and filesystem/network/process/environment paths are rejected.
- Optional previews are rejected until codec and keyframe validation exists. PNG validation currently checks the magic bytes only.

The source scanner is an early policy filter, **not a security boundary**. Rust macros and compiler behavior require an OS-isolated build process, followed by a Wasm capability sandbox. Linux builds now invoke the verified single-file compiler inside bubblewrap; game code executes only through the Wasmtime runtime when Play/run is requested. Do not relax the source profile before those boundaries exist.

## Roadmap

1. **Implemented:** archive reader, manifest, signature/trust verification, source policy, pack/verify CLI, malicious-input tests.
2. OS-isolated offline Cargo wrapper with pinned compiler and approved dependency closures.
3. Wasmtime fuel, memory and epoch limits, minimal WIT host API, asset/input/save access.
4. Pinned Slang compilation and shader reflection validation.
5. Install/list/update CLI and local package registry implemented; isolated single-file Wasm build implemented; grid-v1 game run implemented.
6. Broader bypass fixtures and preview decoding checks.
7. Feature-gated GUI verification, installed library, URL download and installation implemented; grid-v1 playback implemented; broader runtime APIs remain pending.
8. Shared asset pool and store tooling.

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

The executables are `target/release/gamepkg` and `target/release/gamepkg-gui` (with `.exe` on Windows). Launch the GUI with `cargo run --locked --features gui --bin gamepkg-gui`. It lists installed packages and accepts a local package path or HTTPS URL. Inspection runs in a background worker and displays the verified creator fingerprint and permissions before installation. New creators require approval; updates use the installed creator key automatically. It can compile the supported single-file profile on Linux; game grid-v1 playback implemented; broader runtime APIs remain pending. A desktop display is required to launch it.

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

`gamepkg build <id>` uses the same build action as the GUI. This initial build profile compiles the verified, dependency-free `source/src/lib.rs` directly to `wasm32-unknown-unknown` under Linux bubblewrap, without Cargo build scripts. It requires `bwrap`, `timeout`, Rust, `rustup target add wasm32-unknown-unknown`, and the pinned Slang installation described below. The compiler has a private working directory, an isolated network namespace, access to system tools and its toolchain, and a 60-second deadline. It has no access to the user's home directory. Failed sandbox setup stops the build; there is no native or unsandboxed fallback. Compiler memory caps, approved dependency support and broader GPU rendering remain pending.

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
runtime fuel. The guest receives only the requested bytes and cannot open host
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
folder and backend in the desktop Settings page. The default backend is
SPIR-V; WGSL and Metal source outputs are also tested. DXIL requires a suitable
downstream DXC installation and reports an error if unavailable. No backend
fallback occurs. Select GLSL and rebuild to use resource-free fragment passes in
the OpenGL desktop player. SPIR-V, WGSL, Metal and DXIL rendering integration remains pending.

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
pass using the signed manifest's fragment entry at that index. Select GLSL in
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
