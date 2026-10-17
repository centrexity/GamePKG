# Build a transparent GamePKG game

Games use host-supplied `gamepkg::` functions and have no Cargo dependencies. The host injects these bindings after checking every packaged source file.

Start with `examples/minimal` or `examples/good/input-game` in the source repository. GamePKG is the format and player: creators choose where to host their own packages. Players inspect your source and permissions before approving your key.

## Creator checklist

1. Write `source/src/lib.rs` with `pub fn game_tick(input: u32, state: u64) -> u64`. Use direct function calls and checked source modules. The host adds the runtime bindings and export wrapper; do not add your own imports or export attributes.
2. Set a unique game ID, semantic version, runtime profile, and source lockfile hash in `manifest.toml`. Declare only the permissions you need, recommended memory, and hard memory limits. Players choose execution deadlines in Settings. Leave networking as `none` unless you declare the exact HTTPS/multiplayer endpoints needed by your game.
3. Put assets under `assets/`, Slang source under `shaders/`, and include a thumbnail, license and credits. Paths must be portable relative paths with forward slashes, no traversal, device names or symlinks.
4. Declare no Cargo dependencies. Include a root-only `source/Cargo.lock` and put its SHA-256 in `lockfile_sha256`. Do not vendor dependency replacements into your package.
5. If your game requires ROM files, declare each requirement in the manifest. Players select their own files; read only those ROMs through `gamepkg::rom_count`, `rom_size_at` and `rom_read_at`. Do not fetch ROMs or ask for access to their folders.
6. Use `gamepkg` host functions for assets, saves, drawing, audio and achievements. Keep numbers portable: fixed-width serialized values and qualified conversions such as `u32::to_le_bytes(value)`. `usize` is for internal indexing, not stored data.
7. Pack, verify, install, build and play locally before publishing. Test missing assets, absent ROMs, denied permissions, corrupt saves, resource limits and both new and held input. Test both `core-v1` and `component-v1` if you claim both profiles.
8. Keep your private signing key outside the package and backed up. Publish your signed package at your chosen URL. An optional `update_url` points to the next signed package; updates retain your game ID and creator key and increase the version. Players need no private key or Python.

## No guest crates

Cargo dependencies are forbidden, including `gamepkg-*`, renamed, optional, Git, path, build, development and target-specific dependencies. Workspace, patch, replace and feature sections are rejected. The lockfile must contain exactly the root package name and version, with no dependencies, source or checksum. An empty `[dependencies]` table is optional. Copy the Cargo files from `examples/minimal` and change both root identities together.

The player has no dependency approval setting or command. Legacy approval settings are ignored and removed on save. This policy applies during packing, verification, installation and building, even to correctly signed packages. Host dependencies are reviewed separately as part of GamePKG itself.

### Minimal Cargo files

`source/Cargo.toml`:

```toml
[package]
name = "my-game"
version = "0.1.0"
edition = "2021"
build = false

[lib]
path = "src/lib.rs"
```

`source/Cargo.lock`:

```toml
version = 4

[[package]]
name = "my-game"
version = "0.1.0"
```

Keep the name and version identical in both Cargo files. Hash the exact lockfile bytes for the manifest's `lockfile_sha256`. Do not add dependencies or `extern crate` declarations. Call `gamepkg::` directly: it is a host-injected module, not a crate to install, download or list in Cargo.toml. Rust's built-in standard library remains subject to the source restrictions below; it does not grant filesystem or network access.

## Scoped host functions

Use `gamepkg::asset_read` for package-relative assets, `rom_count`, `rom_size_at` and `rom_read_at` for player-selected ROMs, and `save_size`, `save_read` and `save_write` for the game's save blob. They never open arbitrary host files.

Use `gamepkg::random_u64()` for a fresh 64-bit random value supplied by the host.
It needs no additional permission and exposes no device, clock, filesystem or
host identity. Native, core-Wasm and component-Wasm hosts use OS randomness;
browser exports use `crypto.getRandomValues`. If entropy is unavailable the
call fails rather than returning a fixed seed. Calls remain subject to normal
tick deadlines and native RPC limits. For a uniform choice among `n` outcomes,
use rejection sampling before taking a remainder; `% n` alone can introduce bias.
This API requires the updated source host; older release binaries may not expose it.

Use `gamepkg::draw_clear`, `draw_rect`, `draw_circle` and `draw_asset_image` for the host-owned viewport. Games cannot create windows or obtain GPU handles. `gamepkg::network_request(url, output)` performs an asynchronous, bounded HTTPS GET only when the signed manifest permits that exact endpoint. TCP and UDP use the same host-controlled network service. See [networking](https://github.com/centrexity/GamePKG/blob/main/docs/NETWORKING.md).

These functions preserve the source checks, signatures, isolated compiler and Wasm restrictions. Games cannot replace the injected implementation or declare their own host imports.

## Slang shaders, textures and audio

Use the injected shader and audio APIs directly. `examples/good/media-game` demonstrates them without dependencies. Declare each Slang source, entry and stage in `[[shaders]]`. The host verifies the signed source, compiles it in isolation on Build/Rebuild, validates resource reflection, caches the result, and selects the configured playback backend. Games never run a compiler themselves or supply precompiled shader binaries.

```rust
if let Some(shader) = gamepkg::Shader::from_slang("shaders/texture.slang", "blueMain") {
    shader.draw_texture("texture.png");
}
gamepkg::audio_submit(&[100, -100]);
```

`Shader::from_slang(source, entry)` resolves only signed fragment entries; unknown source/entry pairs return `None`. `draw()` draws a resource-free pass; `draw_rgba(width, height, pixels)` supplies a private RGBA snapshot; `draw_texture(path)` loads and decodes a PNG from the package's virtual `assets/` folder. Paths never grant host filesystem access. PNG inputs are capped at 2 MiB, decoded images at 512×512 pixels, decoder allocations at 8 MiB, and queued image data at 8 MiB per frame. The host owns the sampler and texture. Extra shader resources, includes and module imports are rejected. Raw GPU handles are never exposed.

For texture sampling, use the Slang declaration and separate-sampler branch in `examples/good/media-game/shaders/texture.slang`. GLSL plays directly; supported SPIR-V/WGSL passes translate to the host renderer. Desktop playback supports fragment passes, not arbitrary compute dispatch. Wasm deadlines do not interrupt GPU work; compiler restrictions and bounded renderer resources are separate protections.

Set `[permissions] audio = true` for output. `gamepkg::audio_submit(samples)` accepts mono signed 16-bit samples at 48,000 Hz; `gamepkg::audio_submit_pcm16_le(bytes)` decodes explicit little-endian PCM. The runtime caps the total at 1,600 samples per tick, including multiple submissions. Odd byte counts, oversized buffers and denied permissions return `false`. Audio is discarded if the tick traps. The host handles the output device, volume and resampling; games receive no microphone or device access. These are PCM APIs, not compressed-audio or WAV container decoders.

## Execution limits

There is no instruction metering or instruction-budget setting. The runtime interrupts Wasm ticks and start functions using host time deadlines, enforces the lower of the signed memory cap and player ceiling, and bounds draw commands, images, audio and saves. A stopped tick discards pending drawing/audio/save changes. Old instruction-budget fields are accepted only for reading existing packages/settings; they are ignored and omitted when writing new data.

### Migrate a dependency-based package

Remove all Cargo dependencies and reduce the lockfile to the root package. Replace former wrapper calls with `gamepkg::` functions. Use qualified Rust conversions such as `u32::from_be_bytes` and `u32::to_le_bytes` for explicit-endian values. Recalculate the lockfile hash, increase the game version, and sign with the same creator key. Older dependency-bearing packages are rejected; legacy local approvals cannot exempt them.

## Controller and touch input

Games consume one `u32` input value per tick. The same buttons work with the keyboard, a controller, or the player's touch regions. Set `[permissions] input = true`; otherwise the runtime passes zero.

```rust
pub fn game_tick(input: u32, state: u64) -> u64 {
    let mut next = state;
    if gamepkg::held(input, gamepkg::RIGHT) {
        next = next.wrapping_add(1);
    }
    if gamepkg::pressed(input, gamepkg::B) {
        next = 0;
    }
    next
}
```

`pressed` detects newly pressed buttons; `held` detects continuing presses. Constants are `LEFT`, `RIGHT`, `UP`, `DOWN`, `A`, `B`, `X`, `Y`, `L`, `R`, `START`, and `SELECT`. These are digital controls; this API does not expose raw touch coordinates, pressure, controller identities, or analog values to games.

The desktop host uses pinned `gilrs 0.10.10` for Linux/Windows controller input and the existing `egui 0.31.1` input layer for touch. These are host dependencies, not allowed guest dependencies: games cannot open devices or windows. D-pad and the left stick map to directions with a deadzone; face buttons map by position (south=B, west=Y, east=A, north=X). Shoulder buttons, Start and Select map to their names. Multiple connected controllers contribute to the same input. The game window contains only the game. Invisible touch regions occupy the bottom third of its client area, in twelve equal columns from left to right: Left, Right, Up, Down, B, Y, A, X, L, R, Start, Select. Movement and action buttons can be held simultaneously. Losing window focus clears device input. Android controller device access and an Android GUI are not supplied by this desktop integration.

## Source restrictions

Do not use aliases, renamed/wildcard imports, guest macros, closures, function pointers, dynamic dispatch, indirect calls, unsafe code, foreign declarations, binary includes, custom source paths, or direct filesystem/network/process/environment access. Do not use native-endian encoding, raw pointers, memory layout inspection, architecture-specific intrinsics or shadow primitive type names. Guest code does not receive WASI imports. All packaged Rust files are scanned, even unused files.

Source checks make code easier to inspect; isolated compilation and bounded Wasm execution enforce additional boundaries. A signature proves the contents came from a key, not that the code is harmless.

## Build and test

Install the host and compiler tools using the repository installation guide. Keep the signing key and output package outside the source folder.

```sh
gamepkg keygen -o /path/to/private/creator.key
gamepkg pack /path/to/game -o /path/to/output/game.gamepkg --key /path/to/private/creator.key
gamepkg verify /path/to/output/game.gamepkg
gamepkg install /path/to/output/game.gamepkg --trust-creator
gamepkg build com.example.game
gamepkg run com.example.game --frames 60
gamepkg-gui
```

Replace the ID with your manifest ID. For the input example use `com.gamepkg.input`. The desktop player supplies controller and touch controls; the CLI is useful for deterministic frame checks. Linux source builds use bubblewrap; Windows source builds use the configured Linux compiler connection through WSL 2. Optimization settings change local builds without expanding game permissions.

## Native builds, Vulkan and networking

The default Linux Build compiles directly for the local CPU in an isolated native worker. Build Wasm is an explicit alternative. The same source and no-dependency rules apply to both. Direct standard I/O and thread APIs are also rejected; use the host functions. Vulkan is the default desktop renderer, with WGSL or SPIR-V output compiled from Slang. Guests receive no raw window, Vulkan or socket handles.

See [native runtime and build rules](https://github.com/centrexity/GamePKG/blob/main/docs/NATIVE_RUNTIME.md) and [network permissions and API](https://github.com/centrexity/GamePKG/blob/main/docs/NETWORKING.md). HTTPS GET, TCP and UDP use declared signed endpoints and asynchronous host calls. Existing packages declaring `network = "none"` retain no network access.

## Describing a game

GamePKG 0.1.13 adds optional signed display metadata at the top level of `manifest.toml`, before any `[limits]` or other table:

```toml
title = "Puzzle Garden"
description = "A relaxed puzzle adventure. Match tiles to explore the garden."
author = "Example Studio"
homepage = "https://example.com/puzzle-garden"
genres = ["Puzzle", "Adventure"]
engine_version = ">=0.1.13, <0.2.0"
```

Title (256 bytes), description (4096 bytes, including optional newlines), author (128 bytes), homepage (2048 bytes), and up to eight genres (32 bytes each) are optional. Text is rendered literally, without HTML or Markdown. Control sequences and bidirectional formatting controls are rejected; descriptions can contain line breaks. Homepages use the same HTTPS/Tor URL rules as package links. Genre labels must be unique, ignoring case. Author is a self-reported display name; creator identity remains the signing key.

Existing packages need none of these fields and keep their IDs as fallback titles. Metadata changes require a newly signed package and a higher game version. GamePKG 0.1.13 still reads older packages; update older hosts before importing packages or lists that use the new fields.

A `.gamepkglist` entry can also include `description = "..."` with the same text limits. Catalog titles and descriptions are unverified preview text; after installation the signed manifest supplies the displayed information. Importing descriptions downloads no games.

## Reviewed folder-backed engines

Only exact host-approved built-in engine profiles may request a `[game_data]` folder. The user selects the original installation through the system folder picker. The host remembers it for the signed game and creator, pins directory handles at launch and mounts only the declared data directories read-only. These packages must use their reviewed launcher template; extra Rust modules, C/C++ source, build scripts and dependencies are rejected. See the engine's repository documentation for its profile, source provenance, required tools and resource limits. Ordinary guest memory and image limits remain unchanged.

A host-approved native selector can explicitly sign `game_data.user_selected_servers = true` together with `network = "multiplayer"`, empty endpoint lists and no static local-network grant. The host must support that exact reviewed profile. This lets the user choose an exact join or hosting address, or explicitly delegate discovery to a chosen master, in the library or with `configure-network`; blank choices grant no networking. The host validates and pins the chosen game/server-info endpoints and requires a separate user opt-in for local/private addresses. Ordinary guest source cannot use this declaration or change these host-only choices.
