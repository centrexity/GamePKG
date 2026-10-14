# Security model

GamePKG checks package signatures and resource declarations, compiles supported
source in an isolated Linux process, and runs the result under Wasmtime with
scoped host APIs. A signature establishes package integrity and creator-key
continuity. First installation requires creator approval; it does not identify
a real-world person or guarantee harmless code.

The source policy rejects build scripts, all guest macros, renamed and wildcard
imports, type aliases, closures, function-pointer types, dynamic traits, computed
call targets, binary includes,
source path overrides, unsafe guest code, and filesystem/network/process
paths. The pinned guest crate allowlist currently contains byteorder-lite 0.1.0
with default features disabled. The signed lockfile must match its approved
graph and registry checksum. The host supplies the source offline. Local
approval can be revoked or restored through `allowlist remove/add` or desktop
Settings; unknown crates cannot be added to the bundled catalog. Source scanning is a policy filter; compilation
and runtime isolation enforce the remaining boundaries. Every game Rust file,
including unused modules, is parsed before compilation. A second pass parses the
exact private build tree, including generated bindings and pinned dependency
source. Parsing is a syntax and policy check, not a proof of harmless behavior.

Guest data follows a fixed wasm32 ABI on 32-bit and 64-bit hosts. Asset, ROM,
and save byte counts use fixed-width `u32` values. Numeric byte conversions must
name a fixed-width primitive and explicit little- or big-endian order; native
endian encoding, pointer access, layout inspection, and architecture-specific
intrinsics are rejected. Memory64 guests are rejected. Index and length values
may use `usize` internally, but serialized numbers require an explicit width.

Virtual paths are exact archive keys, never host paths. They are not URL-decoded,
expanded, or Unicode-normalized. Absolute paths, traversal, empty components,
backslashes, drive/stream colons, control characters, Windows device names,
reserved characters, and trailing dots/spaces are rejected. Paths are bounded
to 1,024 UTF-8 bytes, 32 components, and 255 bytes per component. Archives reject
case aliases and file/directory conflicts. Compiler source filenames must be
ASCII to avoid normalization aliases across filesystems; source text and virtual
asset names may still contain Unicode. Slang file inclusion and module imports
are forbidden, including in unused shader files.

Dependency declarations may only name the bundled pinned crate with its exact
version, disabled default features, and no extra features or source overrides.
Build, development, target-specific, workspace, patch, replace, and feature
sections cannot introduce dependencies. Host settings may enable or revoke
catalog entries, but cannot add an unreviewed crate. A valid creator signature
does not exempt a package from source or dependency checks.

The adversarial suites in `tests/path_attacks.rs`, `tests/dependency_attacks.rs`,
and `tests/source_attacks.rs` cover generated path spellings, signed hostile
packages, dependency-graph mutations, hidden source/configuration, and symlinks.
Native runtime tests call the raw core and component imports with a real verified
package and check that rejected reads preserve output memory. The browser suite
`node tests/web_path_attacks.mjs` directly exercises the shipped worker's core and
component read/image imports. Positive controls confirm ordinary asset reads and
the approved graph remain usable. These are regression checks, not a claim of
exhaustive protection against every possible attack.

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
player; current shader playback uses bounded-texture GLSL passes or validated SPIR-V/WGSL
translated to GLSL for the OpenGL player.

Portable `.gamepkgstore` imports verify every contained signature and shared
blob before showing creator identities for approval. Inspection runs no game
code and installs nothing. Rejected library files are shown as unavailable
without hiding other verified games; installation checks remain strict.

RGBA sprite drawing accepts images up to 512×512 pixels, with exact byte counts and at most 8 MiB of queued image bytes per frame. The host validates guest memory ranges, charges fuel, snapshots accepted pixels, and discards queued images after trapped ticks. Component pixel lists are length-checked through lazy guest-memory views before a host copy. Sprites draw through the host; GLSL shaders may receive one explicitly supplied host-owned texture through the bounded shader-image API.

PNG sprite loading uses bounded verified archive ranges rather than host filesystem paths. Encoded files are limited to 2 MiB, decoded dimensions to 512×512, and decoder allocations to 8 MiB. The host charges archive decompression prefixes and reserves decoding fuel before reading the full file. These renderer budgets are separate from guest linear-memory limits.

GLSL fragment passes may use one host-owned combined `Sampler2D gamepkgTexture` at binding 0 via `draw_shader_image(index, pixel_width, pixel_height, rgba)`. Pixels are snapshotted and fuel-charged before queueing, bounded to 512×512 and included in the shared 8 MiB frame image budget. Additional shader resources are rejected. Core and WIT runtimes support this API. Textured SPIR-V/WGSL playback validates one float 2D image and one non-comparison sampler at group 0 bindings 0 and 1 before translating to GLSL. SPIR-V builds define `GAMEPKG_SEPARATE_SAMPLER=1`; use the example’s explicit texture/sampler branch.
