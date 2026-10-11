# Implementation and verification status

This audit maps the starting design and subsequent requested features to implemented behavior and concrete verification. It is not a claim that every planned platform or graphics feature is finished.

## Public deliverables

The public repository is [centrexity/GamePKG](https://github.com/centrexity/GamePKG). [Linux preview 0.1.3](https://github.com/centrexity/GamePKG/releases/tag/v0.1.3) contains both CLI and desktop programs, checksums, setup tools, example source, and dependency sources/notices. Apache-2.0 and NOTICE provide the project's attribution terms. The three signed demo packages, catalog and portable store are attached to the same release. Players supply no signing key.

Locally, the final 0.1.3 archive passed `tests/release_package.py`: archive hashes, original dependency sources, both executable versions, a bundled dependency game compiled in isolation, CLI gameplay, integrity verification, and actual desktop gameplay. GitHub's recorded SHA-256 digests matched both local host artifacts and all seven demo artifacts.

## Requested behavior and evidence

| Requirement | Implemented behavior | Verification sources |
| --- | --- | --- |
| Source game format and embedded creator identity | Signed manifest/tree, lockfile hash, stored/zstd ZIP entries, embedded Ed25519 public key | `tests/security.rs`, `tests/manifest_compatibility.rs`, `tests/examples.rs` |
| Creator approval and signed updates | First-creator review; existing IDs retain creator continuity and require higher versions; signed update URL | `tests/registry.rs`, `tests/github_download.py`, `tests/updates_gui.py` |
| Positive and malicious examples | Every good example builds and ticks; every bad example fails before publishing a package | `tests/good_examples.py`, `tests/security.rs` |
| Isolated local compilation and optimization | Offline Linux compiler sandbox, private work area, approved pinned dependencies, bounded resources; optimization setting | `tests/isolated_build.py`, `tests/approved_crate_game.py`, `tests/compiler_limits.py`, `tests/settings_gui.py` |
| Scoped runtime | No ambient WASI filesystem or sockets; typed imports for permitted assets, input, saves, ROMs, draw and audio | `tests/runtime.rs`, `tests/components.rs`, `tests/runtime_assets.py`, `tests/runtime_saves.py` |
| CLI and playable desktop library | Icons, versions, build/rebuild/play, return to library, details and rejection notices | `tests/cli.rs`, `tests/library_gui.py`, `tests/library_play_gui.py`, `tests/release_package.py` |
| URL downloads and GitHub projects | HTTPS, explicit GitHub tags and unique matching release assets; bounded transfers and signature review | `tests/https_download.py`, `tests/github_download.py` |
| Tor update URLs | Onion traffic uses SOCKS5 with proxy-side DNS, cannot redirect to ordinary hosts, no direct fallback | `tests/tor_download.py` uses an actual local SOCKS protocol server; a live Tor onion service has not been tested |
| Multi-game catalogs and predownload state | Bounded `.gamepkglist`, atomic metadata-only import, Not downloaded rows, explicit selected download | `tests/catalog.rs`, `tests/web_library.py` |
| Safety-first README and homepage | Explain source distribution, capability limits, local optimization, update checks, and development limits | `README.md`, `html/index.html`, `html/guide/security.html`; all five public pages are deployed and HTTP-verified at https://centrexity.github.io/GamePKG/; custom gamepkg.com deployment is not established |
| Recommended memory and hard limits | Recommendation is advisory; manifest hard limit and user ceiling are enforced separately | `tests/manifest_memory.rs`, `tests/runtime.rs` |
| Settings page | Save/cancel/reset, validation, stored settings, compiler paths, limits, audio/backend and periodic update checks | `tests/settings.rs`, `tests/settings_gui.py`, `tests/updates_gui.py` |
| Hindsight Database/IndexedDB | Actual package worker API stores metadata, icons, catalog, settings, history, achievements and storage information | `tests/hindsight_storage.py`, `tests/web_library.py --play`; both tested package hashes are accepted by 0.1.3 |
| Play time and achievements | Measured time, stable declared achievement IDs, checkpointing and concurrent-safe merge | `tests/progress.rs`, `tests/progress_game.py`, `tests/web_library.py --play` |
| Expected and actual space usage | Manifest recommendation and bounded inventory of packages, recovery, builds, saves, progress and shared assets | `tests/storage.rs`, `tests/progress.rs`, `tests/web_library.py` |
| Multiple user ROMs | Signed named requirements, ordered bounded private snapshots, permission checks and selection invalidation on identity change | `tests/manifest_roms.rs`, `tests/runtime_rom.py`, `tests/integrity_roms.py`, `tests/roms_gui.py` |
| Integrity check and repair | Signed archive, build receipt and playable entry validation; isolated rebuild; exact-signed-package store asset restoration | `tests/integrity_roms.py`, `tests/integrity_store.py`, `tests/special_files.py` |
| Slang shader compilation | Pinned Slang outputs and reflection for SPIR-V, WGSL, Metal, DXIL and GLSL; pinned DXC for DXIL | `tests/shaders.py` compiles actual outputs and rejects invalid resources/compiler changes |
| Open media and direct assets | PNG thumbnails/sprites, bounded AV1 preview and Theora fallback; verified asset offset reads | `tests/previews.py`, `tests/previews_gui.py`, `tests/sprites_gui.py --png`, `tests/security.rs` |
| Shared pool and portable store | Content-addressed references, deduplication, verified whole-bundle import and creator review | `tests/pool.rs`, `tests/store.rs`, `tests/shared_assets_game.py`, `tests/store_gui.py` |

The 0.1.3 source passed `cargo test --locked --all-features`, formatting and all-target/all-feature Clippy locally. Its Hindsight gameplay test verified real achievement unlocks and measured play time, including cached display after browser restart without host authorization. The downloaded 0.1.2 desktop was also exercised with all three public demos: actual guest colors and keyboard-driven square/sprite movement were checked from screenshots. New demo 0.1.1 retains the same creator; upgrading an installed 0.1.0 required no second trust approval, then its public signed update URL reported up to date and build/run/integrity checks passed.

## Remaining scope and limits

- Isolated source compilation is implemented for Linux. Windows/macOS automated checks cover host compilation and portable verification/runtime tests; they do not prove isolated source builds or native desktop GPU playback there.
- Desktop rendering supports sprites and the documented fragment shader paths through OpenGL, including translated SPIR-V/WGSL and bounded texture binding. Native Metal/DXIL playback and broader vertex/compute/resource support are unfinished.
- Hindsight is supplied separately; the tested package is not redistributed. Its native desktop backend reports unavailable, so IndexedDB persistence is in the browser library. Installation, builds, gameplay and repairs use the desktop or CLI.
- A live onion service and the custom gamepkg.com domain remain unverified. The static website is deployed at https://centrexity.github.io/GamePKG/; GitHub deployment run 38175133082 succeeded and all five public pages returned HTTP 200 with current release links.
- Key rotation, automatic save migration and shared-pool garbage collection are future development, not completed capabilities.

The workflows in `.github/workflows/ci.yml` and `portable.yml` run the listed regression gates. Inspect the exact run for a commit before claiming its hosted checks passed; a queued or running job is not a passing result.
