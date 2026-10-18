# GamePKG implementation and verification

This audit describes the current project. GamePKG is an independent Rust host with its own package format, source policy, desktop library, CLI and local browser view.

## Storage

The Rust host owns signed archives, catalog TOML, settings TOML, per-creator saves and progress, remembered ROM selections and compiled caches. The browser host rebuilds `database/library.json` from native records using atomic replacement. This snapshot is a recoverable index, not an independent authority for signatures or execution permissions.

The optional browser cache is implemented by GamePKG using the standard IndexedDB API. It requires no separately supplied package, database server or external language runtime. The authenticated host remains usable when browser caching is unavailable. Cache deletion cannot delete native saves or installed games. See [storage choices](STORAGE.md) for the database assessment.

## Implemented behavior and regression coverage

| Area | Behavior | Relevant verification |
| --- | --- | --- |
| Package identity | Embedded creator keys, signed contents and same-creator updates | `tests/security.rs`, `tests/registry.rs` |
| Game metadata | Signed titles, descriptions, authors, project links and genres; unverified catalog previews | `tests/metadata.rs`, `tests/metadata_gui.py` |
| Offline catalogs | Add games without downloading them or approving creators | `tests/catalog.rs`, `tests/library_play_gui.py` |
| Native compilation | Default Linux CPU build; explicit Wasm targets; no guest crates | `tests/native_build.py`, `tests/dependencies.rs` |
| Runtime isolation | Native worker process, mandatory syscall allowlist, namespaces, memory ceilings and deadlines | `tests/native_build.py`, `tests/runtime.rs` |
| Separate game windows | Independent player processes, saved progress on normal close | `tests/game_window.py`, `tests/library_play_gui.py` |
| Graphics | Vulkan default, explicit OpenGL, bounded shaders and textures | `tests/shader_gui.py`, `tests/appearance_gui.py` |
| Networking | Signed on/off networking; bounded host HTTPS GET, TCP and UDP | `tests/network_game.py`, `src/network.rs` tests |
| Reviewed original-data engine | Pinned Cube 2 source, signed selector, remembered read-only original directories, game-only Vulkan window, private edited maps, host-owned multiplayer and discovered servers | `tests/cube2_policy.rs`, `tests/cube2_package.py`, `tests/cube2_gui.py`, `tests/cube2_pointer_gui.py`, ignored `src/cube2/session.rs` original-engine tests |
| Player-owned ROMs | Multiple per-game remembered paths, bounded read-only snapshots | `tests/manifest_roms.rs`, `tests/remembered_roms_gui.py` |
| Progress | Play time, scoped achievements and concurrent-safe merges | `tests/progress.rs`, `tests/progress_gui.py --browser-cache` |
| Independent storage | Native restart persistence, browser restart/cache isolation, unauthorized-request rejection | `tests/home_database.py`, `tests/browser_storage.py`, `tests/web_library.py` |
| Integrity and repair | Signed archive/asset verification and target-specific rebuilds | `tests/integrity_roms.py`, `tests/integrity_store.py` |

## Platform limits

Direct native game compilation and worker isolation currently require Linux. Windows has native CLI/GUI executables and explicit Wasm build/play controls using the configured Linux compiler exchange. A Windows host build is not evidence of a Windows native game compiler. Vulkan playback also requires a working driver.

The web library displays metadata and cached icons; installation, builds, gameplay and repair remain in the native app or CLI. Browser cache quotas and eviction are browser-controlled. Native signed records and saves remain the recovery source.

Source checks complement process isolation and host validation; they are not a complete security boundary by themselves. Source availability and signatures do not prove that a game is harmless or identify a real-world author.

Hosted CI results must be inspected for the exact commit. Locally passing checks do not imply every hosted platform job or hardware driver has passed.
