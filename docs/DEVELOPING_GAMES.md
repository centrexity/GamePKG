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

Reviewed engine profiles are host features, not a way to approve game crates.
For example, the Takedawn profile accepts only its exact host-pinned source kit
and launcher; additional package source, build scripts and replacement assets
are rejected. Its prepared Rust game library has no third-party Cargo
dependencies. The original native runtime and its vendored host dependencies
belong to that reviewed kit. A game author cannot opt arbitrary code into this
exception by copying the profile name or supplying a digest in the manifest.
See [native engine profiles](NATIVE_RUNTIME.md#reviewed-native-engine-profiles)
for the separate build/runtime requirements and platform qualification.

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

For large saves, current source-built hosts also provide
`gamepkg::save_chunk(offset, total, bytes)`. Start at offset zero, send consecutive
chunks of at most 65,536 bytes and finish the declared total within the same
tick. The total cannot exceed 1,048,576 bytes. The host keeps the old save until
the complete replacement arrives; partial sequences expire before the next
tick. Completed writes retain normal successful-tick persistence and rollback.
This lets a game serialize through a small reusable buffer. Browser chunked and
whole-blob writes share the existing 1 MiB per-tick transfer budget. This API
requires rebuilding the host from current source.

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

`gamepkg::window_size()` returns `Option<[u32; 2]>` containing the actual physical
client-area width and height of the game's own desktop window. It requires signed
`input` permission and remains stable between host observations. Unavailable,
minimized, headless, embedded-library and browser contexts return `None`; denied
input and closed games also return `None`. This is measured state, including
external resizing and fullscreen changes, rather than a predicted preset. It
does not resize the window. Dimensions outside the 16-bit wire representation
are unavailable, not clamped. Generated native/core/component bindings require
the updated source-built host; released 0.1.17 hosts lack this call.

Current source-built desktop hosts also provide:

- `window_vsync_preference()` reads the committed `Option<bool>` presentation
  preference for the next game-window launch. It requires signed input and a
  desktop host with readable persistent game-window settings. Other contexts
  return `None`. Reads have no side effects or fresh-action requirement and do
  not predict a change staged during the current tick.
- `window_set_vsync(enabled)` requests the next-launch preference with a fresh
  user action. A successful tick commits the request; a failed tick discards it.
  Restart the game to apply it. Preferences are scoped by game, creator and
  library, and default to enabled for older window records. OpenGL configures its
  swap interval; Vulkan selects strict FIFO when enabled and automatic non-VSync
  presentation when disabled.
  Drivers and compositors control final presentation; Vulkan can fall back to a
  supported mode. Browser and headless hosts offer no desktop VSync preference.
- `window_preview_active()` reads whether a permitted, committed host preview is
  outstanding. It does not predict a start, Keep, Revert or close staged during
  the current tick. The host's expiry is visible on the tick it occurs. Reads
  have no side effects, require no fresh action, and still obey execution
  deadlines. Browser/default headless runs have no active preview.
- `window_preview_size(width, height)` starts one pending physical-size preview.
  It requires signed input permission, a fresh action, an actual size observation
  and a known windowed state. Requests accept widths 320–10,000 and heights
  240–10,000 pixels; these request bounds do not change the renderer-limit setting.
  A request matching the observed size, or a second outstanding preview, returns
  `false`. The operating system may constrain the applied size; read
  `window_size()` for the actual result.
- `window_keep_size()` confirms a pending preview with a fresh user action.
- `window_revert_size()` restores only the size recorded by the host before that
  preview. It needs no fresh action, so a game's confirmation timer can cancel
  safely. It cannot supply a new arbitrary size.

These correspond to operations 5 (packed low/high-16-bit width/height), 7 (Keep,
value 0) and 6 (Revert, value 0) on `window_control`. Operation 8 with value 0 is
its read-only committed-preview query. Guest-requested changes still
require successful tick/checkpoint commit. The host additionally counts 300
subsequent attempted ticks and reverts on the following tick if the preview
remains unconfirmed; failed later ticks do not reset this host fallback. Closing
also cancels an outstanding preview. Unconfirmed geometry is excluded from saved
window preferences, and restored geometry waits for physical acknowledgement.
Browser, headless and embedded-library hosts reject previews. Normal and
maximized desktop preview flows are verified on Linux: cancel/timeout restores
maximization and the prior normal bounds, while Keep records the confirmed normal
geometry. Restoration waits for normal-size acknowledgement before maximizing.
For normal-origin previews, entering fullscreen preserves the actual fullscreen
flag across expiry/close/reopen. Keep in fullscreen saves the last windowed
geometry and mode, rather than monitor dimensions. An expired/reverted normal
size waits for fullscreen exit before being applied. Fullscreen pixels cannot
acknowledge a normal-size restore. A fullscreen entry queued with a preview waits
for its geometry; fullscreen exit remains available during deferred restoration.
Linux native/core/component checks cover these flows at 100%/150% UI scaling.
More complex maximized/fullscreen combinations, monitor-clamp/hotplug edges,
native-DPI and other-platform behavior remain under qualification. The original
Desktop game Settings menus can use these APIs for resolution choices and
Keep/Revert confirmation. The host remains authoritative
for expiry and actual geometry. Browser/headless hosts offer no desktop size
selection; fullscreen users return to windowed mode before choosing a size.

`gamepkg::window_control(operation, value)` requests changes to the game's own
window and requires signed `input` permission. Operations 0 and 1 accept 0/1 to
set cursor visibility and absolute cursor confinement respectively. Operation 2
with value 0 toggles actual fullscreen; operation 3 with value 0 closes the game.
Operation 4 with value 0 requests opening the signed manifest's `homepage` in
the user's browser. No URL or process arguments are accepted from the game. It
requires a declared HTTPS homepage (or a valid v3 onion HTTP/HTTPS URL), a fresh
action and signed input permission; at most one homepage request is accepted per
tick. Desktop game windows and browser exports consume committed requests once.
Browser tabs use `noopener,noreferrer` and require active browser user activation;
unfocused or file-dialog-blocked requests are discarded. Headless runs do not
open a browser. Older released hosts reject this new operation; rebuild from
current source to use it.
Operation 9 reads the committed desktop VSync preference: value 0 tests
availability and value 1 reads enabled. Operation 10 requests disabled/enabled
with value 0/1 and a fresh user action. These operations back the typed VSync
helpers above and are unavailable to browser/headless hosts.
Fullscreen, close and homepage requests require a fresh key, pointer, controller or touch action;
held buttons and repeated keys do not qualify. The return value means the host
accepted the request, not that the operating system fulfilled it. Requests commit
only after a successful tick and persistence. Browser workers stage window
requests and new file-picker jobs until the player acknowledges that frame's
save/history writes. A denied acknowledgement rolls those requests back; they
are not replayed after storage recovers. Frame serials and restart generations
prevent late acknowledgements from committing another frame. A fresh fullscreen,
close or homepage action is needed after a rejected request. This is request
rollback, not atomic multi-record storage or rollback of guest memory.

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

## Recorded effects on desktop

`examples/good/recorded-effects` plays a generated chime when Start/Enter is
pressed. It uses no guest dependencies. Select this mode with `audio = true`
under `[permissions]` and `effects_memory_bytes = 1048576` under `[limits]`.
The effects share is 64 KiB to 4 MiB and comes out of the existing hard memory
limit and the player's memory ceiling. For example, reserving 1 MiB under a
16 MiB effective hard limit leaves 15 MiB for game memory. Include room for
cache metadata, output conversion and temporary loading as well as recordings.

- `gamepkg::effect_play("sounds/bounce.wav")` stages a recorded sound.
- `gamepkg::effect_volume(64)` sets effect gain from 0 to 128.
- `gamepkg::effect_pause(true)` freezes voice positions; `false` resumes them.
- `gamepkg::effect_stop()` stops all current effect voices.

Assets must be canonical little-endian 44.1 kHz stereo S16 WAV, with a standard
44-byte header, at most 2 MiB of PCM per recording. Stored package entries and
resolved shared assets are supported. Compressed entries fail explicitly until
their decompressor workspace is included in the shared accounting. The host
checks verified scoped paths and the complete entry CRC before caching PCM.

The host uses four first-free voices, mixes with sequential integer saturation,
and converts the combined stream once to its 48 kHz output. Voices advance on
the audio device clock independently of game ticks. A fifth simultaneous voice
is rejected and reported by the output backend. Muting gain still advances
voices. Cache metadata is fixed at 32 recordings, and a tick can stage at most
32 controls. Full caches reject new names without evicting active recordings.

Calls return whether their arguments and staged request were accepted; final
device queue or voice exhaustion can still fail later. A failed tick, save or
history write discards its staged commands. A rejected output batch is discarded
as a whole. The mode rejects `audio_submit`/`audio_submit_stereo` to avoid mixing
two incompatible playback paths. Omitting the effects share keeps the existing
PCM API and disables these recorded-effect calls. Headless embedding must attach
a trusted output publisher; desktop playback does this automatically.

Source-built hosts support desktop native/core/component playback and core/component browser exports when the audio companions are embedded. Browser controls are prepared before the save/history checkpoint and applied only after success. Failed checkpoints discard new cache entries and commands without replay. Players need no Python, external decoder, additional game crates, microphone access or file paths.

## Recorded music

Declare `audio = true` and `music_memory_bytes = 7340032` (7 MiB) to use the
bundled host MP3 decoder. The share comes out of the signed hard limit and the
player's memory ceiling. With a 16 MiB effective hard limit, 7 MiB for music and
1 MiB for effects leaves 8 MiB for the game. Include the decoder's declared
memory, host staging and output converter in the music share. Insufficient
memory fails explicitly; the host does not increase the game's limit.

- `gamepkg::music_play("music/theme.mp3")` requests a verified asset.
- `gamepkg::music_volume(64)` sets integer gain from 0 to 128.
- `gamepkg::music_pause(true)` pauses; `false` resumes.
- `gamepkg::music_stop()` stops the current track.
- `gamepkg::music_finished()` reports natural track completion.
- `gamepkg::music_failed()` reports asynchronous loading or decoding failure.

Control calls report staging acceptance. The track loads asynchronously after
execution, save and history success; poll completion/failure on later ticks.
Failed ticks discard controls and preserve prior music. Invalid complete music
batches reject their staged effects too; denied effect output also prevents the
music batch from publishing. Decoder faults after acceptance remain asynchronous
failures and can be followed by a new Play request. At most 32 music operations
are staged per tick. Only the final same-tick track replacement loads.

Music accepts stored package MP3 assets and uses verified scoped readers. ZIP
compressed music entries fail explicitly until decompressor workspace is
accounted. Music advances on the output clock independently of game ticks, mixes
at 44.1 kHz before the shared 48 kHz conversion, and can coexist with recorded
effects. Declaring either recorded-audio share disables premixed PCM submission.
No guest dependency, decoder file, Python installation or player file path is
required. Source-built hosts support desktop native/core/component and core/component browser exports with embedded companions. Browser music loads asynchronously after a successful checkpoint; Stop/replacement cancels older startup. Declared shares include decoder, cache, transfer and converter reservations and reduce the guest memory ceiling. Browser compiler/JIT/GC and caller-owned asset storage are separate from those explicit reservations. The canonical Arkanoo recipe now enables original recordings; wider platform and game fidelity verification remain open.

Browser decoder input uses sequential transfers of at most 64 KiB. The complete
upload has a fixed five-second allowance; each acknowledgement is limited to
100 ms. Codec open, read and rewind each retain their 100 ms execution limit.
This separates message scheduling for large assets from codec execution without
renewing the aggregate upload allowance or increasing audio memory reservations.

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

Set `[permissions] audio = true` for output. `gamepkg::audio_submit(samples)` accepts mono signed 16-bit samples at 48,000 Hz; `gamepkg::audio_submit_pcm16_le(bytes)` decodes explicit little-endian PCM. `gamepkg::audio_submit_stereo(samples)` accepts interleaved left/right signed 16-bit values at the same rate; its length must be even. The runtime caps the total at 1,600 sample values per tick (800 stereo frames), including multiple submissions. A tick may submit mono or stereo, but cannot mix nonempty formats; an empty submission does not select a format. Odd byte counts, oversized buffers and denied permissions return `false`. Audio is discarded if the tick traps. The host handles the output device, volume and resampling; games receive no microphone or device access. These are PCM APIs, not compressed-audio or WAV container decoders.

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

The desktop host uses pinned `gilrs 0.10.10` for Linux/Windows controller input and the existing `egui 0.31.1` input layer for touch. These are host dependencies, not allowed guest dependencies: games cannot open devices or windows. D-pad and the left stick map to directions with a deadzone; non-finite stick readings are ignored independently on each axis. Face buttons map by position (south=B, west=Y, east=A, north=X). Both the left bumper and lower trigger map to L; both right controls map to R. The action stays held until all corresponding controls are released. Start and Select map to their names. Multiple connected controllers contribute to the same input. The game window contains only the game. Invisible touch regions occupy the bottom third of its client area, in twelve equal columns from left to right: Left, Right, Up, Down, B, Y, A, X, L, R, Start, Select. Movement and action buttons can be held simultaneously. Losing window focus clears device input. Android controller device access and an Android GUI are not supplied by this desktop integration.

On startup or after observed focus loss, the desktop host drains pending controller
events before accepting fresh controller presses again. This work retains the
256-events-per-frame bound even for a backlog spanning multiple frames. Held
controller state can resume, but resynchronizing it does not generate a fresh
press or authorize a fresh-gesture window/file request. A new press after the
pending queue has drained works normally. Keyboard and touch use their separate
focused event paths.

## Source restrictions

Do not use aliases, renamed/wildcard imports, guest macros, closures, function pointers, dynamic dispatch, indirect calls, unsafe code, foreign declarations, binary includes, custom source paths, or direct filesystem/network/process/environment access. Do not use native-endian encoding, raw pointers, memory layout inspection, architecture-specific intrinsics or shadow primitive type names. Guest code does not receive WASI imports. All packaged Rust files are scanned, even unused files. A forbidden-construct error names the offending source file. Avoid using a function name for a local variable: function-item values are rejected by the same checks that prevent indirect calls.

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


Browser export may store larger canonical effect WAVs losslessly compressed.
Game source continues to read the original WAV bytes and call the same effect
functions. The exported player uses bounded reusable decoding buffers, reserved
inside the existing hard memory allowance; the exported guest memory maximum
is reduced to leave room for both owners. The exporter keeps raw storage when
compression would not save space or would leave too little guest memory. The
32 MiB encoded asset limit, effect cache limits and execution deadlines still
apply. Players need no external decoder, Python or other preparation tools.


## Reviewed DOOM recipes with patch WADs

For `builtin_engine = "doom-e257a095"`, declare the player's base IWAD first in `[[roms]]`, then up to six PWAD files in their intended loading order. The original engine receives them through `-file`; later files override earlier matching lumps. Every declared file is required and uses the normal per-game file picker and remembered location. The standard DOOM recipe continues to request only its base IWAD. Use a distinct game ID/save schema for a mod recipe when its saves are incompatible with the base game.

For example, add the following declarations to the existing reviewed recipe, after its base IWAD declaration:

```toml
[[roms]]
id = "patch_content"
title = "Your compatible patch WAD"

[[roms]]
id = "patch_override"
title = "Your compatible override WAD"
```

Once declared, both files are required by this recipe. Declare only the patches that recipe actually needs. These files remain player-selected resources; the manifest does not grant access to their containing folders. Native and Wasm builds use the same exact read-only virtual names and original loading order. Six patches plus the base IWAD leave one of the unchanged eight virtual handles for configuration/save operations. Combined WAD input stays within 64 MiB and the engine's existing hard memory limit still applies; that input bound does not guarantee that a large mod will fit all engine allocations.

The original engine's shareware restrictions and supported WAD semantics remain in force. This does not provide arbitrary native extensions, every source-port feature, multiplayer or browser export. Rebuild the game after updating GamePKG. Maintainers can run `python3 tools/check_doom_mods.py --wad /path/to/DOOM.WAD --baseline` to privately verify all six slots, duplicate-lump order, generated E1M1 map precedence, exact player starts, rendering and wall collision, original gameplay/movement, save/reload, malformed patch rejection and unchanged single-WAD behavior on native and Wasm. The tool creates synthetic title patches and keys in temporary storage and reads the original IWAD in place; players do not need Python.


### Signed WAD loading modes

A DOOM recipe may set top-level `doom_wad_modes` beside `builtin_engine`, before any TOML tables. If supplied, the array has exactly one entry per additional PWAD, excluding the base IWAD. Omitting it keeps every patch in `file` mode. Values are `file`, `merge`, `nwt_merge`, `flats`, `sprites` and `both`, selecting the original `-file`, `-merge`, `-nwtmerge`, `-af`, `-as` and `-aa` paths respectively. Other games cannot use this field, and arbitrary command-line arguments are rejected.

```toml
doom_wad_modes = ["merge", "file"]
```

The original `nwt_merge` operation masks matching IWAD sprite names, discards the patch directory and closes that patch handle. The original workflow then adds the PWAD again through `file`; follow the mod's required sprite-section layout. Its metadata masking/cleanup has separate sanitizer coverage. A generated `SS_START`/`PISGA0`/`S_END` fixture also passes mask-then-file loading, visible replacement, firing animation and exact idle return on native/Wasm. Select the same PWAD for the separate mask and reload requirements. Use `tools/check_doom_mods.py --wad /path/to/DOOM.WAD --nwt-probe` for that private check; broader real-mod layouts remain separate qualification work.

The original parser processes groups in this order: `merge`, `nwt_merge`, `flats`, `sprites`, `both`, then `file`. Declaration order is preserved within each group. Therefore an ordinary `file` patch loads after merged patches even if it is declared earlier. Use the merge modes when the original mod requires sprite/flat merging; this does not add source-port scripting or native extensions. Registration is sealed when the engine initializes, and the existing file-handle/input/memory boundaries still apply. Rebuild the engine and game after updating GamePKG.

Maintainers can add `--mode merge` to `tools/check_doom_mods.py` for the signed original merge path. Current native/Wasm qualification covers complete resource/map override order, exact starts, rendering/collision and original saving/reload for `file` and `merge`. Mixed `file`/`merge` recipes are also verified with the file declared both first and last; the original file-group priority wins in both arrangements. Maintainers can select `--mode mixed_file_first` or `--mode mixed_file_last`. Generated flat replacement is also verified for `merge`, `flats` and `both`; `sprites` and `nwt_merge` are verified to leave that flat content untouched. Use `--flat-probe --mode flats` (or another merge-family mode) for this separate probe. Generated pistol-sprite replacement is verified for `merge`, `sprites` and `both`, with `flats` verified to leave it unchanged; use `--sprite-probe --mode sprites` for this separate test. Directional/world sprites and more complex NWT/content cases remain tracked in `docs/port-parity.toml`.


For a flat name absent from the base IWAD, use `merge` to insert it into the original flat directory. The original `flats`/`both` algorithms replace existing entries; they do not insert new names. Generated new-flat probes verify that distinction on native/Wasm and require missing-resource errors to name the absent flat. Run `tools/check_doom_mods.py --wad /path/to/DOOM.WAD --new-flat-probe --mode merge` or a replacement-only mode for the corresponding positive/negative case. Engine failures now preserve bounded diagnostics: DOOM retains the newest 4,095 diagnostic bytes plus a null terminator in its existing 4 KiB buffer and reads it after a Wasm trap without further game execution, and native games retain the newest 4 KiB of stderr for startup/tick failures.


The private `--recovery-probe --mode flats` check (also available for `both`, `sprites` and `nwt_merge`) creates an original save, deliberately loads an incompatible generated map, verifies the named missing resource and byte-identical committed save files, then reopens the valid map. Recovery must restore exact coordinates, preserve the original saved slot, be unpaused and accept original movement controls on both native and Wasm. These fixtures use temporary libraries and do not alter the player's library.


The installed game page and package details display DOOM files in their actual loading-group order, with plain-language roles for base data, merging, sprite preparation and resource overrides. Within-group declaration order is preserved. File pickers and remembered paths remain tied to each original manifest entry, even when its visual position changes.


Library readiness and game-page file explanations share the same basic selected-file check: the path must resolve to a regular file within the existing 64 MiB per-ROM boundary. Missing files and oversized selections identify the affected requirement. After changing files outside the app, use **Recheck readiness**. Cancelling a picker preserves remembered selections; replacing one file leaves the other requirements unchanged. Format/content checks still run at launch, and final bounded file reads remain authoritative.

## Host snapshot slots

The reviewed Zelda profile supports host-managed snapshot slots alongside its
normal SRAM save. The desktop uses the original Shift+F1–F10 save/F1–F10 load
bindings by default. Settings can map each load/save key independently to any
of the 20 slots, or disable it; save preferences before the next game launch.
Old settings preserve the defaults. Headless `run --frames` supports `--load-slot 1..20`, `--save-slot 1..20`
and an explicit `--target native|wasm`. See the [slot guide](../engines/zelda3/README.md#snapshot-slots).
Other profiles do not acquire snapshot support or extra file permissions.

Slot ownership includes game ID, signed creator and save schema. Fixed-size
files carry the engine payload and wrapper counter, with a checksum binding the
metadata and payload. A checksum is not authentication of arbitrary imported
state. The host uses directory-relative `cap-std`/`cap-fs-ext` operations for
bounded reads, no-follow opens and atomic replacement. These are implementation
dependencies of GamePKG; they are not allowed guest crates and do not add SDK
file paths or snapshot imports. The existing snapshot partition accounts for
owned buffers and transfer scratch within the existing signed memory ceiling.

The native/Wasm maintainer gate `tools/check_zelda_inputs.py --gui-slots` tests
actual saved gameplay after restart, exact subsequent pixels/PCM, corrupt-file
rejection, storage totals, CLI slots and original keys in private Vulkan windows.
Production slot tests also run in ELF32 and PE32 (Wine), using a pinned
independently calculated little-endian file-format vector. Physical Windows,
complete 32-bit game playback, legacy state imports and full replay remain
separate qualification work. Players need none of the maintainer test tools.

## Pinned Zelda presentation and enhancement options

Recipes using `builtin_engine = "zelda3-45a149d8"` can configure the original
renderer with this optional signed table:

```toml
[zelda_video]
extra_width = 71
extend_height = false
no_sprite_limits = false
dim_flashes = false
skip_intro_on_keypress = false
```

`extra_width` adds 0–96 pixels **on each side** of the original 256-pixel image.
The example produces 398×224, matching the original integer 16:9 setting.
`extend_height = true` selects the original 240-line mode; otherwise height is
224. Omitting the table preserves the 256×224 image. Widescreen enables the
original sprite visibility and visual fixes. The optional `no_sprite_limits = true` selects the original renderer's extra
sprite mode. It defaults to false, preserving the console-style per-line object
and tile limits. Players can also use Allow extra sprites on the game page;
changing view shape or height preserves that choice. `dim_flashes = true` enables
the original port’s dimmer flash effect; it defaults off. Players can save the
same choice with Dim flashes under Display. It changes neither image dimensions
nor sprite limits. `skip_intro_on_keypress = true` enables the original earlier
button-press skip threshold during the intro; it defaults off. Players can save
that choice with Skip intro earlier on button press. Older display
preferences remain readable, with missing enhancement flags defaulting off. The host centers the complete
image without stretching its aspect ratio. Both native and Wasm paths use the
same options. Older Wasm engine caches need rebuilding for the current renderer
fixes. Readiness and Integrity check detect stale renderer revisions; Integrity
fix rebuilds them from verified source. Existing saved preferences and SRAM remain readable.

The table defines creator-signed defaults. Players can choose Original width,
Widescreen (16:9), Widest view and extended vertical view under Display on the
game page, then save for the next launch. These host-owned preferences are
scoped to the creator and game and persist across updates. Package default
removes the override and follows the current signed recipe. They change no
filesystem/network permissions or signed package bytes. Other engine profiles,
unknown fields, invalid types and widths above 96 are rejected. Existing image,
framebuffer, memory and execution limits still apply. This table does not enable
MSU music, translations, replays or save-state tools.

Original gameplay enhancement switches are available in a nested signed table.
Every omitted switch defaults to false. For example:

```toml
[zelda_video.enhancements]
item_switch_lr = true
disable_low_health_beep = true
```

Players can change the same switches in **Display → Gameplay enhancements**, then
save for the next launch. An explicit all-off override stays off even when the
recipe recommends enhancements; **Package default** restores the creator’s
signed choices. View/height changes preserve these choices.

| Manifest/CLI name | Original option |
| --- | --- |
| `item_switch_lr` | Cycle items with shoulder buttons |
| `item_switch_lr_limit` | Limit shoulder cycling to the first four inventory positions |
| `turn_while_dashing` | Turn while dashing |
| `mirror_to_darkworld` | Allow mirror travel to the Dark World |
| `collect_items_with_sword` | Collect items with the sword |
| `break_pots_with_sword` | Break pots with the sword |
| `disable_low_health_beep` | Disable the low-health beep |
| `show_max_items_in_yellow` | Highlight maximum item counts in yellow |
| `more_active_bombs` | Allow more active bombs |
| `carry_more_rupees` | Carry up to 9,999 rupees |
| `misc_bug_fixes` | Original port's miscellaneous bug fixes |
| `game_changing_bug_fixes` | Original port's gameplay-changing bug fixes |
| `cancel_bird_travel` | Allow canceling bird travel |

With bird-travel cancellation enabled, press X at the destination menu to cancel. A, B and Y confirm the selected destination. With the option disabled, X also confirms, following the original game. Face-button input is accepted after the original menu debounce countdown expires.

`item_switch_lr_limit` modifies shoulder-button cycling and needs
`item_switch_lr` to have an effect. It selects among the first four inventory
positions in the current order; if none is owned, selection stays unchanged.
The currency switch follows the original refill logic: existing balances remain
until gameplay updates them, and increases apply the selected cap. The bug-fix switches preserve the pinned
original port’s optional behavior, including changes to original game rules.
These flags do not grant filesystem or networking access. Configuration mapping,
serialization and selected gameplay paths are verified; they do not establish
complete campaign or every-feature interaction parity.

The CLI offers the same launch preferences:

```sh
gamepkg configure-display com.gamepkg.zelda3 --preset wide --extended-height --extra-sprites --dim-flashes --skip-intro
gamepkg configure-display com.gamepkg.zelda3 --preset original --enhancement item_switch_lr --enhancement disable_low_health_beep
gamepkg configure-display com.gamepkg.zelda3 --preset default
```

Presets are `default`, `original`, `wide` and `widest`. `--extended-height`
selects 240 lines. `--extra-sprites` enables the original extra-sprite mode.
`--dim-flashes` enables dimmer flashes. `--skip-intro` enables earlier button-press
intro skipping. Repeat `--enhancement NAME` for the named original gameplay
options above. Unknown names are rejected before saved settings change.
Override flags cannot accompany `default`. A current engine needs no
rebuild when preferences change; legacy engines need rebuilding for current display
features. Corrupt or oversized preference files fail validation without being
overwritten, and Integrity check includes saved display settings.

The reviewed renderer adaptations use unsigned bitplane/mask operations and
bounded horizontal mosaic spans for side areas and clipped windows. Their
count-checked replacements apply only to the pinned engine sources after hash
verification. The original HUD layer retains its 256-pixel width. A renderer
revision gate checks the current typed constant (including the compiler's
initialization wrapper) before accepting a Wasm cache; it does not execute code
during readiness checks.
