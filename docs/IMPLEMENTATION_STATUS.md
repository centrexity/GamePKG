# Implementation and verification status

This audit maps the starting design and subsequent requested features to implemented behavior and concrete verification. It is not a claim that every planned platform or graphics feature is finished.

## Public deliverables

The public repository is [centrexity/GamePKG](https://github.com/centrexity/GamePKG). [Linux and Windows preview 0.1.4](https://github.com/centrexity/GamePKG/releases/tag/v0.1.4) contains both CLI and desktop programs, checksums, setup tools, example source, and dependency sources/notices. Apache-2.0 and NOTICE provide the project's attribution terms. The three signed demo packages, catalog and portable store are attached to the same release. Players supply no signing key.

Locally, the final 0.1.3 archive passed `tests/release_package.py`: archive hashes, original dependency sources, both executable versions, a bundled dependency game compiled in isolation, CLI gameplay, integrity verification, and actual desktop gameplay. GitHub's recorded SHA-256 digests matched both local host artifacts and all seven demo artifacts.

The native Windows development preview at source commit `1dcf3d9` passed build, all-feature Rust tests, Clippy, ZIP packaging and `tests/windows_package.py` on Windows in [run 38177469252](https://github.com/centrexity/GamePKG/actions/runs/38177469252). Its extracted archive was independently checked locally: 84,276,827 bytes, SHA-256 `a256e48b5517f52eda75724f44f5c404be4700c9abecc7cf3b174b3c788ce043`, matching both executable hashes in `BUILD.json`. This preview predates the experimental WSL connection and does not prove native desktop GPU gameplay. macOS build, tests and Clippy passed in the same run.

Windows source build and playback are now verified at commit `2e6e5cb` in [run 38183943740](https://github.com/centrexity/GamePKG/actions/runs/38183943740): native build/tests/Clippy, real WSL 2 compilation, shared assets, both core and component runtime profiles, damaged-Wasm detection and repair, and actual native GUI blue/green/released-blue pixels driven by keyboard events. The GUI test uses checksum-pinned Mesa 26.2.4 software OpenGL on the hosted Windows runner; it is not a claim about every hardware driver. Screenshots identify each played game. The Windows archive passed its extracted-package checks and local checksum/executable-hash inspection: 84,471,577 bytes, SHA-256 `aae3806d39e4112692326f9b0248a09f13882fb5a26b0fe68736d687d85c13ad`. Test graphics DLLs are excluded from `bin/`.

Release 0.1.4 is public at source commit `2e8f1d07d01dc9e099fe26ab37a913c9efc6a2f2`. All native gates passed in [run 38185741076](https://github.com/centrexity/GamePKG/actions/runs/38185741076), including real Windows WSL compilation, core/component native GUI rendering/input, integrity repair, packaging and extracted installation. The Linux archive independently passed `tests/release_package.py` locally. All eleven GitHub release asset digests matched local verified files. Demo game version 0.1.2 keeps the same creator; a clean installed 0.1.1 upgraded without another creator approval, rebuilt and played. Stable update endpoints under `https://centrexity.github.io/GamePKG/demos/` returned exact signed-package hashes, and the installed game checked its live endpoint successfully.

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
| Tor update URLs | Onion traffic uses SOCKS5 with proxy-side DNS, cannot redirect to ordinary hosts, no direct fallback | `tests/tor_download.py` verifies routing and redirect rejection; `tests/tor_live.py` verifies signed install, update detection and same-creator update through a real v3 onion service |
| Multi-game catalogs and predownload state | Bounded `.gamepkglist`, atomic metadata-only import, Not downloaded rows, explicit selected download | `tests/catalog.rs`, `tests/web_library.py` |
| Safety-first README and homepage | Explain source distribution, capability limits, local optimization, update checks, and development limits | `README.md`, `html/index.html`, `html/guide/security.html`; all five public pages are deployed and HTTP-verified at https://centrexity.github.io/GamePKG/; custom gamepkg.com deployment is not established |
| Recommended memory and hard limits | Recommendation is advisory; manifest hard limit and user ceiling are enforced separately | `tests/manifest_memory.rs`, `tests/runtime.rs` |
| Settings page | Save/cancel/reset, validation, stored settings, compiler paths, limits, audio/backend and periodic update checks | `tests/settings.rs`, `tests/settings_gui.py`, `tests/updates_gui.py` |
| Hindsight Database/IndexedDB | Actual package worker API stores metadata, icons, catalog, settings, history, achievements and storage information | `tests/hindsight_storage.py`, `tests/web_library.py --play`; both tested package hashes are accepted by 0.1.3 |
| Play time and achievements | Measured time, stable declared achievement IDs, checkpointing and concurrent-safe merge | `tests/progress.rs`, `tests/progress_game.py`, `tests/progress_gui.py --indexeddb-package PATH` (real keyboard collection, return to library, actual Hindsight persistence and unauthorized browser restart), `tests/web_library.py --play` |
| Expected and actual space usage | Manifest recommendation and bounded inventory of packages, recovery, builds, saves, progress and shared assets | `tests/storage.rs`, `tests/progress.rs`, `tests/web_library.py` |
| Multiple user ROMs | Signed named requirements, ordered bounded private snapshots, permission checks and selection invalidation on identity change | `tests/manifest_roms.rs`, `tests/runtime_rom.py`, `tests/integrity_roms.py`, `tests/roms_gui.py` |
| Integrity check and repair | Signed archive, build receipt and playable entry validation; isolated rebuild; exact-signed-package store asset restoration | `tests/integrity_roms.py`, `tests/integrity_store.py`, `tests/special_files.py` |
| Slang shader compilation | Pinned Slang outputs and reflection for SPIR-V, WGSL, Metal, DXIL and GLSL; pinned DXC for DXIL | `tests/shaders.py` compiles actual outputs and rejects invalid resources/compiler changes |
| Open media and direct assets | PNG thumbnails/sprites, bounded AV1 preview and Theora fallback; verified asset offset reads | `tests/previews.py`, `tests/previews_gui.py`, `tests/sprites_gui.py --png`, `tests/security.rs` |
| Shared pool and portable store | Content-addressed references, deduplication, verified whole-bundle import and creator review | `tests/pool.rs`, `tests/store.rs`, `tests/shared_assets_game.py`, `tests/store_gui.py` |

The 0.1.3 source passed `cargo test --locked --all-features`, formatting and all-target/all-feature Clippy locally. Its Hindsight gameplay test verified real achievement unlocks and measured play time, including cached display after browser restart without host authorization. The downloaded 0.1.2 desktop was also exercised with all three public demos: actual guest colors and keyboard-driven square/sprite movement were checked from screenshots. New demo 0.1.1 retains the same creator; upgrading an installed 0.1.0 required no second trust approval, then its public signed update URL reported up to date and build/run/integrity checks passed.

The released 0.1.4 program also passed the actual Hindsight `tests/web_library.py --play --indexeddb-package PATH` flow locally: authenticated installed/catalog metadata, cached verified icons, live build status, storage information, play history and achievements survived a browser restart without host authorization; cross-origin requests were rejected. The public release's three demo URLs each passed download/install, isolated build, playback, integrity and live stable-update checks in a clean library. The deployed 0.1.4 download page passed browser checks at 320 and 1280 pixels without horizontal overflow. Its website deployment completed in run 38187428188.

## Remaining scope and limits

- Isolated source compilation is verified for Linux and the opt-in Windows WSL 2 connection. Windows native GUI GLSL rendering, keyboard input and repair passed for both runtime profiles using software OpenGL. Windows setup still requires a configured WSL 2 compiler and working desktop graphics driver. Cross-platform fixture tests cover core/component output and GLSL/SPIR-V/WGSL transfer. macOS host compilation, runtime tests and transferred-cache playback pass; isolated macOS compilation remains unfinished.
- Desktop rendering supports sprites and the documented fragment shader paths through OpenGL, including translated SPIR-V/WGSL and bounded texture binding. Native Metal/DXIL playback and broader vertex/compute/resource support are unfinished.
- Hindsight is supplied separately; the tested package is not redistributed. Its native desktop backend reports unavailable, so IndexedDB persistence is in the browser library. Installation, builds, gameplay and repairs use the desktop or CLI.
- A temporary live v3 onion service passed signed installation and same-creator updates using Tor 0.4.9.11, including NO_PROXY override. The custom gamepkg.com domain remains unverified. The static website is deployed at https://centrexity.github.io/GamePKG/; GitHub deployment run 38175133082 succeeded and all five public pages returned HTTP 200 with current release links.
- Key rotation, automatic save migration and shared-pool garbage collection are future development, not completed capabilities.

The workflows in `.github/workflows/ci.yml` and `portable.yml` run the listed regression gates. Inspect the exact run for a commit before claiming its hosted checks passed; a queued or running job is not a passing result.

## Source-policy, persistent database and web-export additions

The current source parses all packaged Rust files and resolves the callback-name
policy across modules. It rejects guest aliases, macros and indirect-call source
forms. Before a compiler is launched, it additionally parses all Rust files in
the exact private build tree, including trusted generated bindings and pinned
dependency source. Core and component runtime imports remain restricted to the
scoped API. `tests/modules.rs`, `tests/security.rs`, `tests/runtime.rs`,
`tests/components.rs` and staged-source unit tests cover these rules.

`examples/good/2048` adapts Adrien Ball's MIT-licensed 2048-rs row merging at the
revision recorded in its credits. The original license is included in the signed
package. All sixteen good examples and seven bad examples passed their actual
pack/verify/install/build/run or expected-rejection checks locally. The native
2048 GUI rendered tiles and changed the board on a right-arrow key; its library
**Export for web** button produced a complete player folder.

The home-folder library now holds an atomic `database/library.json` snapshot.
`tests/home_database.py` verified default home selection and persistent library
identity and records across two native host sessions. `tests/web_library.py
--play` verified the actual Hindsight package, play history and achievements,
including browser restart without host authorization. The native snapshot is
rebuilt from verified host data and supplements Hindsight's browser database.

`web-export ID --output FOLDER` exports a verified core or host-generated component guest build, a browser worker
player, assets, readable notices and the signed source package. Browser tests
verified rendered 2048 tiles and input, stop, output overwrite rejection,
artifact checksum rejection, scoped assets and traversal denial, explicit ROM
selection, persistent saves, achievements and termination of an infinite loop.
A Wasm memory-maximum test verifies that memory growth cannot exceed the manifest
hard limit. The GUI and CLI use the same export function.

Browser support covers both runtime profiles, scoped
asset/ROM/save/achievement operations, raw and PNG sprites, output-only audio
with mute, and verified WebGL 2 fragment/texture passes. Browser tests passed
actual sprite movement, audio scheduling/mute, and shader blue/green/released-blue
pixels for both profiles. The adapter selects exactly one validated embedded
guest module from a component and does not forward arbitrary component graphs.
A corrected preflight file bound matches the 64 MiB package-file limit; the
actual oversized-artifact test still reaches the isolated compiler, fails and
preserves earlier compiled output. The browser provides no Wasmtime fuel metering. Existing
published 0.1.4 binaries predate these source changes. Compiler isolation and
runtime capabilities remain necessary; Rust parsing does not prove safety.

The source is now version 0.1.5 so Windows compiler-worker version checks reject
older 0.1.4 workers that lack the complete private-tree parsing pass. Callback
checks also cover associated functions, tuple/enum constructors and implicit
prelude function values, while retaining ordinary bindings, constants and direct
calls. These additions passed the all-feature Rust test suite and local browser
checks. Published 0.1.4 archives remain the previous preview until replacement
archives are verified and released.

An actual 10,000-unary-operator source probe aborted the earlier CLI with a stack
overflow. The lexer preflight now rejects it, excessive grouping and long
operator expressions with a normal error before AST construction. The actual
CLI test verifies non-abort exit status and absence of published package data;
string/raw-string/comment tests ensure punctuation in data is not syntax nesting.
Near-boundary source probes (256, 512 and 900 operators) packed successfully with
a 2 MiB process stack, and all sixteen good/seven bad examples still passed.
