# Build a transparent GamePKG game

Start with `examples/minimal` or `examples/good/input-game` in the source repository. GamePKG is the format and player: creators choose where to host their own packages. Players inspect your source and permissions before approving your key.

## Creator checklist

1. Write `source/src/lib.rs` with `pub fn game_tick(input: u32, state: u64) -> u64`. Use direct function calls and checked source modules. The host adds the runtime bindings and export wrapper; do not add your own imports or export attributes.
2. Set a unique game ID, semantic version, runtime profile, and source lockfile hash in `manifest.toml`. Declare only the permissions you need, recommended memory, hard memory, and fuel limits. Guest networking must be `none`.
3. Put assets under `assets/`, Slang source under `shaders/`, and include a thumbnail, license and credits. Paths must be portable relative paths with forward slashes, no traversal, device names or symlinks.
4. Use only the exact dependencies below. Include `source/Cargo.lock`; put its SHA-256 in `lockfile_sha256`. GamePKG supplies the approved dependency source offline. Do not vendor dependency replacements into your package.
5. If your game requires ROM files, declare each requirement in the manifest. Players select their own files; read only those ROMs through `gamepkg::rom_count`, `rom_size_at` and `rom_read_at`. Do not fetch ROMs or ask for access to their folders.
6. Use `gamepkg` host functions for assets, saves, drawing, audio and achievements. Keep numbers portable: fixed-width serialized values and qualified conversions such as `u32::to_le_bytes(value)`. `usize` is for internal indexing, not stored data.
7. Pack, verify, install, build and play locally before publishing. Test missing assets, absent ROMs, denied permissions, corrupt saves, resource limits and both new and held input. Test both `core-v1` and `component-v1` if you claim both profiles.
8. Keep your private signing key outside the package and backed up. Publish your signed package at your chosen URL. An optional `update_url` points to the next signed package; updates retain your game ID and creator key and increase the version. Players need no private key or Python.

## Allowed guest crates

| Crate | Exact version | Configuration | Purpose |
| --- | --- | --- | --- |
| `gamepkg-byteorder` | `=0.1.0` | `default-features = false`, no extra features | Explicit little/big-endian `u32` reads and writes |
| `gamepkg-input` | `=0.1.0` | `default-features = false`, no extra features | Portable keyboard, controller and touchscreen button decoding |
| `gamepkg-file` | `=0.1.0` | `default-features = false`, no extra features | Scoped assets, selected ROMs and saves |
| `gamepkg-window` | `=0.1.0` | `default-features = false`, no extra features | Drawing into the host viewport |
| `gamepkg-networking` | `=0.1.0` | `default-features = false`, no extra features | Explicit permission-denied responses; no network access |

Only these exact GamePKG-owned wrappers are allowed guest dependencies. The `gamepkg-` prefix by itself grants nothing. Raw third-party crates, including `byteorder-lite`, are rejected. All five are approved by default. Players may revoke individual approvals in Settings or with `gamepkg allowlist remove NAME`. Unknown crates, renamed dependencies, Git/path sources, build scripts, extra features, and unapproved transitive dependencies are rejected. Approval cannot expand the bundled catalog.

```toml
[dependencies.gamepkg-input]
version = "=0.1.0"
default-features = false
```

All catalog entries are host-owned bundled source, not crates.io packages to download. Copy the lockfile in `examples/good/input-game` for input alone or `examples/good/wrapper-game` for all wrappers. Each dependency entry has only `name` and `version`, with no registry source or checksum. List root dependencies alphabetically. The host checks the complete graph and supplies its own source; `gamepkg allowlist list` reports each pinned source-file SHA-256.

## Scoped wrappers

Use `gamepkg_file::asset_read` for package-relative asset paths, `rom_count`, `rom_size_at` and `rom_read_at` for player-selected ROMs, and `save_size`, `save_read` and `save_write` for the game's save blob. These call the same permission-checked host operations in core and component games. They never open arbitrary host files. Existing `gamepkg::` bindings remain compatible scoped APIs.

Use `gamepkg_window::clear`, `rect`, `circle` and `asset_image` for drawing into the host-owned game viewport. The wrapper cannot create arbitrary windows or obtain GPU handles. Use the existing scoped host API for additional drawing and audio operations.

`gamepkg_networking::request(url, output)` always returns `Error::PermissionDenied`, leaves output untouched, and performs no I/O. The manifest still requires `network = "none"`. A wrapper name cannot enable networking; any future networking capability needs an explicit host policy and runtime implementation.

These wrappers narrow the callable API; they do not replace source checks, package signatures, the isolated compiler, or Wasm runtime restrictions. Guest declarations cannot select replacement sources or access the wrappers' private bridge symbols. The bridges are generated by the host with the same compiler for both runtime profiles.

### Migrate a direct third-party dependency

Packages that used `byteorder-lite` must replace it with `gamepkg-byteorder`, update source imports to `gamepkg_byteorder`, use its explicit-endian `u32` API, regenerate the approved lock graph and hash, increase their game version, and sign again with the same creator key. Existing settings naming `byteorder-lite` must be updated to the wrapper names; unknown catalog names are rejected rather than silently approved.

## Controller and touch input

Games consume one `u32` input value per tick. The same buttons work with the keyboard, a controller, or the player's on-screen touch controls. Set `[permissions] input = true`; otherwise the runtime passes zero.

```rust
pub fn game_tick(input: u32, state: u64) -> u64 {
    let mut next = state;
    if gamepkg_input::held(input, gamepkg_input::RIGHT) {
        next = next.wrapping_add(1);
    }
    if gamepkg_input::pressed(input, gamepkg_input::B) {
        next = 0;
    }
    next
}
```

`pressed` detects newly pressed buttons; `held` detects continuing presses. Constants are `LEFT`, `RIGHT`, `UP`, `DOWN`, `A`, `B`, `X`, `Y`, `L`, `R`, `START`, and `SELECT`. These are digital controls; this API does not expose raw touch coordinates, pressure, controller identities, or analog values to games.

The desktop host uses pinned `gilrs 0.10.10` for Linux/Windows controller input and the existing `egui 0.31.1` input layer for touch. These are host dependencies, not allowed guest dependencies: games cannot open devices or windows. D-pad and the left stick map to directions with a deadzone; face buttons map by position (south=B, west=Y, east=A, north=X). Shoulder buttons, Start and Select map to their names. Multiple connected controllers contribute to the same input. Choose **Touch controls** in the player to show the buttons; touch events also reveal them automatically. Movement and action buttons can be held simultaneously. Losing window focus clears device input. Android controller device access and an Android GUI are not supplied by this desktop integration.

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
