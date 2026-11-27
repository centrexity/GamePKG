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
This is a compatibility check, not a creator signature or source provenance proof. The fingerprint includes the admitted source-kit identities and the platform builder, so identical app versions cannot hide different built-in engine recipes.

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

Native engine ports, their source kits and platform-specific setup live in the
separate GamePKG Ports checkout. GamePKG core does not ship those recipes or
grant them direct platform access. A port must migrate to the shared service
contract and pass admission before its signed package can launch; unmigrated
profiles remain blocked for build and play.

## Protections

GamePKG checks dependencies and source before injecting its bindings. Games
use GamePKG functions for system services. Game source cannot declare
dependencies, a build script, macros, aliases, indirect calls, unsafe code,
or direct system declarations.

The native compiler runs with fixed GamePKG arguments in an isolated work
area. It emits optimized objects. The host audits the exact objects for
forbidden imports before linking them to the trusted compatibility runtime.
Build completes source admission before it writes or signs a receipt. The signed
receipt binds the verified package digest, tree and source-kit identities,
scanner policy, SDK, compiler and target identities to the executable hash.
Launch checks that attestation and the executable hash before running code; it
does not repeat the source scan. Receipts without the required attestation fail
closed with a rebuild requirement.

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
