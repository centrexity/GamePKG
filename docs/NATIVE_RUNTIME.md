# Native builds and playback

The [shared native port service contract](PORT_SERVICES.md) covers input, windows, Vulkan graphics, audio, capability-relative file access, networking, and timing. GamePKG supplies a versioned compatibility layer that is compiled into each native game executable. The game process owns its own window and runtime services. GamePKG verifies and builds the game, sets the process permissions and resource limits, launches it, and records play history. The in-process build and launch path has a Linux x64 maintainer fixture. Each port still needs to migrate its platform calls and pass admission before it can launch.

**Build**, **Rebuild**, and **Play** use the shared compatibility layer on
Linux x64. Ordinary tick games declare `native_tick = true` and export
`game_tick`. Standalone games declare `native_services = true` and export
`game_main`. The creator signs the inputs, and GamePKG checks the source and
audits the exact game objects before linking the compatibility layer.

The app admits native builds and launches through this path only. Unmigrated
packages can still be verified and updated using their existing creator
identity. Prior native caches cannot grant admission. Native compatibility on
Windows and 32-bit Linux still needs a platform launcher.

**Build Wasm**, **Play Wasm**, and **Export for web** remain separate actions
for supported source packages. The CLI commands are `gamepkg build GAME_ID`
for native and `gamepkg build GAME_ID --target wasm` for Wasm. Use
`gamepkg run GAME_ID --frames 120` for a headless Wasm test.

Native cache entries bind the source package, host SDK and compiler inputs,
target and CPU, audited objects, and linked runtime. Rebuild for the
destination computer after moving a source package.

Maintainers must build the CLI and GUI together. Two binaries can report the same
app version and audio decoder while using different native build recipes, causing
a CLI-built game to appear unbuilt in the desktop app. The native Rust release
builder, relocated-entry check and release packager reject such pairs. The hidden
`check-build-identity` CLI command and `--check-build-identity` GUI option report
the version, platform, architecture and native recipe without opening a window or
loading player settings. `BUILD.json` records that matching identity and the
packager verifies the copied executable hashes against the validated pair.
This is a compatibility check, not a creator signature or source provenance proof. The fingerprint includes the admitted Takedawn source-kit revision and the platform builder, so identical app versions cannot hide different builtin engine recipes.

## Windows app icon

The launcher's runtime window icon remains the bundled PNG
`html/assets/gamepkg-icon.png`. On Windows, `build.rs` turns the matching
`html/assets/gamepkg.ico` into a Win32 `RT_GROUP_ICON`/`RT_ICON` resource and
links it into both `gamepkg.exe` and `gamepkg-gui.exe`. It uses `GAMEPKG_RC`
when set, otherwise Windows SDK `rc.exe` or MinGW `windres`. The release
packager and `tests/windows_package.py` reject an executable without a valid
resource, including a group with no 32-pixel-or-larger image. Compatibility SDK
game-host builds set `GAMEPKG_LOCAL_RUNTIME_CONFIG` and deliberately skip this
resource, so launcher artwork never appears in a built game by accident. The
icon is host build evidence only; it is not part of a game package or its
signature. Game build icons have a separate [qualification plan](GAME_BUILD_ICONS.md).

Native shader compilation uses Slang 2026.19. Windows prepares its verified
compiler automatically when the Slang folder is empty. Its compiler currently
requires 64-bit Windows; a 32-bit app on a 64-bit AMD64 OS can launch that tool
and still build/run native 32-bit game code. Shader-free games also build on
32-bit Windows. SPIR-V/WGSL work with Vulkan; GLSL also works with OpenGL. Windows prepares the pinned Microsoft DXC 1.9.2609 SDK when DXIL is selected.
DXIL compilation and cache integrity pass inside confinement; DXIL playback
still needs a renderer. Use SPIR-V/WGSL for Vulkan or GLSL for OpenGL. A shader
compiler for a genuinely 32-bit OS remains open.
Current checks validate shader compilation, reflection/translation and runtime
requests; they do not substitute for physical GPU/input/audio qualification.
Service-runtime packages may select `native_graphics = "textured-v1"` for a
host-pinned textured raster profile with one RGBA8 2D texture/sampler pair. It
still uses the WGSL/SPIR-V Vulkan preparation path; GLSL/OpenGL is only for the
ordinary 2D shader service.

The legacy C-based engines below retain direct platform support and remain
blocked. Their historical optional build/play controls and compiler exchanges
are not admission paths. The descriptions below document preserved migration
inputs; they do not claim these engines can currently launch.

## Legacy native engine profiles — currently blocked

The Takedawn profile uses prepared Rust game functions and the pinned original
C/C++ renderer, audio, input and system runtime. Its package can only select one
of the source kits admitted by GamePKG's policy; it cannot replace a kit, add game
source or choose build commands. Python and C2Rust are used during maintainer
preparation and are absent from the player's build and launch requirements.

On Linux, Build compiles offline with the local Rust compiler and Clang 18. CMake, Ninja,
SDL2, epoxy, OpenSSL, Vulkan and glslang 15 development tools are also required.
Use **Native tools folder** in Settings for an additional dependency prefix;
`GAMEPKG_TAKEDAWN_NATIVE_SYSROOT` overrides it. The first optimized build is
substantial; a complete prepared Rust build took about 77 minutes on the test
machine. Verified per-unit compiler checkpoints let interrupted or repeated
builds reuse completed work. Source, compiler, CPU and option changes invalidate
the corresponding checkpoints; corrupt objects are compiled again. These
compiler files count toward the game's displayed storage use. The current shared kit
supports Linux x86-64 and the native Windows x64 preview.

On Windows x64, **Prepare native build tools** in Settings (or
`gamepkg setup-native-tools`) installs the verified standalone Rust/LLVM-MinGW,
CMake, Ninja, SDL and Vulkan/glslang tools. No Python, WSL or vendor installer is
required. The current unreleased 0.1.4 recipe selects v15 (0.1.3 selects the older v14), with 24,003 prepared
Rust game functions across 487 units and 24,035 native exports. It requires the
0.1.18 development app. See [Windows source builds](../engines/takedawn/rust-port/WINDOWS_BUILD.md).

Play opens the original Vulkan window in its own native process. The selected
supported Xbox ISO is opened read-only and read in place. Saves and caches use
one game/creator/schema directory; another simultaneous session cannot write to
it. A successful startup remembers the selected ISO and begins play tracking.
An invalid disc does neither. Linux uses a 4 GiB physical-memory scope; Windows
uses a 4 GiB committed-memory job limit. **Memory ceiling** must be at least
4096 MiB. GamePKG's mute and volume
settings apply to the original runtime.

Native playback also enables immediate traps for untranslated instructions and
logs unresolved functions. A sticky host audit stops the session and reports any
reached unsupported path, including final output and managed-restart history.
This does not fill in missing game behavior. Update an older game package if its
runtime reaches such a path.

The optional **60 FPS crash cameras** setting uses the reviewed Takedawn
runtime's existing full-rate camera and simulation path. It is on by default
when the settings file does not specify a choice. An explicitly saved `false`
stays disabled. Change it in Settings or set `native_crash_60fps` in the app
settings file. It applies to the next
launch and managed restarts without rebuilding the game. Other engines do not
use this setting. This path advances one simulation step per frame and needs
sustained 60 FPS to preserve normal game speed. Leave it off if the hardware
cannot keep that pace. This setting enables the full-rate path; it does not
establish that a particular device or scene sustains 60 FPS.

Maintainers can check the Windows command-handoff overlays without game assets:
`python3 tools/check_takedawn_kick_handoff.py --toolkit /path/to/pinned/xboxrecomp`.
This stages the pinned sources, rejects corrupt inputs and source/output aliases,
and compiles the actual acknowledgement blocks for sanitized 64-bit and native
32-bit regression checks. The negative control reproduces a flush acknowledged
before a newly published command segment was consumed; the staged block keeps
the flush pending until that segment has been scanned. The Linux memory overlay
retains its previous behavior.

Add `--windows-output /path/to/fixtures` to cross-build the Win32 handoff fixture
from the actual staged helpers. Run `takedawn-kick-wake-fixture.exe` on Windows
to exercise 20,000 event handoffs and 2,000 timer-only handoffs. Run it again with
arguments `1`, `2`, and `3` in separate processes to force timer creation,
timer arming, and wait failures respectively. These media-free checks verify
ordering, early/repeated notifications and fallback progress; native race,
frame-time, save/reload and physical input/audio checks remain separate.

The optional **Native frame-stall watchdog** in Settings is off by default,
including for existing settings files. In the reviewed standalone Takedawn
runtime it watches actual frame flips and writes a hang report under the game's
save directory (`crash`) after the chosen 5–600 second delay. It does not kill or
restart the game. Before the first frame it allows at least 60 seconds. Loading
and minimized windows can legitimately stop producing frames, so enable this
only when investigating a stall. Changes apply on the next launch and also to
managed game restarts. This diagnostic option does not change sandboxed tick
deadlines, memory limits, or maintainer headless-test deadlines.

This reviewed host engine contains unsafe Rust and native C/C++ code. It uses
its own process isolation rather than the generic worker's per-tick host API
and syscall allowlist described below. On Linux, its namespace exposes system libraries,
GPU/audio/controller devices, selected desktop sockets, the read-only ISO and
its own writable data directory. It has no network namespace connection or
mount of the player's home/library. Verification and isolation do not establish
complete engine memory safety or full platform/gameplay fidelity. Qualification
details are in [the Rust port notes](../engines/takedawn/rust-port/README.md).
On Windows, the player uses an AppContainer and owned job, with networking off,
the selected read-only disc handle, verified runtime files and its own data
directory. Basic real-window access under that confinement is verified on
Windows; physical original-game Vulkan, sound and controllers still need testing.

## Protections

GamePKG checks dependencies and source before injecting its bindings. Games
use GamePKG functions for system services. Game source cannot declare
dependencies, a build script, macros, aliases, indirect calls, unsafe code,
or direct system declarations.

The native compiler runs with fixed GamePKG arguments in an isolated work
area. It emits optimized objects. The host audits the exact objects for
forbidden imports before linking them to the trusted compatibility runtime.
Build receipts authenticate the resulting executable.

GamePKG copies the verified executable into a private execution directory.
Its Linux namespace limits access to system resources, display and approved
devices. Networking follows the signed permission. The game's worker
detaches its descriptor table and installs the syscall filter before calling
game code. The executable owns its window and presents, reads input and
supplies audio through the linked compatibility layer. User-selected ROMs
remain open read-only. Private saves and all guest calls use capability
checks. The launcher enforces the signed process memory limit in a Linux
memory scope and checks the user's configured ceiling before launch.

## Rendering

The desktop uses Vulkan through the host renderer by default. Game source uses the GamePKG drawing, texture, shader and audio functions; it never receives a Vulkan device or window-system handle. Signed Slang fragment passes are compiled to WGSL or SPIR-V, validated, and rendered through host-owned pipelines with bounded textures.

OpenGL is an explicit alternative in Settings and requires an app restart. `gamepkg-gui --renderer opengl` can open the app on a machine without usable Vulkan. GLSL shader output is supported by the OpenGL renderer; Vulkan shader builds use WGSL or SPIR-V. The default shader output is WGSL.

Play opens a separate process and game window named for the game. The library stays available; closing it does not stop an already running game. Close the game window to end the session and finish saving progress. The game window owns its rendering and audio, while native game code continues to run in its restricted worker process. `gamepkg-gui --play GAME_ID --target native` (or `--target wasm`) starts this player directly.
