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

The ordinary checked-source native worker supports Linux and Windows x86/x64 in
the current development source. The normal loading path selects native builds,
retains scoped saves and records play history. Verified Rust/linker tools and
Windows Slang tools are prepared without WSL/Python. Slang currently requires a
64-bit AMD64 OS (a 32-bit app can launch its compiler there); shader-free games
also build on 32-bit Windows. The reviewed Takedawn profile remains Windows x64;
other reviewed engine profiles retain their separate documented platform limits.

[Actual Windows shader/source checks](https://github.com/centrexity/GamePKG/actions/runs/40662676099)
pass at `5df761a`; the [normal game loader integration](https://github.com/centrexity/GamePKG/actions/runs/40670512933)
passes at `0f9c0a0`. The [mixed-bitness workflow](https://github.com/centrexity/GamePKG/actions/runs/40672606863)
also passes at `3442e33`: both app/game widths build and load all three desktop
shader backends on 64-bit AMD64 Windows. The 32-bit CI processes run on a 64-bit
OS; genuinely 32-bit-OS and physical driver qualification remain separate.
These checks cover signed native builds, scoped assets/saves/history,
SPIR-V/WGSL/GLSL validation and native requests, corrupt outputs and denied host
includes. They do not verify actual native shader pixels or every physical
GPU/controller/audio setup. [Confined DXIL compilation and cache checks](https://github.com/centrexity/GamePKG/actions/runs/40676730080)
pass at `f0d1dc3` in both app widths on AMD64 Windows, including verified DXC
setup, corrupt output and denied host includes. DXIL playback, a truly 32-bit-OS
shader compiler and broader platform/engine delivery remain open. See [native profile boundaries](NATIVE_RUNTIME.md).

Linux source builds now select the app's exact target for both Rust and Clang,
including a 32-bit app using a 64-bit installed compiler. Local signed ELF32 and
ELF64 fixtures pass native Build, assets/drawing, scoped saves across fresh
processes, normal-loader native selection, increasing play history, wrong-width
and corrupt-artifact rejection. Optimized signed DOOM and Zelda3 C-engine builds
and artifact integrity also pass at both widths without original media. Matching
32-bit development libraries/headers are needed; the 32-bit CI job now explicitly
installs them and runs both native fixtures. These build checks do not establish
32-bit original-game graphics/audio/controllers or gameplay.

The v18/0.1.7 development candidate requires app 0.1.20. It packages the GPU
report-index correction with unchanged prepared game code. Its optimized native
Linux and physical Windows Build, selected/remembered ISO, history and integrity
checks pass. The new Linux runtime also completes a three-lap Race and normal
autosave/menu return; the Windows GUI passes Ready and native Play child/normal
close checks. The v18 Windows runtime also passes a protected three-lap
Race/autosave/fresh-record-reload test on real Intel Vulkan hardware, with all
five saved files unchanged during reload. The qualified v17/0.1.6 baseline uses
app 0.1.19. Its selected and remembered
in-place ISO launch, history, integrity and the current desktop library pass
their scoped checks. Physical Windows also has a protected native three-lap
Race/save/reload check; manual controllers and audible output remain separate.
The current Windows preview launcher opens the prepared library with Play
enabled. An older app window does not automatically use that preview library.

The current Linux native game additionally earns Waterfront Face-Off Gold and
Compact Grand Prix Bronze through the original game's maintainer AI steering
helper. All three Grand Prix races finish naturally, with cumulative standings,
normal rewards, successful autosave and campaign return. A fresh process retains
5/173 completed events, 7,140 points, nine takedowns, the new Downtown race and
all three unlocked Muscle cars. All five earned profile files remain unchanged
during that reload. These are selected campaign routes, not full campaign,
manual physics/controller or audio-equivalence qualification. See the
[current validation scope](native-windows-gameplay-validation.md#current-v17-campaign-and-preview-checks).

The earlier v16/0.1.5 recipe requires the 0.1.18 development app. Native Team
Crush, Double Impact, Party Crash and a three-lap two-player
Race now have scoped completion evidence. In the Race with zero CPU rivals, both
players finish naturally: P1 first in 10:20.03 and P2 second in 11:26.56. Normal Continue
actions traverse results, Score (1–0), successful autosave and mode selection.
The private profile changes while the original reference saves and executable
remain unchanged. A fresh process loads that earned profile and displays the
Downtown Southbound lap records P2 2:25.15 and P1 2:38.53, matching the completed
race; all five UDATA/TDATA profile files remain byte-identical after reload.
A further original three-lap race on the same track also completes with the
default four CPU rivals: P2 fifth in 11:57.71, P1 sixth in 15:47.01, followed by
Score 0–1, successful autosave and mode return retaining 0–1. Normal controller
recovery and boost free cars after collisions; no game-state edits are used.
This is Linux headless Vulkan coverage with dummy audio. Other rival counts
and tracks, complete campaign, physics/audio equivalence and physical Windows
qualification remain open.

The default Party Crash Solo route also completes two shared-controller turns
in one round at Trailer Trash: both score $84,633, then original Final Standings,
Continue/Retry and main-menu return work. Its profile files remain unchanged;
no Party Crash autosave or saved-score reload is claimed.

The Double Impact route independently retains its 0–1 win count on return to
mode selection. Linux library Play at app commit `85297c3` additionally verifies
the remembered in-place ISO, earned profile, separate game window, normal
window-manager close, reaping and automatic play-history/readiness refresh.

Windows-under-Wine normal signed v16 Build and integrity repair pass using all
487 retained game objects and a Windows-side Z: temporary directory. These are
warm Build/repair checks, not default C: temporary-directory, cold-build or
protected original-game playback qualification. The immutable Windows source
preview pins app commit `85297c3` and the signed v16 package; subsequent app
changes do not relabel that bundle. See the
[matching preview and evidence](native-windows-gameplay-validation.md).

The earlier v15/0.1.4 multiplayer PS2 input fix has sanitized 32/64-bit input checks. The normal
signed Linux v15 package passes cold optimized Build, in-place/remembered ISO
launches, play history and integrity repair with all 487 object hashes/timestamps
and player data preserved. The exact repaired executable passes original Race
and Crash plus three both-player acceleration/pause/Resume routes. The full
Windows v15 signed cold Build/repair passes in
[run 40612591810](https://github.com/centrexity/GamePKG/actions/runs/40612591810):
15,968-second optimized Build, automatic native readiness, rejected companion
corruption and 112-second repair retaining all 487 object hashes/timestamps and
signed package identity. That check uses no original game media. Physical Windows
gameplay qualification remains open. The v14 evidence below remains specific to
that earlier kit.
The published 0.1.17 app does not include this native engine. A Windows host build
or asset-free window test does not prove original-game Vulkan/audio/controller
behavior on physical Windows hardware. The v14 Windows signed Build/repair
gate passes in [run 40446319498](https://github.com/centrexity/GamePKG/actions/runs/40446319498),
including all 487 units, native readiness and corruption/repair with unchanged
unit hashes/timestamps. Physical game hardware qualification remains separate
from those compiler checks and the Windows-under-Wine routes. See the [gameplay validation scope](native-windows-gameplay-validation.md).

The verified v14 Linux build also earns an original Downtown Road Rage1 Bronze
medal through normal steering, accelerator and boost inputs with debug AI off.
Original rewards, two event unlocks and autosave survive a fresh process with
unchanged saved bytes. Reviewed actions use the original pause menus; this is
scoped medal/persistence coverage, not a continuous real-time campaign or physical
Windows qualification. A separate unpaused headless run also earns Bronze and
verifies original rewards/autosave plus an unchanged fresh-process medal and
progress reload, with actual controller-read and strict native-path checks.
This does not qualify exact wall-clock pacing or physical controllers.
The earlier v14 preview uses app `f87f383`. The earlier immutable v15b hardware
bundle uses verified app `081a5b1`, the admitted v15 recipe and separate preview
library/settings. Its actual Windows CLI signature import and CMD launcher pass
under private Wine. It deliberately excludes newer generic-compiler changes;
original-game physical Windows checks remain pending. See the
[bundle and evidence](native-windows-gameplay-validation.md).

The web library displays metadata and cached icons; installation, builds, gameplay and repair remain in the native app or CLI. Browser cache quotas and eviction are browser-controlled. Native signed records and saves remain the recovery source.

Source checks complement process isolation and host validation; they are not a complete security boundary by themselves. Source availability and signatures do not prove that a game is harmless or identify a real-world author.

Hosted CI results must be inspected for the exact commit. Locally passing checks do not imply every hosted platform job or hardware driver has passed.
