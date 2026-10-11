# Security model

GamePKG checks package signatures and resource declarations, compiles supported
source in an isolated Linux process, and runs the result under Wasmtime with
scoped host APIs. A signature establishes package integrity and creator-key
continuity. First installation requires creator approval; it does not identify
a real-world person or guarantee harmless code.

The source policy rejects build scripts, procedural macros, binary includes,
source path overrides, unsafe guest code, and filesystem/network/process
paths. The pinned guest crate allowlist currently contains byteorder-lite 0.1.0
with default features disabled. The signed lockfile must match its approved
graph and registry checksum. The host supplies the source offline. Local
approval can be revoked or restored through `allowlist remove/add` or desktop
Settings; unknown crates cannot be added to the bundled catalog. Source scanning is a policy filter; compilation
and runtime isolation enforce the remaining boundaries.

Linux builds use offline, locked Cargo inside bubblewrap, with an isolated
network, private working directory and Cargo home, read-only system/toolchain
mounts, and no user home directory. Dependencies come from host-owned bundled
source. Compiler processes have memory, file-size and time limits, with bounded
diagnostics. Failed sandbox setup stops the build.

Core and versioned WIT component games receive only scoped asset, input,
drawing, audio, ROM and save operations. WASI and ambient filesystem/socket
imports are unavailable. Wasmtime enforces linear memory, fuel and epoch
deadlines. ROM access uses an explicitly selected read-only session snapshot.
Save writes commit only after successful ticks. Audio permission permits output
without microphone access. Shared assets use signed hashes and private
verified session snapshots.

Optional video previews decode in a separate network-isolated process with
bounded output. Resource-free compiled fragment shaders can render in the
OpenGL player; broader GPU resource support is still under development. GPU
drivers and the host runtime remain trusted components, and GPU work is not
metered by Wasm fuel. These restrictions reduce access and do not establish a
general guarantee that a package cannot cause harm.

Packaged source modules are supported. Every Rust file is scanned, including
unused modules; declarations must resolve unambiguously within the package.
Only checked source files are staged for compilation.

Slang 2026.19 compiles the signed shader source inside the build sandbox.
SPIR-V, WGSL, Metal, and DXIL outputs are tested. DXIL additionally requires
checksum-pinned DXC 1.9.2609 libraries, selected through Settings or
`GAMEPKG_DXC_ROOT`. Missing or modified libraries stop the build. Compilation
support does not imply that every output backend can render in the desktop
player; current shader playback uses resource-free GLSL passes or validated SPIR-V/WGSL
translated to GLSL for the OpenGL player.

Portable `.gamepkgstore` imports verify every contained signature and shared
blob before showing creator identities for approval. Inspection runs no game
code and installs nothing. Rejected library files are shown as unavailable
without hiding other verified games; installation checks remain strict.

RGBA sprite drawing accepts images up to 512×512 pixels, with exact byte counts and at most 8 MiB of queued image bytes per frame. The host validates guest memory ranges, charges fuel, snapshots accepted pixels, and discards queued images after trapped ticks. Component pixel lists are length-checked through lazy guest-memory views before a host copy. Sprites draw through the host; package shaders still cannot access external texture bindings.
