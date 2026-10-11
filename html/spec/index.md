# GamePKG format: foundation profile

The ZIP contains `manifest.toml`, `signature.toml`, `thumbnail.png`, `LICENSE`, `CREDITS.md`, `source/Cargo.toml`, `source/Cargo.lock`, `source/src/lib.rs`, optional `shaders/*.slang`, and `assets/**`. All paths are relative UTF-8 paths with forward slashes. Directory entries are omitted. Compression methods are store and zstd; the packer currently writes store for every file.

The manifest example in `examples/minimal/manifest.toml` is authoritative for currently supported fields. Unknown fields fail validation. `lockfile_sha256` is the SHA-256 of raw Cargo.lock bytes; it is lockfile integrity, not a complete SBOM.

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

This remains an experimental runtime. Broader GPU resource support and
non-Linux isolated builds are unfinished.

An optional top-level `NOTICE` carries license attribution and is included in the signed tree. Creators choose the license in their package's `LICENSE`; the host's Apache 2.0 license does not dictate the license of independently authored games.

## Creator identity and update continuity

A creator's trusted public key identifies the signing authority. The installer binds that key to the package ID after the user's initial trust decision. An update must have the same ID, a strictly higher semantic version, and a valid signature from the pinned creator key. Unrelated keys cannot replace a package merely by claiming its ID. Key rotation requires authorization from the previously trusted key and a deliberate trust transition. The installed archive records the trusted key; install and update enforce same-ID/same-key continuity and increasing semantic versions. Key rotation remains unimplemented and key changes are rejected.

`requires_rom` is an optional top-level boolean, defaulting to false. True declares that a ROM must be supplied by the end user. It is signed metadata; the host never infers permission to fetch or redistribute a ROM. The CLI accepts `run <id> --rom <file>` and the desktop library accepts an explicit ROM file path. Launching a ROM-required game without a file fails. The host shares a bounded read-only session snapshot through scoped ROM operations; no folder access is granted.

Update URLs support HTTP/HTTPS v3 `.onion` services through a local Tor SOCKS proxy. Ordinary hosts require HTTPS. Onion redirects cannot leave onion hosts, and missing Tor connectivity never triggers a direct fallback.

## Game list format

`.gamepkglist` is a UTF-8 TOML document with `format_version = 1` and a `games` array written as `[[games]]` tables. Each entry requires `id`, `title`, and `download_url`; `version` (semantic version) and `requires_rom` (boolean) are optional unverified claims. Unknown fields, duplicate IDs, unsafe URLs, inputs exceeding 4 MiB, and more than 10,000 games are rejected.

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

The host API includes `draw-image(x, y, width, height, pixel-width, pixel-height, pixels)`. Coordinates use the 640×480 virtual canvas; the last argument is a row-major RGBA byte list with exactly four bytes per pixel. Each pixel dimension is 1–512. Image bytes count toward a per-frame 8 MiB budget and the existing 1,024-command budget. The host copies accepted pixels into a private frame snapshot, charges fuel, and validates finite display coordinates and dimensions. The desktop uses nearest-neighbor sampling and alpha transparency. Sprites are scoped host draw commands; this API does not expose arbitrary GPU bindings.
