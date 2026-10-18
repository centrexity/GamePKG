# Build a transparent GamePKG game

Games use host-supplied `gamepkg::` functions and have no Cargo dependencies. The host injects these bindings after checking every packaged source file.

Start with `examples/minimal` or `examples/good/input-game` in the source repository. GamePKG is the format and player: creators choose where to host their own packages. Players inspect your source and permissions before approving your key.

## Creator checklist

1. Write `source/src/lib.rs` with `pub fn game_tick(input: u32, state: u64) -> u64`. Use direct function calls and checked source modules. The host adds the runtime bindings and export wrapper; do not add your own imports or export attributes.
2. Set a unique game ID, semantic version, runtime profile, and source lockfile hash in `manifest.toml`. Declare only the permissions you need, recommended memory, and hard memory limits. Players choose execution deadlines in Settings. Use `network = false` for offline games or `network = true` to enable internet and local-network access. No endpoint allowlist is required.
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

Use `gamepkg::input_events()` for ordered pointer, physical keyboard and committed
text events. It returns `None` when the signed `input` permission is denied and
`Some(Vec<InputEvent>)` otherwise. The simple `game_tick` action-button mask
continues to work for keyboard, controller and touch actions. This API requires
the updated source host.

`gamepkg::window_control(operation, value)` requests changes to the game's own
window and requires signed `input` permission. Operations 0 and 1 accept 0/1 to
set cursor visibility and absolute cursor confinement respectively. Operation 2
with value 0 toggles actual fullscreen; operation 3 with value 0 closes the game.
Fullscreen and close require a fresh key, pointer, controller or touch action;
held buttons and repeated keys do not qualify. The return value means the host
accepted the request, not that the operating system fulfilled it. Requests commit
only after a successful tick and persistence; failed ticks roll them back.

`gamepkg::window_fullscreen()` returns `Some(true)` or `Some(false)` for the host's latest observation of this game's own window, or `None` when no observation is available, input permission is denied, or the game has closed. Reads need no fresh input gesture and remain stable during a tick. Use this value for fullscreen captions; accepting a toggle request does not establish that fullscreen changed. Keep the last confirmed state while observation is unavailable. The native window controller keeps a pending toggle until the current mode is known; unavailable does not imply windowed. Native/core/component and browser bindings expose the same result without native handles or access to other windows. Browsers observe their game canvas's DOM fullscreen state, including external exits.

Focus loss, file dialogs and Ctrl+Alt+Escape release the pointer. Click the game
to resume capture after emergency release. Desktop absolute confinement preserves
pointer position. Browsers use pointer lock with a bounded virtual canvas position
and may require the displayed Capture pointer or Enter fullscreen button because
of browser gesture restrictions. Escape releases browser capture and reaches the
game once. Close cancels pending file selection. These APIs require the updated
source host and are unavailable in older release binaries.

Each `InputEvent` has `kind: u32`, `x: f32`, `y: f32`, and `code: u32`:

| Kind | Meaning | Coordinates and code |
| --- | --- | --- |
| 0 | Focus/overflow reset | Release held keys and buttons; zero fields |
| 1 | Pointer moved | Position in the 640×480 game canvas; code is held buttons |
| 2 / 3 | Pointer button down / up | Canvas position; code identifies one button |
| 4 | Wheel | Signed deltas; code 0 = lines, 1 = pixels, 2 = pages |
| 5 / 6 | Physical key down / up | Zero coordinates; SDL scancode 1–511; down bit 16 marks repeat |
| 7 | Committed text | Zero coordinates; code is one Unicode scalar, excluding NUL |
| 8 | Pointer left/cancelled | Release pointer buttons; zero fields |
| 9 / 10 | Logical key down / up | Zero coordinates; Unicode or SDL named-key code; down bit 31 marks repeat |

Pointer button bits are left 1, right 2, middle 4, back 8 and forward 16.
Logical keys preserve the keyboard layout, with ASCII shortcut letters folded to
lowercase. Named keys use `0x40000000 | scancode`, except Space 32, Enter 13,
Escape 27, Tab 9, Backspace 8 and Delete 127. Committed text preserves case.
Logical keys and committed text follow physical keys in event order. Preedit text and typing
outside the focused game surface are excluded. Unknown browser physical keys
are omitted. Initial OS lock/modifier state, native IME activation and full native touch equivalence
still need further work.

At most 2,048 events (32 KiB) are queued. Overflow resets held input. Reads
consume queued events, and unread events expire at the next tick. Coordinates,
Unicode, button codes and wire lengths are validated by the host. Core,
component, native and browser bindings use the same fixed-width little-endian
records internally; game source uses the typed helper and never device handles.
No new filesystem, network, window-creation or Cargo dependency access is granted.

Use `gamepkg::draw_clear`, `draw_rect`, `draw_circle` and `draw_asset_image` for the host-owned viewport. Games cannot create windows or obtain GPU handles. `gamepkg::network_request(url, output)` performs an asynchronous, bounded HTTPS GET only when the signed manifest enables networking. TCP and UDP use the same host-controlled network service. See [networking](https://github.com/centrexity/GamePKG/blob/main/docs/NETWORKING.md).

`gamepkg::draw_asset_image_linear(path, x, y, width, height)` draws a package PNG with linear filtering when smooth scaling matches the original art. `draw_asset_image` uses nearest-neighbor filtering. Paths are relative to virtual `assets/`, for example `"heart.png"`, never an OS path. Both helpers share all texture, command and per-frame image limits; repeated cached draws still count fully. Native/Wasm retain up to 32 immutable decoded asset images within a shared 1 MiB pixel budget per instance, with least-recently-used eviction and paths capped at 1,024 bytes. Every draw still spends its full frame budget. The linear helper requires the current source-built host; released 0.1.17 binaries do not provide it.

Native linear sprite sampling interpolates premultiplied encoded RGB bytes using host-owned textures, separately from UI rendering. Browser linear sprites use a host-owned WebGL 2 sampler with the same single-level bilinear filter and encoded-byte premultiplication. Browser Canvas2D minification is not used for these sprites because it can change downscaled artwork. A browser lacking WebGL 2 reports failure explicitly. `gamepkg::draw_asset_image_linear_straight(path, x, y, width, height)` instead interpolates straight RGB and alpha before multiplying sampled RGB by sampled alpha, as needed by engines using straight-alpha textures. It shares the same path checks, immutable cache, source bounds and full per-draw budget charges. Browser export preserves nonopaque PNG RGBA bytes, including hidden RGB at zero alpha, in bounded precise-image metadata; those decoded bytes count against the existing 32 MiB export limit. These helpers require the current source-built host. Filtered sprite coverage follows SDL pixel-center edges while UV coordinates preserve the original sampling rectangle.

`gamepkg::frame_time_us()` returns relative monotonic microseconds sampled once at the start of each tick. The first tick is zero. Every read during that tick returns the same value; each game/session starts a separate clock. It exposes no calendar time and cannot be used to wait within a tick. Time and memory limits still apply. Native, core/component Wasm and browser players support it in the current source-built host.

Each tick supplies a fresh frame. With no `draw_clear` call, the viewport uses opaque dark RGB `#10171d`. A clear color with alpha below 255 is composited against opaque black for that frame, including a fully transparent clear. Redraw every visible primitive each tick; earlier frames do not supply retained pixels. Native and browser players use the same rule.

`gamepkg::draw_clip(x, y, width, height)` replaces the current drawing clip within the 640×480 logical canvas. Coordinates and dimensions must be finite and nonnegative, and the rectangle must fit the canvas; an empty rectangle draws nothing. `draw_clip_reset()` restores the full canvas. Both return whether the command was accepted and count toward the 1,024-command frame limit. Clips apply to shapes, images and shaders, reset each tick, and do not reduce image-transfer charges. `draw_clear` discards preceding drawing and clipping commands without refunding transferred image bytes. These APIs require the current source-built host.

`gamepkg::draw_image_linear(x, y, width, height, pixel_width, pixel_height, pixels)` draws generated RGBA pixels with linear filtering. It uses the same 512×512 source-image bounds, guest-memory validation, per-frame command limit and cumulative 8 MiB image-transfer budget as `draw_image`. Both return whether the command was accepted. Native, core/component Wasm and browser players support it in the current source-built host.

Game rectangles use measured SDL/OpenGL pixel-center coverage: the left and lower edge are included at exact pixel-center ties. Native rendering accounts for the viewport origin and physical DPI before snapping coverage; the browser snaps rectangle coverage before filling, avoiding Canvas edge antialiasing. This applies only to game drawing, independently of app UI styling. Empty rectangles draw nothing and still retain their normal command charge.

Browser nearest images containing one texel use pixel-center coverage with the lower-neighbor boundary tie rule. Thin pixel strips retain their source color.

Game rectangle/circle colors and sprite pixels use straight RGBA bytes. Native game drawing premultiplies encoded RGB bytes for source-over compositing, with a conversion separate from the app's UI styling. The conversion retains supplied opaque-color bytes. Maintainers can run `tests/blending_gui.py` to compare rectangles, nearest sprites and ordered overlaps with SDL in native and browser players.

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

The raw mask constants `BUTTON_A/B/X/Y/L/R/START/SELECT` and the corresponding
`HELD_*` constants, including `HELD_LEFT/RIGHT/UP/DOWN`, are also shared by native,
core and component source builds. Use the named constants instead of hard-coded
bit positions. Older hosts may need rebuilding for the complete component set.

Keyboard press bits ignore OS key repeat. If two keyboard bindings share an
action (such as Left and A), it stays held until both are released and pressing
the second binding does not create another edge. A quick press/release within
one frame still supplies a press bit. Browser exports use the same behavior and
held-bit layout as the desktop app; losing focus clears pending keyboard input.

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

See [native runtime and build rules](https://github.com/centrexity/GamePKG/blob/main/docs/NATIVE_RUNTIME.md) and [network permissions and API](https://github.com/centrexity/GamePKG/blob/main/docs/NETWORKING.md). HTTPS GET, TCP and UDP use asynchronous host calls controlled by the signed `network = true`/`false` permission. Enabled games can use internet and local-network addresses without endpoint lists. Legacy `network = "none"` remains disabled; earlier enabled transport names now enable all supported networking.

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

A host-approved native selector can sign `game_data.user_selected_servers = true` to expose remembered join, hosting and master launch preferences in the library or `configure-network`. These preferences do not restrict an enabled network permission. Blank fields clear launch commands; `network = false` disables access regardless of those fields. Local/private addresses need no separate opt-in. Ordinary guest source cannot change these host-only launch preferences.

## Player-selected import and export

Declare each file purpose in the signed manifest. This is optional and default
denied. It also requires `permissions.input = true`:

```toml
[[permissions.files]]
id = "patterns"
title = "Level pattern text"
extensions = ["txt"]
max_bytes = 1048576
read = true
write = true
```

Call `gamepkg::file_import(0)` or `file_export(0, "pattern.txt", bytes)` during a
tick that receives a fresh focused keypress or pointer-button press. The index
addresses the signed list above. Each action permits at most one request; held
key repeats and later ticks do not preserve that authorization. Requests become
visible to the host only after the tick succeeds. The desktop app opens its
system Open/Save dialog asynchronously. Browser exports show explicit upload or
save/download controls, requiring another player action. The CLI does not open
dialogs. Games never receive host paths, directory access or reusable OS handles.
This API needs the updated source host; older release binaries may not expose it.

The request returns `Some(handle)` or `None` for unavailable, denied or invalid
requests. Poll `file_status(handle)` each tick:

| Status | Meaning |
| --- | --- |
| 1 | Waiting for the player or host |
| 2 | Imported file ready; `size` and `name_bytes` are available |
| 3 | Export completed or handed to the browser download manager |
| 4 | Player cancelled |
| 5 | Request failed |

`file_name(handle, output)` copies only the selected basename as UTF-8.
`file_read(handle, offset, output)` reads at most 64 KiB from the selected
read-only snapshot. Loop over chunks for larger inputs. Reads do not consume
file bytes. `file_close(handle)` releases a completed request; pending requests
cannot be closed and replaced to flood the player with dialogs. A request and
its result belong to one running game instance. Stale replies cannot be applied
to a replacement game. Cancellation and errors must preserve the working game data.

There are at most 32 purposes, each with a unique ASCII ID, a short title and
1–8 lowercase alphanumeric extensions. Imports are capped at 64 MiB, or the
smaller declared limit. Export-enabled purposes are capped at 1 MiB, including
both directions when `read` and `write` share a purpose. Only one request/result
is retained at a time. Names must be portable basenames, without traversal,
streams, control characters or Windows device names. Native selection rejects
symlinks and non-regular files; exports atomically replace only the explicitly
chosen destination. The filename supplied by the guest is a suggestion, never
an automatic output path. No microphone, device, process or network access is
added. The guest is responsible for validating its actual file format.

These operations do not yet provide a directory/resource browser, remembered
mod folders, streaming media resources or large recording exports. Those remain
separate requirements for the engine ports.
