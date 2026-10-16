# GamePKG

**GamePKG** is a self-contained, sandboxed game package format and host. A `.gamepkg` file ships Rust source, Slang shaders, and assets. The host compiles the source to Wasm under a capability sandbox, compiles shaders per platform, and serves assets directly from the archive so nothing is extracted twice.

Site: [gamepkg.com](https://gamepkg.com)

## Goals

- Distribute games as source + assets, not opaque binaries.
- Enforce a deny-by-default sandbox: approved crates only, no build scripts, no proc macros, no binary includes, no ambient filesystem or network.
- Keep assets inside the archive (random-access zip, `store` for media, zstd for text).
- Require Slang for shaders; host compiles to SPIR-V / DXIL / Metal / WGSL.
- Open codecs for previews: PNG thumbnail, optional AV1-in-WebM, optional Theora-in-Ogg fallback.
- CLI for build/install/run and a GUI for browsing, installing, and playing.

## Package layout

```text
mygame.gamepkg          # zip, random-access
  manifest.toml
  thumbnail.png
  preview.av1.webm      # optional
  preview.ogv           # optional Theora fallback
  LICENSE
  CREDITS.md
  source/
    Cargo.toml
    Cargo.lock
    src/lib.rs
  shaders/
    **/*.slang
  assets/
    ...
```

`manifest.toml` holds identity, engine/format version, entry point (WIT world), resource limits, filesystem and network policy, shader profile, permissions (`input`, `save`, `audio`, etc.), save schema, and SBOM hash of the lockfile. Signature covers manifest + tree hash.

## Repository layout

```text
gamepkg/
  Cargo.toml
  README.md
  LICENSE
  html/                 # static site for gamepkg.com
    index.html
    spec/
    guide/
  src/
    main.rs             # CLI entry
    gui.rs              # GUI entry (feature-gated)
    lib.rs
    manifest.rs
    archive.rs          # zip index, offset reads, content-addressed refs
    build.rs            # cargo wrapper: allowlist, no build.rs, wasm target
    sandbox.rs          # Wasmtime linker, memory, epoch, WASI scoping
    shaders.rs          # slangc invocation + reflection
    hostapi.rs          # WIT world the guest imports
    verify.rs           # signature, SBOM, source scan
    cli/
    gui/
  tests/
    fixtures/           # minimal valid and intentionally malicious packages
    bypass_fs.rs
    bypass_net.rs
    bypass_include.rs
    bypass_build_script.rs
    bypass_proc_macro.rs
    bypass_crate.rs
    archive_roundtrip.rs
    shader_reject.rs
  crates/               # optional workspace members
    gamepkg-host-api/   # WIT + bindgen for guests
    gamepkg-cli/
```

## Host behavior

1. Open archive, verify signature and manifest version.
2. Scan `source/`: reject `build.rs`, proc-macro crates, path/git deps, `include_bytes!` / `include_str!` / `include!`, anything not on the crate allowlist.
3. `cargo build --target wasm32-wasip1 --offline --locked` (or `wasm32-unknown-unknown` with a custom WIT world). No preopens, no sockets unless the manifest grants a host-proxied capability.
4. Compile `.slang` with pinned `slangc` to the active backend; cache output outside the package.
5. Instantiate under Wasmtime with memory cap, epoch deadline, and an empty or tightly scoped linker. Guest sees only the host API (load asset by virtual path, input, draw, audio, save blob).
6. Serve assets by central-directory offset. Optional content-addressed pool for shared blobs across store packages; sideloaded packages stay self-contained.

## CLI

```text
gamepkg build <dir> -o game.gamepkg
gamepkg verify game.gamepkg
gamepkg install game.gamepkg
gamepkg run <id>
gamepkg pack --sign ...
```

## GUI

Feature-gated (`gui`). Library view, package detail (permissions, SBOM, thumbnail, preview), install/run, settings for backend, memory ceilings and time deadlines. Same core library as the CLI; GUI is a thin frontend (egui, iced, or a small system webview over the `html/` assets if you want one UI stack).

## Safety tests

`tests/` contains fixtures that must be rejected:

- Package with `build.rs` or a proc-macro dependency.
- Source using `std::fs`, `std::net`, `include_bytes!`, or `unsafe` blocks that reach libc (if native path is ever enabled; Wasm path should make these fail at compile or instantiate).
- Manifest claiming `net = "none"` while source imports sockets.
- Shader using inline SPIR-V or a non-`.slang` file.
- Archive with assets that escape the virtual root via `../`.
- Oversized preview, wrong codec, or missing keyframe.
- Tampered manifest or lockfile after signing.

Positive tests: minimal guest that loads an asset and ticks; offset reads that do not extract; Theora fallback when AV1 is declined.

## Static site (`html/`)

Landing page, spec summary, quickstart, security model, and crate allowlist policy. Plain static files so the same tree can be served from gamepkg.com or embedded later.

## Implementation order

1. Archive reader + manifest parser + signature verify.
2. Source scanner and cargo wrapper to Wasm.
3. Wasmtime sandbox + minimal host API.
4. Slang build step.
5. CLI (`verify`, `build`, `run`).
6. Bypass test suite.
7. GUI.
8. Content-addressed asset pool and store tooling.

## Non-goals for v1

No bundled native runtime inside the package, no prebuilt Wasm, no guest-controlled post-install scripts, no automatic network. Updates are a new signed package with the same `id` and a higher version.

This is the starting writeup only. The repo, code, and site come next.