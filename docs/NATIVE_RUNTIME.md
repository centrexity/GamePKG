# Native builds and playback

On Linux, **Build** and **Rebuild** compile the signed game source directly for the current operating system and CPU. They do not also build Wasm or a web player. **Build Wasm**, **Play Wasm**, and **Export for web** are separate actions. The CLI equivalents are `gamepkg build GAME_ID` and `gamepkg build GAME_ID --target wasm`.

Native C-based ports need Clang 18 and lld-18; they do not need wasi-libc or a WASI sysroot. The optional Wasm build of a C-based port does need those libraries. The saved **C/WASI build tools folder** applies to that optional compiler. Games that do not call shader functions do not require Slang for their native build. Shader games require the pinned Slang compiler.

Native executable caches are specific to the package contents, host version, host bindings, operating system, CPU architecture and detected CPU features. Moving source packages between machines remains supported; build them for the destination machine. A native build does not replace a separately built Wasm artifact. Optional DOOM Wasm engines also record the reviewed source-kit fingerprint. When that kit changes, readiness, launch and integrity checks require rebuilding the engine; Integrity fix rebuilds it while retaining the signed package and saves. This compatibility marker supplements the build receipt and runtime validation.

The current native worker implementation requires Linux. The Windows desktop app retains explicit Wasm build/play controls and its configured Linux compiler exchange. It does not silently present a Wasm build as a direct CPU build.

## Reviewed native engine profiles

The Takedawn profile uses prepared Rust game functions and the pinned original
C/C++ renderer, audio, input and system runtime. Its package can only select the
source-kit digest compiled into GamePKG; it cannot replace that kit, add game
source or choose build commands. Python and C2Rust are used during maintainer
preparation and are absent from the player's build and launch requirements.

Build compiles offline with the local Rust compiler and Clang 18. CMake, Ninja,
SDL2, epoxy, OpenSSL, Vulkan and glslang 15 development tools are also required.
Use **Native tools folder** in Settings for an additional dependency prefix;
`GAMEPKG_TAKEDAWN_NATIVE_SYSROOT` overrides it. The first optimized build is
substantial; a complete prepared Rust build took about 77 minutes on the test
machine. Verified per-unit compiler checkpoints let interrupted or repeated
builds reuse completed work. Source, compiler, CPU and option changes invalidate
the corresponding checkpoints; corrupt objects are compiled again. These
compiler files count toward the game's displayed storage use. The current kit
supports Linux x86-64.

Play opens the original Vulkan window in its own native process. The selected
supported Xbox ISO is opened read-only and read in place. Saves and caches use
one game/creator/schema directory; another simultaneous session cannot write to
it. A successful startup remembers the selected ISO and begins play tracking.
An invalid disc does neither. The native process uses a 4 GiB physical-memory
scope; **Memory ceiling** must be at least 4096 MiB. GamePKG's mute and volume
settings apply to the original runtime.

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
and syscall allowlist described below. Its namespace exposes system libraries,
GPU/audio/controller devices, selected desktop sockets, the read-only ISO and
its own writable data directory. It has no network namespace connection or
mount of the player's home/library. Verification and isolation do not establish
complete engine memory safety or full platform/gameplay fidelity. Qualification
details are in [the Rust port notes](../engines/takedawn/rust-port/README.md).

## Protections

For ordinary game source, verification applies the same source and dependency rules before either compiler runs. Games cannot declare Cargo dependencies, build scripts, macros, aliases, indirect calls, unsafe blocks, foreign declarations, or direct filesystem, socket, process, environment, standard I/O or thread APIs. Integer layout and byte-order checks still apply to portable game data. Only the host's pinned engines can supply native runtime code.

Both compilers run in isolated work areas without network or home-directory access. Native compilation invokes the selected Rust toolchain directly, with fixed host-generated bindings and linker arguments. A source package cannot choose compiler flags or supply an executable.

Native gameplay runs in a separate worker; game code is not loaded into the desktop app. Linux namespaces isolate its filesystem, processes and network. The worker also installs a mandatory syscall allowlist before calling game code. It cannot open files, create sockets, execute programs, or create processes. Assets, explicitly selected ROM snapshots, saves, drawing, audio and permitted networking travel through bounded host messages. Internal worker entry points use reserved identifiers rejected in game source.

The worker's address space is capped at the lower of the signed hard memory limit and the user's ceiling, including its native runtime and C engine. Startup has a five-second limit; each tick uses the configured deadline. Stalled or failed workers are terminated, and incomplete tick output and saves are discarded. Host rendering, ROM snapshots and network buffers have separate bounds; the worker limit is not a cap on the entire desktop application or GPU.

Cached executables are checked against their integrity receipt and copied into a private, read-only execution snapshot before launch. Native and optional Wasm caches remain separate. Source checks, OS isolation and host validation work together; source inspection alone is not a security boundary.

## Rendering

The desktop uses Vulkan through the host renderer by default. Game source uses the GamePKG drawing, texture, shader and audio functions; it never receives a Vulkan device or window-system handle. Signed Slang fragment passes are compiled to WGSL or SPIR-V, validated, and rendered through host-owned pipelines with bounded textures.

OpenGL is an explicit alternative in Settings and requires an app restart. `gamepkg-gui --renderer opengl` can open the app on a machine without usable Vulkan. GLSL shader output is supported by the OpenGL renderer; Vulkan shader builds use WGSL or SPIR-V. The default shader output is WGSL.

Play opens a separate process and game window named for the game. The library stays available; closing it does not stop an already running game. Close the game window to end the session and finish saving progress. The game window owns its rendering and audio, while native game code continues to run in its restricted worker process. `gamepkg-gui --play GAME_ID --target native` (or `--target wasm`) starts this player directly.
