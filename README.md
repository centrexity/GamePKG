# GamePKG — signed, source-first game packages

GamePKG runs games from signed packages. A package carries its source, assets
and a creator signature; the host verifies that signature before installing,
building or launching it. Games are not trusted with direct filesystem,
graphics, controller, audio or network access. They call GamePKG-owned services,
and the host enforces the permissions and resource limits in the signed manifest.

This repository contains the generic host: CLI, desktop library, isolated build
policies, Wasm/native runtimes, shared host services and the public source
format. Ports live in the separate GamePKG Ports project and own game-specific
engines, source trees and media handling.

## Why a signed package?

- Install and launch start from the embedded signature, not a source rescan.
- `gamepkg list` reads signed manifests only; verification stays fast.
- Desktop, install and build paths enforce signed embedded checks.
- Native ports use signed source attestation without repeating the build-time
  AST scan on every launch.
- Saves, settings and remembered paths are scoped to the signed creator and
  game identity.
- Guests get bounded functions, not host APIs or Cargo dependencies.

## Quick start

Install a release from the [releases page](https://github.com/centrexity/GamePKG/releases)
or build locally with `cargo build --release --all-features`. Then:

```sh
# Import a signed catalog without downloading anything.
gamepkg list
gamepkg import-list ./catalog.gamepkglist

# Download and install a game; the creator must be approved once.
gamepkg download com.example.game --trust-creator
gamepkg install ./package.gamepkg --trust-creator

# Compile for this computer and play.
gamepkg build com.example.game
gamepkg run com.example.game
```

The desktop GUI (`gamepkg-gui`) performs the same signed operations and adds
library browsing, previews, settings, save inspection and build controls.

## What a package can access

A signed manifest declares permissions, memory/storage limits, required input,
save and audio behavior, optional game-data folders, and build policy. The host
grants only those capabilities. Guests use the WIT surface for drawing, audio,
input, assets, saves and allowed network operations. Native ports use the same
host services through the native session protocol; engines never receive raw
platform handles.

No game source may add Cargo dependencies, spawn host processes, open files or
sockets directly. Compat crates are GamePKG-owned path aliases with explicit
limits. Codecs, SIMD and platform access stay behind audited host code.

## Native ports

The host provides generic preparation, process ownership, window/device
adapters, compiler toolchain selection and a Windows runner/SDK. A port supplies
its reviewed engine source and manifest, and consumes those services through the
documented native interface. No game engine source ships in this repository.

See [native runtime](docs/NATIVE_RUNTIME.md) and
[port services](docs/PORT_SERVICES.md) for the protocol and service boundary.

## Development

```sh
cargo fmt --check
cargo check --offline --all-targets --all-features
cargo clippy --offline --all-targets --all-features -- -D warnings
cargo test --offline --all-features
```

Host-only gates cover build identity, native preparation, package verification
and the Windows runner. The [developer guide](docs/DEVELOPING_GAMES.md)
describes the source policy and package workflow.

## Documentation

- [Installation](docs/INSTALL.md) and [Windows installation](docs/INSTALL_WINDOWS.md)
- [Developing games](docs/DEVELOPING_GAMES.md)
- [Launch options](docs/LAUNCH_OPTIONS.md)
- [Networking](docs/NETWORKING.md)
- [Storage](docs/STORAGE.md)
- [Portability policy](docs/PORTABILITY.md)
- [Port fidelity](docs/PORT_FIDELITY.md)
- [Implementation status](docs/IMPLEMENTATION_STATUS.md)

## License and attribution

GamePKG is Apache-2.0 licensed. Host dependencies and vendored components keep
their upstream notices in [NOTICE](NOTICE) and under `vendor/`. Ports keep their
own engine, artwork and media notices in the Ports project. No game assets or
engines are bundled here.
