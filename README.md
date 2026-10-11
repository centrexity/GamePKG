# GamePKG

GamePKG is a source-first game package format: Rust source, Slang shaders, and assets in one signed, random-access ZIP archive.

This repository implements the **package foundation**, not a playable host yet. It provides strict manifest parsing, bounded archive reads without extraction, Ed25519 signatures with explicit signer trust, SHA-256 tree and lockfile integrity, a conservative Rust source policy, and a CLI for packing, verifying, and reading assets.

## Try it

Install Rust, then run:

```sh
cargo test --locked
cargo run -- --help
```

A complete package input is in `examples/minimal`. Generate a private signing seed and its public key with your Ed25519 key tooling. The private key file must contain exactly 32 bytes encoded as 64 hexadecimal characters. Keep it outside your package directory and restrict access to it.

```sh
cargo run -- pack examples/minimal -o minimal.gamepkg --key /path/to/signing.key
cargo run -- verify minimal.gamepkg --trusted-key PUBLIC_KEY_HEX
cargo run -- asset minimal.gamepkg hello.txt --trusted-key PUBLIC_KEY_HEX -o hello.txt
```

Packing validates before publishing and refuses to overwrite files. The output must be outside the input directory. Signing keys are never stored in packages. Trust must come from a separately obtained public key; an archive's embedded key is not automatically trusted.

## Initial policy

- Format version 1; network must be `none`; positive fuel and memory limit up to 512 MiB.
- Required manifest, PNG-signature thumbnail, license, credits, Cargo manifest/lockfile, and `source/src/lib.rs`.
- At most 10,000 entries, 64 MiB per file, 256 MiB uncompressed total; stored and zstd entries only.
- No absolute paths, traversal, backslashes, duplicate paths, directory entries, or symlinks.
- Only `.slang` files under `shaders/`.
- Only one Rust source file, no dependencies, `package.build = false`, and restricted Cargo fields. Includes, attributes, external modules, unsafe blocks, foreign declarations, and filesystem/network/process/environment paths are rejected.
- Optional previews are rejected until codec and keyframe validation exists. PNG validation currently checks the magic bytes only.

The source scanner is an early policy filter, **not a security boundary**. Rust macros and compiler behavior require an OS-isolated build process, followed by a Wasm capability sandbox. This implementation never invokes untrusted builds or executes game code. Do not relax the source profile before those boundaries exist.

## Roadmap

1. **Implemented:** archive reader, manifest, signature/trust verification, source policy, pack/verify CLI, malicious-input tests.
2. OS-isolated offline Cargo wrapper with pinned compiler and approved dependency closures.
3. Wasmtime fuel, memory and epoch limits, minimal WIT host API, asset/input/save access.
4. Pinned Slang compilation and shader reflection validation.
5. Build/install/run CLI and transactional local package registry.
6. Broader bypass fixtures and preview decoding checks.
7. Thin feature-gated GUI.
8. Shared asset pool and store tooling.

See [the format notes](html/spec/index.md) and [security design](html/guide/security.md). The original design brief is preserved in `DESIGN.md`. No site has been deployed.

## Creator signing and updates

Creators sign their `.gamepkg` files with their own Ed25519 private keys. Verification proves the package was signed by the holder of a trusted key and that its signed contents have not changed. A signature alone does not prove a person's real-world identity: the public key must first be associated with the creator through a trusted channel.

The planned installer will pin the creator's public key to the package ID on first approved installation. Updates must keep the same ID, verify against that pinned key, and have a strictly higher semantic version. A different key must never silently take ownership of an existing ID. Key rotation needs authorization from the old key and an explicit trust transition; recovery after losing a key needs a separate approval flow. Installation, version ordering, rotation, and recovery are not implemented yet. The current `verify` command already requires an explicitly trusted public key.

## License and attribution

GamePKG and its minimal example are licensed under [Apache 2.0](LICENSE). [NOTICE](NOTICE) identifies the project and links to https://gamepkg.com. When redistributing covered code or derivative works, preserve applicable copyright and attribution notices, include the license, and mark modified files as required by its terms. NOTICE attribution may appear in distributed notices, source/documentation, or the appropriate notice display; the license does not mandate a clickable or prominently displayed backlink on every website.

Creators choose their own licenses for their games and assets. Using the GamePKG format or host does not automatically license a game under Apache 2.0. The package verifier accepts an optional top-level `NOTICE`, includes it in the signed tree, and does not require it for independently authored games.
