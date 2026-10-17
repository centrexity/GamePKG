# GamePKG format: foundation profile

The ZIP contains `manifest.toml`, `signature.toml`, `thumbnail.png`, `LICENSE`, `CREDITS.md`, `source/Cargo.toml`, `source/Cargo.lock`, `source/src/lib.rs`, optional `shaders/*.slang`, and `assets/**`. All paths are relative UTF-8 paths with forward slashes. Directory entries are omitted. Compression methods are store and zstd; the packer currently writes store for every file.

The manifest example in `examples/minimal/manifest.toml` is authoritative for currently supported fields. Unknown fields fail validation. `lockfile_sha256` is the SHA-256 of raw Cargo.lock bytes; it is lockfile integrity, not a complete SBOM.

## Guest dependency policy

Game source must have **no Cargo dependencies**, including `gamepkg-*` crates. `[dependencies]` must be absent or empty. Build, development, target-specific, workspace, patch, replace and feature sections are rejected. Renamed, optional, Git and path dependencies receive no exception.

`source/Cargo.lock` must use version 3 or 4 and contain exactly one package with the root Cargo name and version, without dependency lists, sources, checksums or extra package fields. Its raw bytes must match `lockfile_sha256`.

The host injects the scoped `gamepkg::` module after source validation. It supplies input, scoped assets/ROMs/saves, drawing, shaders, textures, audio and achievements. It is not a Cargo dependency. Packing, verification, installation and building enforce this policy; signatures and settings cannot waive it. GamePKG's own host dependencies belong to the player implementation, not the game package. See the [developer instructions](../guide/developers.html) for minimal Cargo files and API examples.

## Signature encoding

`signature.toml` has `public_key` (32 bytes in hexadecimal) and `signature` (64 bytes in hexadecimal). The manifest includes `creator_key` (32 bytes in hexadecimal), which must match the signature envelope key. Verification checks that embedded key; advanced callers may additionally supply an externally trusted key. An optional `update_url` is an HTTPS or Tor onion URL pointing directly to the latest signed package.

The signed message is a SHA-256 tree digest. Initialize SHA-256 with UTF-8 bytes `GamePKG tree v1` and one NUL byte. Sort every file except `signature.toml` lexicographically by its UTF-8 name. For each, hash: name byte length as a little-endian u64, name bytes, content byte length as a little-endian u64, then the raw 32-byte SHA-256 of its contents. Ed25519 signs the resulting 32-byte digest. This includes the manifest, shaders, source, assets, and license. ZIP timestamps, entry order and compression are not signed.

## Runtime and compatibility

`runtime_profile` is `core-v1` (the legacy default) or `component-v1`.
`entry_world` must be `game` for compilation and play. Source must expose a
callable `game_tick(input: u32, state: u64) -> u64`; root re-exports from checked
modules are supported. The host generates the export wrapper. Builds require
a valid root tick export before publishing their cache.

Component games use the versioned `gamepkg:engine@0.1.0` world in
`wit/game.wit`. Both profiles receive scoped assets, input, draw commands,
audio, saves, and explicitly selected ROM reads. They receive no WASI or
ambient filesystem/network imports. `engine_version` is a semantic-version
requirement checked against the host during verification and loading.
`save_schema` is a positive integer; changing it selects a separate save
namespace without deleting or converting earlier saves.

Compilation runs in an isolated Linux environment, including through WSL 2
on Windows. The desktop player presents through host-controlled Vulkan by default, with an explicit OpenGL option.

An optional top-level `NOTICE` carries license attribution and is included in the signed tree. Creators choose the license in their package's `LICENSE`; the host's Apache 2.0 license does not dictate the license of independently authored games.

## Creator identity and update continuity

A creator's trusted public key identifies the signing authority. The installer binds that key to the package ID after the user's initial trust decision. An update must have the same ID, a strictly higher semantic version, and a valid signature from the pinned creator key. Unrelated keys cannot replace a package merely by claiming its ID. The installed archive records the trusted key; install and update enforce same-ID/same-key continuity and increasing semantic versions. Updates that change the creator key are rejected.

`requires_rom` is an optional top-level boolean, defaulting to false. True declares that a ROM must be supplied by the end user. It is signed metadata; the host never infers permission to fetch or provide a ROM. The CLI accepts `run <id> --rom <file>` and the desktop library accepts an explicit ROM file path. Launching a ROM-required game without a file fails. The host shares a bounded read-only session snapshot through scoped ROM operations; no folder access is granted.

Update URLs support HTTP/HTTPS v3 `.onion` services through a local Tor SOCKS proxy. Ordinary hosts require HTTPS. Onion redirects cannot leave onion hosts, and missing Tor connectivity never triggers a direct fallback.

## Game list format

`.gamepkglist` is a UTF-8 TOML document with `format_version = 1` and a `games` array written as `[[games]]` tables. Each entry requires `id`, `title`, and `download_url`; `version` (semantic version), `requires_rom` (boolean), and `description` (up to 4096 bytes of plain text) are optional unverified claims. Unknown fields, duplicate IDs, unsafe URLs, inputs exceeding 4 MiB, and more than 10,000 games are rejected.

Import saves metadata only, with no package/thumbnail downloads and no creator trust. Games remain in a Not downloaded state until selected for download. A fetched package must verify and match the catalog ID; first installation requires creator approval. Installed signed manifest metadata takes precedence over catalog claims. Identical reimports merge without duplication; conflicting existing IDs reject the import atomically.

The `[limits]` table accepts `hard_memory_bytes` (required, positive, at most 512 MiB) and `recommended_memory_bytes` (optional, positive, no greater than the hard limit). The recommendation defaults to the hard limit and is advisory; the runtime enforces the hard limit on guest linear memory. Legacy `memory_bytes` is an alias for `hard_memory_bytes`, and both names cannot appear together.

## Shared store assets

A signed manifest may contain `shared_assets`, mapping asset-relative paths to
`sha256` (64 lowercase hexadecimal characters) and `size` (bytes, at most
64 MiB). Referenced paths cannot also be embedded under `assets/`. Logical
contents remain limited to 256 MiB. Reusing a digest requires the same size.

The host verifies signed references offline, then requires matching local pool
blobs before installation, builds, or play. It does not download blobs
automatically. Gameplay uses private verified snapshots through the existing
scoped asset API. `pack --shared-assets` creates store packages; ordinary
packages retain their embedded assets for self-contained sideloading.

## Portable store bundle

`.gamepkgstore` is a bounded ZIP containing `store.toml` with
`format_version = 1`, `packages/<id>.gamepkg`, and referenced shared blobs at
`blobs/<sha256>.blob`. All package signatures and referenced blob hashes/sizes
are checked before import. Unknown files, missing or unreferenced blobs,
duplicate entries, and conflicting identities or sizes are rejected. Limits:
10,000 entries, 64 MiB per entry, 256 MiB total. Packages retain their own
creator identities; importing new creators requires approval. Inspection does
not install or execute games.

## RGBA sprite drawing

The host API includes `draw-image(x, y, width, height, pixel-width, pixel-height, pixels)`. Coordinates use the 640×480 virtual canvas; the last argument is a row-major RGBA byte list with exactly four bytes per pixel. Each pixel dimension is 1–512. Image bytes count toward a per-frame 8 MiB budget and the existing 1,024-command budget. The host copies accepted pixels into a private frame snapshot, and validates finite display coordinates and dimensions. The desktop uses nearest-neighbor sampling and alpha transparency. Sprites are scoped host draw commands; this API does not expose arbitrary GPU bindings.

`draw-asset-image(path, x, y, width, height)` loads a PNG through the verified virtual asset API, including resolved shared references. The encoded file is limited to 2 MiB and the decoder allocation budget to 8 MiB; the same 512×512 dimensions, exact RGBA snapshot, and frame budget apply. Invalid data, unavailable assets, and paths outside the asset root return false.

GLSL fragment passes may use one host-owned combined `Sampler2D gamepkgTexture` at binding 0 via `draw_shader_image(index, pixel_width, pixel_height, rgba)`. Pixels are snapshotted before queueing, bounded to 512×512 and included in the shared 8 MiB frame image budget. Additional shader resources are rejected. Core and WIT runtimes support this API. Textured SPIR-V/WGSL playback validates one float 2D image and one non-comparison sampler at group 0 bindings 0 and 1 before translating to GLSL. SPIR-V builds define `GAMEPKG_SEPARATE_SAMPLER=1`; use the example’s explicit texture/sampler branch.

## Multiple player ROMs

Optional `[[roms]]` entries contain unique `id` and human-readable `title` fields. Up to eight entries are permitted. A nonempty list implies `requires_rom = true`; legacy `requires_rom = true` without entries requires one file. The player supplies every file in declaration order. Read-only session snapshots are limited to 64 MiB per file and 128 MiB combined. Core and component hosts expose count, indexed size and indexed read; legacy functions refer to the first file.

## Installed integrity and repair

Signed package verification covers the archive contents; shared assets are checked against signed hashes. Compiled Wasm has a host-written SHA-256 receipt checked before playback. Integrity repair restores damaged archives only from a verified local recovery copy, and rebuilds compiled files from verified source. A damaged or missing recovery copy requires a verified replacement original. Player saves and history are never reset automatically. Shader caches retain playback validation separately. Build checksums detect changes, but are not signatures protecting a compromised host.

## Shader asset textures and execution policy

`draw-shader-asset-image(index, path)` supplies a PNG from the verified virtual assets to the selected fragment pass. It uses the same path checks, 2 MiB encoded limit, 512×512 decoded limit, 8 MiB decoder allocation limit, private snapshots and frame image budget as sprite loading. Unknown indices and invalid files return false. The core ABI names this operation `draw_shader_asset_image`; the component API uses the hyphenated name.

The runtime uses memory limits and host time deadlines, without instruction metering. Legacy instruction-budget values in existing signed manifests are accepted but ignored; new manifests omit them.

## Game display metadata

Since 0.1.13, optional signed root fields include `title` (256 bytes), `description` (4096 bytes, with line breaks), `author` (128 bytes), `homepage` (2048 bytes, HTTPS/Tor), and `genres` (up to eight unique labels, 32 bytes each). Text is literal, without markup; control sequences and bidirectional formatting controls are rejected. Old packages fall back to their IDs. Catalog entries may also supply unverified descriptions. Installed signed metadata takes precedence. Use `engine_version = ">=0.1.13, <0.2.0"` and update older hosts first. See the developer guide for the manifest example.

## User-selected native-server permission

Only an exact host-approved folder-backed native selector may sign `game_data.user_selected_servers = true`. It requires `network = "multiplayer"`, empty static endpoint lists and no static local-network grant. The host validates and remembers an explicit user-selected join or hosting address and authorizes only its game port and adjacent server-information port for that session. Blank choices remain offline. Local/private addresses require a separate user opt-in. Choices are scoped to the signed game, creator and declaration; game code cannot change them. Ordinary guest packages cannot request this delegation.
