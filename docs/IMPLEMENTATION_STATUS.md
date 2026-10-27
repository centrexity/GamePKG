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
| Native compilation | Default Linux checked-source CPU build; reviewed Takedawn native profile on Linux/Windows x64; explicit Wasm targets; no guest crates | `tests/native_build.py`, `tests/dependencies.rs`, `tools/check_takedawn_windows_package.ps1` |
| Runtime isolation | Ordinary Linux worker syscall allowlist, namespaces, memory ceilings and tick deadlines; reviewed native engines use their own process boundaries | `tests/native_build.py`, `tests/runtime.rs`, `src/takedawn_player.rs`, `src/takedawn/windows_player.rs` |
| Separate game windows | Independent player processes, saved progress on normal close | `tests/game_window.py`, `tests/library_play_gui.py` |
| Graphics | Vulkan default, explicit OpenGL, bounded shaders and textures | `tests/shader_gui.py`, `tests/appearance_gui.py` |
| Networking | Signed on/off networking; bounded host HTTPS GET, TCP and UDP | `tests/network_game.py`, `src/network.rs` tests |
| Reviewed original-data engine | Pinned Cube 2 source, signed selector, remembered read-only original directories, game-only Vulkan window, private edited maps, host-owned multiplayer and discovered servers | `tests/cube2_policy.rs`, `tests/cube2_package.py`, `tests/cube2_gui.py`, `tests/cube2_pointer_gui.py`, ignored `src/cube2/session.rs` original-engine tests |
| Player-owned ROMs | Multiple per-game remembered paths, bounded read-only snapshots | `tests/manifest_roms.rs`, `tests/remembered_roms_gui.py` |
| Progress | Play time, scoped achievements and concurrent-safe merges | `tests/progress.rs`, `tests/progress_gui.py --browser-cache` |
| Independent storage | Native restart persistence, browser restart/cache isolation, unauthorized-request rejection | `tests/home_database.py`, `tests/browser_storage.py`, `tests/web_library.py` |
| Integrity and repair | Signed archive/asset verification and target-specific rebuilds | `tests/integrity_roms.py`, `tests/integrity_store.py` |
| Reviewed standalone native game | Prepared Rust game functions, original C/C++ platform runtime, read-only ISO in place, scoped saves, native restart ownership and play time; optional frame-stall reports default off | `engines/takedawn/rust-port/WINDOWS_BUILD.md`, `docs/native-windows-gameplay-validation.md`, focused `native_windows_window.yml` gate |

## Platform limits

The ordinary checked-source native worker currently requires Linux. The separate
reviewed Takedawn profile builds directly for Linux x64 and Windows x64, using a
host-admitted prepared source kit. Its managed standalone Windows compiler needs
no WSL or Python. Other Windows source profiles use explicit Wasm build/play
controls and the optional Linux compiler exchange. See [native profile boundaries](NATIVE_RUNTIME.md).

The current development recipe is v14/0.1.3 and requires the 0.1.18 development app.
The published 0.1.17 app does not include this native engine. A Windows host build
or asset-free window test does not prove original-game Vulkan/audio/controller
behavior on physical Windows hardware. The full v14 Windows signed Build/repair
gate passes in [run 40446319498](https://github.com/centrexity/GamePKG/actions/runs/40446319498),
including all 487 units, native readiness and corruption/repair with unchanged
unit hashes/timestamps. Physical game hardware qualification remains separate
from those compiler checks and the Windows-under-Wine routes. See the [gameplay validation scope](native-windows-gameplay-validation.md).

The current v14 Linux build also earns an original Downtown Road Rage1 Bronze
medal through normal steering, accelerator and boost inputs with debug AI off.
Original rewards, two event unlocks and autosave survive a fresh process with
unchanged saved bytes. Reviewed actions use the original pause menus; this is
scoped medal/persistence coverage, not a continuous real-time campaign or physical
Windows qualification. The private Windows preview uses app `f87f383` and a
verified actual-Windows CI archive; original-game hardware checks remain pending.

The web library displays metadata and cached icons; installation, builds, gameplay and repair remain in the native app or CLI. Browser cache quotas and eviction are browser-controlled. Native signed records and saves remain the recovery source.

Source checks complement process isolation and host validation; they are not a complete security boundary by themselves. Source availability and signatures do not prove that a game is harmless or identify a real-world author.

Hosted CI results must be inspected for the exact commit. Locally passing checks do not imply every hosted platform job or hardware driver has passed.
