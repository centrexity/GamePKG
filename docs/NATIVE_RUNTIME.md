# Native builds and playback

On Linux, **Build** and **Rebuild** compile the signed game source directly for the current operating system and CPU. They do not also build Wasm or a web player. **Build Wasm**, **Play Wasm**, and **Export for web** are separate actions. The CLI equivalents are `gamepkg build GAME_ID` and `gamepkg build GAME_ID --target wasm`.

Native C-based ports need Clang 18 and lld-18; they do not need wasi-libc or a WASI sysroot. The optional Wasm build of a C-based port does need those libraries. The saved **C/WASI build tools folder** applies to that optional compiler. Games that do not call shader functions do not require Slang for their native build. Shader games require the pinned Slang compiler.

Native executable caches are specific to the package contents, host version, host bindings, operating system, CPU architecture and detected CPU features. Moving source packages between machines remains supported; build them for the destination machine. A native build does not replace a separately built Wasm artifact.

The current native worker implementation requires Linux. The Windows desktop app retains explicit Wasm build/play controls and its configured Linux compiler exchange. It does not silently present a Wasm build as a direct CPU build.

## Protections

Verification applies the same source and dependency rules before either compiler runs. Games cannot declare Cargo dependencies, build scripts, macros, aliases, indirect calls, unsafe blocks, foreign declarations, or direct filesystem, socket, process, environment, standard I/O or thread APIs. Integer layout and byte-order checks still apply to portable game data. Only the host's pinned C engines can supply C code.

Both compilers run in isolated work areas without network or home-directory access. Native compilation invokes the selected Rust toolchain directly, with fixed host-generated bindings and linker arguments. A source package cannot choose compiler flags or supply an executable.

Native gameplay runs in a separate worker; game code is not loaded into the desktop app. Linux namespaces isolate its filesystem, processes and network. The worker also installs a mandatory syscall allowlist before calling game code. It cannot open files, create sockets, execute programs, or create processes. Assets, explicitly selected ROM snapshots, saves, drawing, audio and permitted networking travel through bounded host messages. Internal worker entry points use reserved identifiers rejected in game source.

The worker's address space is capped at the lower of the signed hard memory limit and the user's ceiling, including its native runtime and C engine. Startup has a five-second limit; each tick uses the configured deadline. Stalled or failed workers are terminated, and incomplete tick output and saves are discarded. Host rendering, ROM snapshots and network buffers have separate bounds; the worker limit is not a cap on the entire desktop application or GPU.

Cached executables are checked against their integrity receipt and copied into a private, read-only execution snapshot before launch. Native and optional Wasm caches remain separate. Source checks, OS isolation and host validation work together; source inspection alone is not a security boundary.

## Rendering

The desktop uses Vulkan through the host renderer by default. Game source uses the GamePKG drawing, texture, shader and audio functions; it never receives a Vulkan device or window-system handle. Signed Slang fragment passes are compiled to WGSL or SPIR-V, validated, and rendered through host-owned pipelines with bounded textures.

OpenGL is an explicit alternative in Settings and requires an app restart. `gamepkg-gui --renderer opengl` can open the app on a machine without usable Vulkan. GLSL shader output is supported by the OpenGL renderer; Vulkan shader builds use WGSL or SPIR-V. The default shader output is WGSL.
