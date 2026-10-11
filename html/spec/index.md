# GamePKG format: foundation profile

The ZIP contains `manifest.toml`, `signature.toml`, `thumbnail.png`, `LICENSE`, `CREDITS.md`, `source/Cargo.toml`, `source/Cargo.lock`, `source/src/lib.rs`, optional `shaders/*.slang`, and `assets/**`. All paths are relative UTF-8 paths with forward slashes. Directory entries are omitted. Compression methods are store and zstd; the packer currently writes store for every file.

The manifest example in `examples/minimal/manifest.toml` is authoritative for currently supported fields. Unknown fields fail validation. `lockfile_sha256` is the SHA-256 of raw Cargo.lock bytes; it is lockfile integrity, not a complete SBOM.

## Signature encoding

`signature.toml` has `public_key` (32 bytes in hexadecimal) and `signature` (64 bytes in hexadecimal). Verification requires the public key to match an external trusted key.

The signed message is a SHA-256 tree digest. Initialize SHA-256 with UTF-8 bytes `GamePKG tree v1` and one NUL byte. Sort every file except `signature.toml` lexicographically by its UTF-8 name. For each, hash: name byte length as a little-endian u64, name bytes, content byte length as a little-endian u64, then the raw 32-byte SHA-256 of its contents. Ed25519 signs the resulting 32-byte digest. This includes the manifest, shaders, source, assets, and license. ZIP timestamps, entry order and compression are not signed.

This is an experimental profile; runtime ABI and compatibility policy are pending.

An optional top-level `NOTICE` carries license attribution and is included in the signed tree. Creators choose the license in their package's `LICENSE`; the host's Apache 2.0 license does not dictate the license of independently authored games.

## Creator identity and update continuity (planned installer policy)

A creator's trusted public key identifies the signing authority. The installer will bind that key to the package ID after the user's initial trust decision. An update must have the same ID, a strictly higher semantic version, and a valid signature from the pinned creator key. Unrelated keys cannot replace a package merely by claiming its ID. Key rotation requires authorization from the previously trusted key and a deliberate trust transition. The current tools verify individual signatures only; installer state, update ordering, and rotation remain unimplemented.
