# Shared services for native ports

Game code uses GamePKG functions for **controllers, keyboard, mouse, touchscreen,
windows, graphics, audio, files and networking**. SDL, Winit, Vulkan, CPAL and OS
APIs belong to audited GamePKG implementations. They are not game dependencies.
Ordinary game packages allow **no Cargo dependencies**; generating bindings must
not relax source checks, permit game-owned unsafe/extern bindings or add crates.

The implementation is `src/port_host/`. The game-facing C interface is
`include/gamepkg_port.h`; dependency-free generated Rust bindings and the native
client are in `engines/native-services/client.rs`. The application embeds both
through `port_host::CLIENT_SOURCE` and `port_host::C_HEADER` for build staging.
These are host-supplied build inputs, not editable package overrides.
Record `port_host::build_identity()` in native build receipts alongside the host
binary/compiler/target identities; it covers the service and generated binding
sources so a changed SDK cannot reuse a stale source kit.

## Native 2D tick games

The Linux x64 maintainer runner links a dependency-free `game_tick(u32, u64)
-> u64` into the same checked native compatibility runtime. The normal source
scan runs before GamePKG supplies the 2D bindings and game-state helper. The
separate game object audit runs before linking. For example:

```sh
cargo run --locked --features gui --example native-tick-game -- /path/to/2048 /path/to/local-test-library
```

Use a signed `.gamepkg` for a stable save identity. The runner signs a source
directory with an ephemeral key for a local test. Add `--smoke` for a hidden
first-frame check. This is a maintainer entry point. The app's standard Build
and Play commands do not select this path yet.

The adapter uses a 640 × 480 logical canvas scaled to fit the game window.
It reads logical keyboard and controller input and uses the monotonic clock
for frame pacing. Tick saves use a private capability and atomic replacement.
The optional asset grant contains copies of the signed package's own assets.
The launcher matches their source-kit identity to the executable receipt and
mounts that snapshot read-only. PNG decoding stays in GamePKG and returns an
opaque image handle.

## User-selected game-data directories

The host can register an already opened directory with `Access::GameData`.
Grant discovery reports kind 6, distinct from package assets (kind 1) and
individual ROMs (kind 3). A path remains relative to that one grant. Open, read,
size, metadata, existence and bounded listing are available. Save operations
require a separate writable grant. Intermediate links, final links, hardlinked
files and special files are rejected. Revoking the directory also revokes its
open file handles. Directory listing examines at most 16,384 entries and
retains the existing bounded response and 4,096-entry limit.

On Linux, a checked native package can request one folder layout in its
signed manifest.

```toml
[game_data]
title = "Original installation"

[[game_data.layouts]]
directories = ["data", "packages"]
paths = ["data/start.cfg"]
min_bytes = 1
```

The player selects the parent folder in the desktop app or with
`gamepkg run GAME_ID --game-data FOLDER`. Only the declared top-level
directories are mounted read-only. Optional startup files must be inside
those directories and pass the declared size checks. The launcher pins
the directories before starting the confined process. The child runtime
supplies them through `Grants::game_data` in manifest order. The game
filters discovery for kind 6 and uses paths relative to each grant. The
game worker receives opaque IDs and no original filesystem handles.

## Process ownership for native ports

Each native port runs as a separate process with its own game-only window.
Its process owns and runs the platform services, using GamePKG's reviewed
implementations for windowing, Vulkan graphics, audio, input, files,
networking, and timing.
The process boundary and the boundary between game source and trusted
GamePKG code are separate decisions.

GamePKG defines the functions game code can call and supplies their
implementations. Each native build receives GamePKG's versioned
compatibility-layer source for its target, including the shared graphics,
audio, input, window, file, networking, and timing implementations.
Maintain this layer in GamePKG so ports use the same functions across
supported platforms.

Check the complete game source before including the bindings. Compile and
optimize game objects separately, audit the exact objects, then link them
with the separately compiled GamePKG layer into the game executable. Keep
the game engine and its platform bridge inside the audited game code.
Record the compatibility-layer version and source identity in the build
receipt. Retain the existing permission, resource, and confinement rules.

Admission allows up to 512 MiB of package source. Verified compiler tools and
their pinned downloads can be up to 2 GiB per file. The finished game
executable has a separate 256 MiB limit.

The launcher verifies and builds the package, establishes its permissions
and resource scope, starts it, and tracks its lifecycle. Runtime service
execution belongs in the game process for every native port.

For a new port, use the compatibility layer above. When reusing an upstream
game engine, include its game code and platform bridge in the audited source.
The old external-engine catalog is read only and exists to verify previously
published signed packages. Its build and launch paths and legacy C host bridge
have been retired. A catalog entry or a copy of a header does not grant build
or play access.

The trusted bootstrap can bind the generated client to the local dispatcher
with `connect_local`. Game bindings still expose the same checked functions.
The native executable runs a trusted service thread for file and network
operations and blocking waits. Its window thread drains the existing bounded
queue and owns platform presentation/input. The game worker sends copied,
bounded calls and retains the same permissions and session revocation checks.
On Linux x86/x86-64 the worker can detach and close its inherited descriptors
before installing the native syscall filter, while trusted providers keep
their descriptors. These worker restrictions supplement the required
confinement of the full executable.

The Linux x64 maintainer command
`cargo run --features gui --example service-window -- --local`
exercises the checkout-backed `preparation::build_in_process`. The builder
hands only sealed game objects to the fixed GamePKG compatibility-layer link.
It embeds the verified manifest and reviewed task table, and the receipt
records the SDK, source kit, compiler and target, exact objects, runtime and
binary hash. `VerifiedPipelines::prepare` checks a private copy of the signed
archive against the admitted game units, compiles the selected vertex and
fragment stages with pinned precise Slang, and combines them with the reviewed
host layout. The native build embeds the resulting payload and records
the Slang preparation identity and ordered, sample-count-specific pipeline
identities. The bootstrap checks those identities before opening the window.

The fixture runs a registered game task, reads a four-byte ROM through an
ordered, read-only grant, and verifies a pixel from the signed shader on
Vulkan before presentation.
A release build pins the hash of its compatibility source bundle at compile
time. It reads that bundle from `share/gamepkg/native-service-sdk.tar.gz`,
verifies a private snapshot, and builds offline with the bundled registry
sources. A cache keyed by the pinned bundle keeps the extracted source at a
stable path. Repeat builds compare the cached files byte-for-byte against the
verified archive under a build lock and reject extra files or links. Cargo
outputs are scoped by toolchain and target. The compatibility layer uses
release optimization with LTO disabled. Game objects retain their own
optimization and audit stage. The source hash contributes to the runtime receipt. A
checkout build without a bundle pin uses its own sources. Qualification on
the actual desktop, the port's reviewed postprocessing and title integration
remain required for production.

A Windows compatibility build can also stage a pinned SDK archive into a
fresh private directory. Its trusted Cargo build uses the verified GNU
Windows Cargo and LLVM-MinGW components selected for the target width. Only
GamePKG source and its locked dependencies enter that Cargo workspace. The
game compiler remains separate, and only sealed game objects enter the link.
The receipt binds the tool identities and the SDK archive pin. Windows preview
packaging carries this matching bundle with the app.

The Linux in-process launcher verifies the executable receipt, copies the
verified binary into a private launch directory and enters a systemd memory
scope using the signed hard limit and no swap. Bubblewrap mounts the required
system libraries, selected local display socket and platform devices. It uses
private user, PID, IPC and UTS namespaces and a private network namespace when
the signed manifest disables networking. It mounts the open ROM descriptors
read-only in declaration order and, with approval, binds the private directory
for the game, creator, save schema and quota. The trusted runtime acquires the existing exclusive save
lock and uses the same flush-and-replace service. The game worker detaches its
file descriptor table before running audited game code. This launcher uses
the selected X11 server's ordinary session authority on X11. The local
maintainer checks use a private Xvfb server, and audible output, physical
controllers, hotplug and Wayland still need desktop qualification.

For a save-enabled package, the launcher also looks for the old scoped
single-blob save using the exact game ID, creator key and save schema.
When available, it supplies a bounded read-only snapshot in grant kind 5
(`GAMEPKG_GRANT_PREVIOUS_SAVE`). The only file in that grant is
`legacy-save.blob`. This grant is separate from the current writable save
directory and the ROM grants. Open it read-only with the ordinary file
API. The game decides whether and how to import its own format. GamePKG
never edits the old blob. Missing, unsafe, or oversized inputs leave the
previous-save grant absent. Both desktop launchers can supply this grant,
and the game checks its availability at runtime.


The Windows process runner can pass up to eight selected ROMs to a trusted
compatibility bootstrap. It duplicates only the selected open files with
read access and supplies their handles in manifest order. The ROMs join the
explicit handle list for the suspended child, which is assigned to a
single-process memory-limited job before it starts. The bootstrap checks the
signed count, verifies that each inherited handle is a distinct disk file,
and clears its inheritance flag. The game-facing ABI receives capability IDs.
The linked Windows executable checks the AppContainer token and current job
limits before accepting those ROMs or opening its window. Normal Windows
Build and Play calls this launcher for offline standalone image-renderer
packages. Synthetic x86 and x64 Wine tests check order, contents, read-only
access and cleared inheritance. The native Windows Doom test also requires
AppContainer, a desktop Vulkan driver and a player-selected WAD.

The launchers record play history for the signed game ID and creator
in the existing library. The trusted player signals readiness after it
successfully presents its first new game frame. Empty startup redraws and
repaints of an existing frame do not advance the frame count. The launcher
uses the scoped signal from the executable to start tracking. A background
worker checkpoints elapsed time and flushes the session when the child
closes its output. The launcher waits for that flush before reporting a
normal terminal status. It drains the worker during process reaping on Stop.

The launcher passes validated graphics options as command-line arguments to
the trusted entry point. The initial window uses the selected aspect ratio and
fullscreen mode. Audio mute and volume stay in the trusted provider. Audited
game code reads the immutable graphics selection through operation 67. Read
current fullscreen and size through the existing window-state call, since
they can change after startup. The fixed local memory layout is reserved
before creating Vulkan providers. The builder calculates the minimum charge
with the same accounting routine as the runtime, including the reviewed
windows, task stacks and bookkeeping reservations.

On Linux x64 the maintainer fixture
`cargo run --features gui --example service-window -- --takedawn-templates`
takes `GAMEPKG_TAKEDAWN_TEMPLATE_ROOT` from the reviewed ports checkout.
The recipes map pipeline index to the cube-slot mask and verify each reviewed
Slang hash against the signed fixture archive before compilation. The native
fixture draws a diffuse triangle through all 16 actual templates and routes
each of their four texture slots through the final combiner. It checks
the center pixels for the diffuse draw and the 2D/cube samples before
presenting. These checks use synthetic vertices and textures.

## Earlier service transport tests

This section describes the earlier proof-of-concept and its test harness.
Use the process ownership requirement above for all native port work.

In the test harness, the launcher constructs one `Session` per confined game
process. That session receives host-created directory grants, opened user-selected ROM files,
the signed manifest permissions and an audited backend. Pass a dedicated private
pipe pair to the generated client bootstrap. `connect` takes ownership of the
pipe files; it is host bootstrap code, not a game API. `serve` processes copied,
bounded messages on the host side. Concurrent game calls are serialized by the
client. Broken/malformed transport permanently revokes the client; no operation
falls back to direct platform access.

The host's event loop must remain responsive: service pipe reads belong on its
I/O worker, then validated requests must be delivered to its owning window
thread with bounded queues/backpressure. `broker::channel` provides that bridge:
the I/O worker runs `Connection::serve`, while the owner calls
`PendingCalls::dispatch` with its per-update request budget. The supplied wake
callback requests a redraw; it must not synchronously reenter the session.
Dropping the owner receiver revokes queued calls. `Desktop` is the concrete owner-thread
backend, not a window-thread dispatcher. Its `paint` consumes one pending frame
and retains it for unsolicited redraws. It adds no GamePKG controls to the game
window. Supply the context of that game's own window, never the library window.

Do not give game code a `Session`, a `Backend`, a device-provider interface,
native window/device pointers, a raw graphics device or capability directories.
The public Rust types are host APIs; only generated safe client functions are
made available to scanned game source. C buffer validity is a client-side FFI
contract; game pointers never enter the separate host process.

`Session::new` is deliberately a host API, not proof a launch is confined.
The launcher must still enforce process permissions on Linux/Windows and deny
direct filesystem/network access independently of this protocol. Source checks,
auditing and renamed functions alone do not constrain native machine code.

## Input and window ownership

`input_backend()` gives trusted providers an instance of the existing
`gamepkg-native-input::Service`, including first-button player assignment,
non-reused device identities, focus suppression, complete analog ranges,
ordered events and bounded overflow/reset behavior. Keep each game's action
mapping and deadzones in game logic; the service does not impose either.

The new `winit_input::Provider` receives events from the **existing game window
loop**. It translates physical keyboard HID usages, committed Unicode/IME text,
absolute/relative mouse motion, five mouse buttons, precise wheel samples and
touchscreen contacts. It preserves full touch identities, normalizes touch
coordinates to 0..1, and cancels contacts even when the final position is invalid.
Focus loss releases state. Alt+Enter returns a host fullscreen request and
consumes only that Enter key; it does not reset held driving/movement keys.
Other window events may only come from the owned game window. Feed relative
`DeviceEvent` samples only while that window is focused. Call `disconnect` on
provider shutdown. Existing SDL/XInput providers remain available for migration;
adapt them to feed this session rather than exposing their APIs to game code.

The host calls `observe_window` with actual focus, size and fullscreen state.
Command acceptance means queued, not an assertion that the OS has already
changed state. `Desktop` implements fullscreen, resize, cursor visibility/lock
and close through the owned viewport. Rumble resolves a logical player's shared
device identity through a host-installed provider callback; unsupported output
returns `Unsupported`, rather than guessing an XInput slot.

## Media

Images are immutable RGBA8 uploads identified by session-scoped opaque IDs.
Presentation validates the complete frame before calling the backend: finite
rectangles, owned image IDs, valid filter modes and host-verified shader indices.
Releasing an image cannot invalidate an already submitted frame. The desktop
backend uses hardware Vulkan, refuses CPU/alternate adapters and respects its
device texture limit. Nearest/linear sampling is available for both ordinary
images and compiled fragment passes. Host shader preparation continues to use
the existing verified Slang compilation/cache; game requests cannot inject shader
source or choose a GPU device. Large uploads are bounded by the 16 MiB message
limit; the image table is bounded to 1,024 entries and 128 MiB, and frames to
4,096 draws.

The original operation 30 is output-only, interleaved PCM16 at 48 kHz, mono or stereo, up to 1,600
frames per request. All sample bytes on the wire are little endian. The desktop
backend uses the existing host audio player with bounded backpressure. It does
not silently drop accepted samples. Microphone capture uses the separate permission-gated API below. `Busy` means retry
later; `Unsupported` means no output/provider was attached.

Native indexed 3D rendering is implemented by `port_host::graphics` and connected
to `Desktop`. Before starting the game, the host calls
`shaders::compile_native_sources` on the signed shader entries, then installs
verified pipeline/layout descriptors with `Desktop::install_graphics`. This uses
the existing pinned Slang worker, checks reflection/stages, validates Naga
resources and emits WGSL for Vulkan. It permits vertex/fragment stages, uniform
binding zero and up to eight 2D/cube texture/sampler pairs; storage, arbitrary
groups and compute stages are refused. Ordinary 2D shader restrictions remain
unchanged. Record these shader hashes in the port's native recipe.
Native preparation fixes Slang `-fp-mode precise`; games cannot select fast
math. Record `shaders::native_preparation_identity` alongside the pipeline
hashes in the final native recipe/receipt. It binds archive/backend, the pinned
Slang executable/library bytes and fixed preparation policy. Pipeline identities
also bind that policy, entry points, layout and state. This does not prove
end-to-end numerical equivalence through Naga/WGSL/driver transformations;
synthetic differential vectors and scene qualification remain required.

Game requests create/update/release opaque vertex, u32 index, uniform and RGBA8
texture resources. Textures support linear/sRGB formats, six cubemap faces, full
mip chains and rectangular uploads. Each indexed frame is fully
validated before submission, including buffer ownership/types, actual indexed
vertex bounds, signed base vertex, scissor bounds, texture dimensions and
sampler modes. Host pipeline descriptors select vertex layout, blending,
culling and depth state. A host-selected 1/2/4/8 sample count controls MSAA with
color resolve; the offscreen color/depth targets stay in GamePKG and the result
is presented in the game's window. Use WebGPU clip coordinates (Z in 0..W).

The host supplies the graphics resource budget. Index/uniform CPU shadows are
charged as well as their GPU storage, and canvas admission checks its attachment
cost. The launcher must account for retained/pending frames and other services
when choosing the total per-game policy. Resources are session-owned; no raw
GPU device, buffer, image or window ID is returned to a game. Engine-specific
features beyond this contract require shared API extensions, never direct
platform fallbacks. Do not move an entire game engine behind `Backend` and
declare its system access trusted.

## Retained attachments and visibility

Operations 106–128 extend ABI 1 without changing existing request layouts.
Targets are session-owned opaque IDs in the same resource namespace. Only
GamePKG owns their RGBA8 resolved color, optional multisample color and retained
Depth32Float attachments by default. The host's installed sample count applies throughout.
Operations 118/119 additionally create and attach independent depth/stencil
resources, so several color targets can share one depth identity. Format 0 is
Depth32Float; 1 is Depth24PlusStencil8; 2 is Depth32FloatStencil8. Format 2
requires DEPTH32FLOAT_STENCIL8 enabled by the host when creating its device.
Unsupported formats/sample counts are refused, never silently downgraded.
New depth is initialized to 1 and stencil to 0. Attaching does not clear it.
Releasing its ID revokes further lookup but attached targets retain the resource
and its memory charge until their references and GPU work are finished.
Each graphics session can hold up to 4,096 GPU resources, including those
awaiting retirement. The 2D image service allows up to 1,024 live image
handles. Both enforce their byte budgets as well as the handle counts.
Color and depth survive separate ordered pass requests. Clear mask bits 1/2
select color/depth; mask zero loads both. Scissored partial clears preserve all
other pixels. A target can occupy a 2D texture slot in a later pass; sampling
the current output is rejected before submission. Use copy for feedback. Copy
snapshots the requested resolved color rectangle before writing, so overlap and
self-copy work, including writing a multisampled destination. Operation 116
also copies a resolved color rectangle into an ordinary 2D/cube texture's
specific mip/layer, preserving every other face/mip. These are color copies,
not depth/stencil copies or depth resolves. Direct texture uploads to a
target are denied. The fixed attachment shaders are host code; games supply no
shader strings. Their bounded format/mask variants are cached, not compiled per draw.
Operation 120 adds independent rectangular stencil clearing (mask bit 4),
including combined color/depth/stencil masks; 108/115 keep their old mask layout
and load stencil. Use a separate clear command at the appropriate query boundary.

Host-verified RasterState descriptors add point/line/triangle lists, front-face
winding, all RGBA write masks, depth bias and front/back stencil compare/ops/masks.
They are part of Pipeline::identity. Game code selects installed variants;
it cannot install shader strings or construct raw pipeline descriptors.
Operation 123 adds per-draw normalized blend constants and an 8-bit stencil
reference. Draw validation rejects incompatible attachment/pipeline depth
formats and counts that do not form the selected primitive list. Point size
remains the standard single-pixel point; larger Xbox point coverage needs a
qualified port-side expansion, not an implied point-size feature.

Owned sampler IDs (122/104) independently select min/mag/mipmap filtering,
clamp/repeat/mirror addressing, finite LOD min/max (0..32, ordered) and anisotropy
1..16. Anisotropy above one requires all three filters linear and host support.
They charge a logical 256 bytes each until GPU retirement; driver internals are
not byte-exact measured allocations. A texture slot's filter word 0x80000000
is followed by sampler ID u64. Other filter words retain the old compact flags.

Operations 124/125 provide bounded asynchronous color readback for title CPU
surface coherence. Start copies the requested rectangle at its ordered GPU
position and returns a ticket; poll is nonblocking and returns tightly packed
RGBA8 rows without the private GPU row padding. At most eight unconsumed
tickets exist; each padded allocation is at most 16 MiB and charged twice for
GPU/mapped-copy capacity. Insufficient reply capacity does not consume a ticket.
Success consumes it; 104 cancels/revokes it without blocking. A copied source
can be released while its ticket remains valid. These are raw stored color
bytes, with no implicit BGRA conversion, guest swizzle or debug dump. Ports
must preserve the title's memory alias/coherence ordering and can split larger
regions into ordered bounded requests.

Presentation copies resolved color to a separate retained image, so subsequent
target mutation or release cannot change an accepted frame. Desktop allows one
pending presentation and retains its displayed frame. Both snapshots are
charged until dropped. Targets charge resolved color, MSAA color and depth;
pending retirement stays charged until queue completion. Admission includes
pending resources in the 1,024-resource bound and permits at most 64 submitted
attachment/frame jobs awaiting completion. Copy scratch memory is charged until
its work completes. GPU allocations internal to the driver are not a byte-exact
host-memory measurement.
Upload staging is also charged and submitted under this bound. A pass may
reference at most 1,048,576 indices in total, so a small repeated draw record
cannot amplify owner-thread validation into unbounded work. Ports can split
larger batches into ordered retained passes without clearing their attachments.
Index and uniform CPU shadows are charged; vertex buffers charge a compact
per-float NaN bitmap used by typed NVI1 templates. This is a per-request work
bound, not a resolution cap, FPS cap or instruction meter.

Visibility is a real occlusion query, not a visibility guess. At most 64 query
IDs exist at once. Operation 115 begins the query before the pass's first draw,
ends it after the last, resolves to a private buffer and maps asynchronously.
Poll returns Busy until mapped; success returns and consumes the actual u64
passing-sample count (MSAA samples, not unique pixels). A query cannot be reused
until consumed; a pending/completed unread query returns Busy on resubmission.
An unused query also polls Busy. Release with operation 104 immediately revokes
the ID; GPU resources and budget survive in-flight work safely. No handle is
recycled. Polling never waits for GPU completion on the window thread.
wgpu's ordinary Occlusion contract guarantees only boolean visibility. The
host's [small pinned HAL patch](../vendor/wgpu-hal/GAMEPKG_PATCH.md) enables
Vulkan occlusionQueryPrecise when supported and emits PRECISE for these queries.
The service checks that this bit was actually enabled, not merely advertised;
query creation returns Unsupported otherwise. Externally supplied raw devices
are unqualified. The hardware regression requires exactly 16,384 samples for a
full 64×64 quad at 4× MSAA, as well as hidden-zero/visible-positive controls.

This does **not** establish NV2A shader/combiner fidelity. Existing Pipeline
bootstrap inputs remain host-verified WGSL, fixed vertex layout, blend/cull/depth
state and bounded texture bindings; game requests select only installed indices.
`Pipeline::identity()` hashes exact programs, entry points, layout, uniform policy
and render state; `Graphics::pipeline_hashes()` preserves index order and includes
the host sample count. [NVI1](NVI1.md) is the coordinated, bounded data representation
for signed vertex/combiner templates. UniformPolicy::Nv2aV1 enforces its exact
layout and validates every referenced block before a frame is submitted. Cache
entries are invalidated by buffer edits. Supplied NaNs in its constants or typed
vertex storage are rejected. Unsupported volume/HDR/dependent-stage-zero cases
fail explicitly. Operation 126 exposes the same structural check independently.
Neither that check nor a pipeline hash implements NV2A semantics or establishes
template fidelity; the signed Slang templates, raw decoder and differential
tests remain port work. Arbitrary runtime GLSL/WGSL/SPIR-V and moving NV2A into
Backend remain prohibited. Existing legacy visuals/defaults have not been changed or
qualified by these service tests.

Operations 127/128 prepare generic render-state variants from an installed
verified template index. Shader modules, entries, vertex/binding layout and uniform
policy remain fixed; only the validated state descriptor changes. Preparation
runs on a host worker, with one active job and at most 256 owned resident/pending
variant IDs. Each charges a logical 1 MiB until both ownership and in-flight use
end. Completed or failed variants stay owned until 104 releases them; cancellation
revokes immediately while the worker retains its own budget/lifetime. Duplicate
canonical requests return the existing ID. Poll is nonblocking: Busy while pending,
Failed on preparation failure, or the exact 32-byte template/state/sample-count
identity when ready. A draw with a pending variant returns Busy before submitting
any pass; there is no substitute, skipped draw or silent state change. Device
and driver internal allocations are still not byte-exact measured memory.
Desktop wakes its viewport on completion; its connect_clock host method can
also wake the same session clock used by channel_with_timing. Callbacks never
reenter Graphics synchronously. The 256-resident policy is an initial bound to
qualify against actual variant usage, not proof the title needs only 256 states.

## Monotonic time and waits

`Session::timing()` returns a host-only cloned Clock. Use the **same** clock with
`broker::channel_with_timing`; the private I/O worker handles operations 60–62
directly. Operation 61 through Session::call/owner dispatch returns Busy without
waiting. The clock starts at session creation, advances without input and never
uses wall-clock time. Wait deadlines are absolute session nanoseconds, at most
one second ahead; past deadlines return immediately. Only one waiter is allowed
per clock. A future generation is Invalid. Spurious condition-variable wakeups
do not end the wait. Changed generations return reason 1 even if the wake
preceded the wait, preventing lost input wakeups. The client serializes calls;
host providers must call Clock::wake after input/window/shutdown changes rather
than depending on a second game request to interrupt its own blocking call.
No separate thread is created for each wait. Host close or Session destruction
revokes the clock and wakes a waiter with Denied. These are pacing primitives,
not a 60 FPS promise or a measured scheduler precision guarantee.
Input sample timestamps retain the existing provider/input-service epoch;
operation 60 starts at Session creation. Use operation 60 consistently for
elapsed input timers and pacing deadlines. Do not subtract an input-event
timestamp from that session-relative clock; no epoch conversion is implicit.

## Files and network

Directory grants are capability directories opened by the host. Asset grants
are read-only. Save grants require signed save permission and must point to a
private per-game directory, not a general user folder or another game's saves.
Relative portable paths reject traversal, drive/UNC/device paths, alternate data
streams, control characters and ambiguous Windows names. Capability-relative
resolution additionally prevents escaping through parent symlinks and rename
races. Final symlinks, special files and hardlink aliases are refused on open.
Revoking a grant closes all handles derived from it. IDs never recycle in a
host process, so stale/cross-session file and image handles cannot alias new ones.

ROM grants use an already-opened user-selected regular file. They expose 64-bit
size/offset reads in place, including offsets beyond 4 GiB, and no write operation.
No extraction or whole-disc memory copy is needed.
Operation 19 opens an independently closeable read-only range over a live
ROM grant. The request contains three little-endian u64 values (grant,
start byte, byte length), and the reply is an opaque u64 file ID. Invalid or
overflowing extents are rejected. The Rust helper is `file_rom_view`.
Reads through the range use relative offsets, stop at its logical EOF and
use the same positional service as other reads. A zero-byte read still
checks the handle. Size reports the declared range length. Closing a range
leaves its parent and peers accessible. Duplicates retain the same range and
revocation root. The 256 opened-file budget includes ranges and duplicates.
Revoking the parent invalidates every derived handle. Range IDs carry no
write or metadata rights and cannot be used as grants to open another range.
The service validates byte bounds. The caller still authenticates and indexes
the disc contents.

Save writes are positional
and bounded to a 256 MiB logical file size; these are normal file writes, not
transactional whole-save replacement. Save grants now enforce aggregate logical-length quota (default 1 GiB, or an
explicit host quota); see the quota and retained-open-file contract below. The
owner still supplies backup policy.

Operation 23 reports the private save grant's configured logical byte quota
and currently available logical bytes. It uses the same accounting as save
writes, including unlinked files still held open. A full entry budget or byte
quota reports zero available bytes. The query discloses no host disk capacity
or ambient filesystem path and reserves no space. Quota scans retain the
existing entry and containment checks. Only a live writable save grant is
accepted.

Operation 24 observes a directory or creates its final component inside a
live private save grant. Mode 0 returns 0 for absent or 1 for an observed
directory. An empty path in mode 0 observes the granted root. Mode 1 returns
1 for an existing directory or 2 for a newly created directory. Creation
requires an existing parent and a nonempty portable relative name. The
operation refuses regular files and checks path components for symlinks. The
result describes one observation and carries no persistent handle or relative
path authority. Both operations run through the existing worker file broker.
Operations 25 and 26 use the identity of an already-opened regular file
in a private save grant. The 48-byte information record contains logical
length, creation/access/write/change times as 100 ns UTC ticks since 1601,
portable attributes, and a zero reserved word. The attribute bits are
read-only (1), hidden (2), system (4), archive (32), or normal (128) alone.
The setter requires a writable handle and replaces the three settable
bits (hidden, system, archive) without taking a path or changing access
rights. Zero clears those three bits. Linux and Android store the bits
on the inode; Windows uses the opened file's native flags. Flushing and
replacing the staged inode preserves its attributes. This operation does
not flush data or publish a staged save. Unsupported timestamps or
attribute storage fail explicitly. The service never substitutes another
timestamp for unavailable creation time.
[Windows file information contract](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_basic_info)

Operation 1 also accepts mode 2 for writable existing files and mode 3 for
exclusive creation. The Rust helpers are `file_open_existing(grant, name)` and
`file_create_new(grant, name)`. Both require a private save grant and return an
opaque readable/writable file ID. Mode 2 never creates or truncates. Mode 3
uses the platform's atomic exclusive create and fails if any entry already
occupies the name. Missing files in mode 2 and name collisions in mode 3 report
Failed, as do other open I/O errors. Callers must not infer a specific cause
from Failed. Both modes enforce the existing path, quota and handle limits.
Legacy modes 0 and 1 retain their behavior. In-place overwrite is separate
from these opens and must not replace the staged save publication protocol.

Operation 14 separately provides flush-and-replace publication. It requires one
private writable save grant for both paths, checks both portable paths, pins the
parent directory capabilities and refuses links, hardlinks and special files.
It syncs the temporary source before atomic rename over the destination; it
never truncates the destination first. Flags 0 requests file flush plus atomic
visibility; flags 1 additionally flushes both parent directories on Unix. On
Windows flags 1 preserves the original saveguard's `MoveFileExW` replacement
with `MOVEFILE_WRITE_THROUGH`, following source flush. This is the documented
[Windows write-through contract](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw),
not a claim of power-loss recovery or Unix directory-fsync equivalence. The
Windows adapter accepts local drive namespaces only, locks every parent against
rename/replacement and compares the pinned parent file IDs before publication.
UNC/unsupported durability is refused before mutation, without a copy fallback.
An ordinary pre-publication I/O failure returns Failed without publishing the
replacement. A detected post-publication durability failure returns
CommittedNotDurable=-6: the new name is already
visible; rollback is not promised and blindly retrying the source name is wrong.
The launcher must exclusively own the private root; the service serializes its
operations but cannot provide transactions against another unconfined same-user
writer. A Windows reader denying delete sharing can prevent replacement; leave
the old save intact and let the original saveguard retry after closing blockers.
Existing handles retain their opened file identity across successful replacement;
close/reopen to address the newly published file.
Operation 17 distinguishes a missing file from an access or I/O failure. It
accepts only a directory grant and portable relative file path. False means
the capability-relative metadata query returned NotFound. A final symlink,
hardlink alias, directory or special file is Denied; other metadata failures
remain errors. ROM/cache IDs and revoked/foreign grants are Denied. Recovery
must never convert an error into permission to discard a pending file.
Operation 18 removes only an empty directory in a writable save grant. It
checks the same portable path and capability containment, rejects final links
and non-directories, and fails on nonempty directories without removing their
contents. It cannot remove the grant root and is never recursive. Port-owned
history cleanup must enumerate its bounded tree explicitly.
Operation 16 duplicates an **opened directory file handle**, preserving its grant,
read/write rights and opened identity while giving it an independent close
lifetime. It consumes one of the same 256 file slots. Grant revocation closes
every duplicate; IDs never recycle. Reply capacity is checked before duplication.
ROM and volatile-cache grant IDs cannot be duplicated or queried through the
metadata operation. No duplication operation can promote read-only access.

Operation 15 returns the opened file's length and last-modified timestamp,
including after rename/unlink. The timestamp is signed floor seconds since
1970-01-01 UTC plus a 0..999,999,999 nanosecond fraction; underlying filesystem
precision is preserved, not improved. Unsupported timestamp facilities/ranges
return Unsupported. These are filesystem times for save recovery ordering, not
monotonic deadlines or a transaction/version guarantee. No physical path, file
identity, owner, or other host metadata is exposed. Burnout's port-side saveguard
still owns pending-save recovery, history and retry policy; these operations do
not implement it in trusted host code.
Games discover only their supplied grants through operation 13. ROM grants
retain the host's manifest selection order; no physical path is returned.

Networking reuses the existing host HTTPS/TCP/UDP service, asynchronous handles,
URL/endpoint validation and bounded queues. Its signed permission remains on/off,
with no added IP allowlist. Disabled networking is rejected before opening a
socket or starting a job. The game process itself still receives no socket API.
Native port operations also support a bound UDP server with explicit peer
records, send-to/receive-from and a local-address query. Insufficient output
capacity or transfer backpressure never consumes a datagram.

## Wire contract, ABI 1

All fields are little endian, with fixed integer widths and IEEE f32 values.
Never cast wire bytes to a native struct, pointer or `usize`. Requests must have
the exact documented size unless they end with a variable payload.

Operation 67 returns window mode (0 windowed, 1 fullscreen), aspect ratio
(0 standard, 1 wide), render scale (1–4), antialiasing (0 off, 1 FXAA, 2 SMAA),
texture LOD (0 off, 1 automatic, 2 −0.5, 3 −1, 4 −1.5), sharpening percent
(0–100), widescreen HUD (0 false, 1 true), and menu/HUD scale (1–4).
Each value is one byte. A value of 255 leaves the individual preference to
the game. Sessions without a native launch snapshot return Unsupported.

Each private-pipe request starts with `GPH1`, operation u32, payload length u32,
reply capacity u32, then payload. The reply is status i32, length u32 and bytes.
Status is zero on success; errors are Invalid=-1, Busy=-2, Failed=-3, Denied=-4,
Unsupported=-5, CommittedNotDurable=-6. Errors have no payload. The C trampoline returns a nonnegative
payload length or that negative status. Messages/capacities above 16 MiB close
the connection before allocation.

| Operation | Request | Successful reply |
| --- | --- | --- |
| 0 version | empty | ABI u32 |
| 1 file open | grant u64, mode u32 (0 read/1 read-write-create/2 read-write-existing/3 create-new), UTF-8 relative path | file ID u64 |
| 2 read | file/ROM ID u64, offset u64, length u32 | bytes, short at EOF |
| 3 write | file ID u64, offset u64, bytes | empty |
| 4 close | file ID u64 | empty; ROM grants are revoked by host only |
| 5 size | file/ROM ID u64 | size u64 |
| 6 list | grant u64, UTF-8 path (empty means root) | repeated kind u32 (1 file/2 directory), name length u32, UTF-8 name |
| 7 create directory | writable grant u64, UTF-8 path | empty |
| 8 remove file | writable grant u64, UTF-8 path | empty |
| 9 resize file | writable file ID u64, size u64 | empty |
| 10 window | command u32, arguments | empty |
| 11 window observation | empty | physical width u32, height u32, flags u32 (fullscreen=1, focus=2, closed=4) |
| 12 sync file | writable file ID u64 | empty |
| 13 grants | empty | repeated ID u64, kind u32 (1 assets/2 saves/3 ROM/4 RAM cache/5 previous save/6 user game data), reserved zero u32 |
| 14 replace save | grant u64, durability u32 (0/1), source length u32, UTF-8 source then destination | empty; -6 means published without establishing the requested durability contract |
| 15 opened-file metadata | handle u64 | length u64, modified seconds i64, modified nanoseconds u32, reserved zero u32 |
| 16 duplicate opened file | handle u64 | new handle u64, same grant/rights, independent close lifetime |
| 17 file presence | directory grant u64, UTF-8 relative path | present u32 (0/1); errors do not mean absence |
| 18 remove empty directory | writable save grant u64, UTF-8 relative path | empty; nonempty directory fails without deleting contents |
| 19 read-only ROM range | ROM grant u64, start u64, length u64 | file ID u64 |
| 23 private save volume | writable save grant u64 | logical quota and available bytes as two u64 |
| 24 observe or create save directory | writable save grant u64, mode u32, UTF-8 relative path | absent, opened or created as u32 values 0, 1 or 2 |
| 25 opened save file info | file ID u64 | length u64, four time u64s, attributes and reserved u32 |
| 26 opened save file attributes | writable file ID u64, attribute bits u32 | empty |
| 20 create image | width u32, height u32, RGBA8 bytes | image ID u64 |
| 21 release image | image ID u64 | empty |
| 22 present | clear RGBA u32 (0xRRGGBBAA), draw count u32, draws | empty |
| 30 audio | channels u32, interleaved LE i16 sample values | empty |
| 40 input events | empty; capacity positive multiple of 40 | repeated 40-byte input events |
| 41 gamepad | logical player u32 (0..3) | 32-byte snapshot; device zero means unclaimed |
| 42 key state | keyboard device u64, physical HID usage u32 | held u32 (0/1) |
| 43 pointer | mouse device u64 | 32-byte snapshot |
| 44 touch contacts | empty | repeated 32-byte contacts |
| 45 rumble | player u32, low u32, high u32 (0..65535), duration ms u32 (0..10000) | empty |
| 46 input devices | empty | repeated device u64, kind u32, logical player i32 (-1 unclaimed); no backend tokens |
| 50 network | existing operation u32, existing request payload | existing service payload without its internal status byte |
| 60 monotonic | empty | session-relative ns u64, wake generation u64 |
| 61 wait until | absolute session ns u64, observed generation u64 | now ns u64, generation u64, reason u32 (0 deadline/1 wake) |
| 62 wake | empty | empty |
| 63 diagnostic | level, code, value count u32; at most 32 values u64 | session sequence u64 |
| 64 diagnostic bytes | level, code, byte count u32; at most 128 KiB exact bytes | session sequence u64 |
| 67 launch settings | empty | eight u8 option values in the order below |
| 100 buffer create | kind u32 (0 vertex/1 index/2 uniform), bytes | GPU resource ID u64 |
| 101 buffer update | resource ID u64, offset u64, bytes | empty |
| 102 texture create | width/height/layers/mips u32, optional format u32 (0 linear/1 sRGB; default 0) | GPU resource ID u64 |
| 103 texture upload | ID u64, mip/layer/x/y/width/height u32, RGBA8 bytes | empty |
| 104 GPU release | GPU resource ID u64 | empty |
| 105 indexed frame | width/height/clear RGBA u32, clear depth f32, draw count u32, indexed draws | empty |
| 106 target create | width/height u32 | target ID u64; initially transparent color, depth 1 |
| 107 target copy | source/destination u64, source x/y, destination x/y, width/height u32 | empty; resolved color rectangle, including overlap/self-copy |
| 108 target pass | target u64, clear mask u32, clear RGBA u32, depth f32, count u32, indexed draws | empty |
| 109 target present | target u64 | empty; bounded immutable snapshot, Desktop only |
| 110 partial clear | target u64, mask u32, x/y/width/height u32, RGBA u32, depth f32 | empty |
| 111 visibility create | empty | query ID u64 |
| 113 visibility poll | query ID u64 | consumed passing-sample count u64; Busy before completion |
| 115 query pass | target u64, query u64, then same mask/clear/depth/count/draws as 108 | empty; brackets the complete pass |
| 116 target-to-texture copy | source target/destination texture u64, source x/y, mip/layer, destination x/y, width/height u32 | empty; specific 2D/cube mip/layer |
| 118 depth create | width/height/format u32 (0 float32, 1 depth24+stencil, 2 float32+stencil) | depth ID u64 |
| 119 attach depth | target/depth IDs u64 | empty; dimensions/sample count must match |
| 120 partial clear with stencil | same as 110, then stencil reference u32; mask additionally permits bit 4 | empty |
| 122 sampler create | min/mag/mip filters u32 (0 nearest/1 linear), U/V/W modes u32 (0 clamp/1 repeat/2 mirror), LOD min/max f32, anisotropy u32 | sampler ID u64 |
| 123 state pass | target u64, query u64 (0 none), mask/clear/depth/count as 108, state draws | empty |
| 124 surface readback start | target u64, x/y/width/height u32 | readback ticket u64 |
| 125 readback poll | ticket u64 | packed RGBA8 bytes; success consumes ticket |
| 126 NVI1 structural check | cube-slot mask u32 (bits 0..3), exactly 13,072 bytes | empty; Invalid or Unsupported on rejection |
| 127 prepare state variant | verified template index u32, 27 state words described below | opaque variant ID u64 |
| 128 variant poll | variant ID u64 | exact template/state/sample SHA-256 bytes (32); Busy/Failed otherwise |

Window commands: 0 fullscreen bool u32; 1 cursor visible bool u32; 2 cursor locked
bool u32; 3 resize physical width/height u32; 4 close (no argument).

Each draw is image ID u64, x/y/width/height f32 in physical pixels, filter u32
(0 nearest/1 linear), shader index u32 (`0xffffffff` for ordinary image).

Native buffers use four-byte aligned lengths/updates; uniform buffers also need
16-byte lengths and are bounded to 64 KiB. Index data is LE u32. Texture layers
are 1 (2D) or 6 (square cubemap), with faces +X/-X/+Y/-Y/+Z/-Z. Mips must fit the
complete dimension chain. Cubemap face/mip uploads are independent and bounded.
All resources require an explicit host-installed native pipeline service.

A standalone source package using `native_services = true` can select the
signed `native_graphics = "vertex-color-v1"` profile. Declare one vertex
and one fragment entry under `[[shaders]]`, with `.slang` sources.
GamePKG compiles both stages with the confined Slang toolchain, validates
the result and installs pipeline 0 with the shared compatibility layer.
Game source calls `buffer_create` and `draw_indexed` or `target_pass`.
The shader and compiler identities are included in the build receipt.

Pipeline 0 accepts a 32-byte vertex with float4 position at location 0
and float4 color at location 1, encoded little-endian. It uses a 64-byte
uniform buffer at group 0 binding 0, u32 triangle indices, one sample,
depth testing with LessEqual and depth writes enabled. The profile has no
texture slots. The native worker issues draws using opaque resource IDs.

The indexed Slang launcher currently runs on Linux x64. The Windows desktop
compatibility path supports standalone image-renderer games. Indexed Slang
still needs the matching Windows pipeline preparation path.

The ordinary shared native launcher passes up to eight selected regular ROM
files read-only and in place, each up to 1 TiB. Kind-3 entries in `grants()`
follow the manifest ROM order. Use those grant IDs and `file_read` with
64-bit offsets. Its signed `limits.hard_memory_bytes` can be up to
4 GiB. The user's memory ceiling must cover that amount before launch,
and Linux enforces the signed cap across the game process and compatibility
layer.

Each indexed draw is pipeline index u32, vertex/index/uniform IDs u64, first
index/count u32, base vertex i32, scissor x/y/width/height u32, texture slot count
u32, then texture ID u64 and sampler flags u32 for each slot. Count must describe
complete triangles. Binding zero is the pipeline's fixed-size uniform block;
slots use texture bindings 1/3/.../15 and samplers 2/4/.../16. Sampler bit 0
selects linear (otherwise nearest); two-bit U/V/W address fields start at bits
1/3/5, each 0 clamp, 1 repeat or 2 mirror. Other bits/values fail validation.
The owned sampler marker 0x80000000 instead consumes a following sampler ID u64.
Each 123 state draw inserts four blend-constant f32 values and stencil reference
u32 immediately after its pipeline index, before the vertex/index/uniform IDs.
Its pipeline index can instead be 0xffffffff followed by ready variant ID u64,
then the same blend/stencil fields. Other draw fields and texture slots follow
the original layout. Blend constants must be finite in [0,1], stencil in 0..255.

Operation 127's 27 state words, after the template index, are:
topology, cull, winding, color mask, depth format, depth-write boolean, depth
compare, signed bias constant (i32 bits), slope-bias f32 bits, clamp-bias f32 bits,
stencil read/write masks, front compare/fail/depth-fail/pass, back
compare/fail/depth-fail/pass, blend-enable boolean, color src/dst/op, alpha
src/dst/op. Topology is 0 points/1 lines/2 triangles; cull 0 none/1 front/2 back;
winding 0 CCW/1 CW; color/stencil masks are bounded to 15/255. Depth formats
match 118. Comparisons 0..7 are Never/Less/Equal/LessEqual/Greater/NotEqual/
GreaterEqual/Always. Stencil operations 0..7 are Keep/Zero/Replace/IncrementClamp/
DecrementClamp/Invert/IncrementWrap/DecrementWrap. Blend factors 0..12 are Zero/
One/Src/OneMinusSrc/SrcAlpha/OneMinusSrcAlpha/Dst/OneMinusDst/DstAlpha/
OneMinusDstAlpha/SrcAlphaSaturated/Constant/OneMinusConstant. Blend operations
0..4 are Add/Subtract/ReverseSubtract/Min/Max. Min/Max require both factors One;
disabled blending requires six zero blend words. Non-stencil formats require
ignored faces (Always/Keep/Keep/Keep) and zero stencil masks. All boolean fields
are exactly 0/1, floats finite, negative-zero bias normalized to positive zero;
unknown fields/values or trailing bytes fail before a worker is started.

Input event fields: device u64, monotonic timestamp ns u64, kind u32, code u32,
x/y f32, contact u64. Kinds, button masks, axis ranges and physical HID conventions
match `include/gamepkg_input.h`; only their fixed-width values are serialized.
Gamepad: device/timestamp u64, buttons u32, LX/LY/RX/RY i16, LT/RT u16.
Pointer: device/timestamp u64, x/y f32, buttons u32, reserved zero u32.
Contact: device/timestamp/contact u64, normalized x/y f32.

Network operations retain the existing host protocol: 0 HTTPS start, 1 poll,
2 TCP connect, 3 UDP connect, 4 send, 5 receive, 6 close, 7 listen, 8 accept,
9 cached HTTPS. Native port extensions: 10 bind UDP (numeric UTF-8 IP:port,
including port zero), 11 receive-from (handle), 12 send-to (handle, 32-byte peer,
payload), 13 local address (handle). Handle prefixes are LE u32; connect/listen take UTF-8 host:port;
HTTPS takes a UTF-8 HTTPS URL. Start/connect/listen/accept and send need at least
four reply bytes, including bind/send-to. Receive-from needs at least 32 bytes
for its peer record, followed by up to 65,536 payload bytes. The peer record
does not count against the payload limit. Address queries return that record.
A peer record is family/port/scope/flow as LE u32 followed by 16 network-order
address bytes. Family is 4 or 6; IPv4 occupies the first four address bytes and
all remaining/scope/flow fields must be zero. IPv6 retains scope/flow identity.
Existing guest bindings keep their original operation range until migrated.
See `src/network.rs` for transport-specific payload details.

Native op 50 sub-op 15 starts an asynchronous TCP connection with an explicit
IPv4 source address. The request has four network-order source address bytes
followed by the destination UTF-8 `host:port`. The OS assigns an ephemeral
source port. DNS and the socket remain GamePKG-owned. This uses the existing
signed networking permission, connection limits, and deadline. The reply is
the ordinary LE u32 connection handle. Sub-op 13 returns its local endpoint
once connected, Busy while it is pending, and Failed if it does not connect.

Native op 50 sub-op 16 accepts an ASCII hostname or numeric IPv4 address
and starts a lookup on the bounded network worker pool. It returns a
LE u32 lookup handle without opening a game socket. Sub-op 17 takes that
handle and returns four network-order IPv4 bytes, Busy while waiting,
or Failed if no IPv4 result is available. The answer remains readable
until sub-op 6 closes the handle. Closing drops the result and revokes
the handle even if the OS resolver is still finishing in its worker.
Networking permission and the existing handle and work budgets apply.

## Migration gates and verification

`admission::CheckedSources::verify` authenticates and scans the complete signed
dependency-free source snapshot before the caller injects CLIENT_SOURCE/C_HEADER.
It refuses blocked native profiles. `admission::AuditedStage::seal` copies finished
relocatable game objects to a private host-owned stage, applies the collective
object audit there, then checks their hashes again. Its receipt records the
policy revision, complete SDK identity, signed source-kit tree identity, exact
compiler/auditor executable hashes, target triple, ordered verified pipeline
hashes and exact staged object hashes/names. `objects()` rechecks the private
inputs immediately before trusted linking. The compiler workers must have
exited and cannot receive this stage as a writable mount. Permission bits alone
are not process isolation. The caller must also pin compiler configuration and
toolchain dependencies, prohibit post-audit LTO/additional game archives and
bind this receipt to the final executable's integrity receipt. These helpers
are **not** a working Takedawn builder, do not consume a new canonical engine
snapshot automatically and do not enable a profile. A receipt's presence alone
never grants executable admission.

Run game source/dependency checks before injecting the audited client. Audit
**all game objects together, before linking host objects**, using
`port_host::audit::objects` and the host's pinned LLVM nm. It permits the broker
trampoline, game-local definitions and a small explicit memory/compiler runtime;
direct SDL, Vulkan, audio-device, OS file/socket and loader imports fail. Do not
pass arbitrary library definitions as game definitions. Object auditing is a
build gate, not proof arbitrary precompiled machine code cannot issue syscalls.
Process confinement remains mandatory. Executable admission in
`src/build_policy.rs` now refuses every unmigrated built-in/native engine
profile, including existing receipts/caches and compiler exchanges. Installed
source packages remain inspectable. A signed manifest, trusted creator,
upstream pin, build target, setting or SDK-looking profile name cannot waive
this rule. No legacy profile is currently admitted for build or play.

Ordinary dependency-free Rust games continue through source scanning before
host bindings are injected and through the existing confined compiler/runtime.
Standard-library access is restricted to explicit value/collection facilities;
direct clocks, backtraces, environment, files, sockets, platform extensions,
macros, aliases, callbacks and FFI are rejected, including qualified-self calls
and nested imports. Native recipe identities include these policy sources.

Each port task must migrate all game code, remove its direct service calls and
use the audited transport before its host admission can be enabled. This needs
an app-side reviewed builder and regression tests; adding a manifest flag or
renaming an engine to a host is insufficient. Compile game units separately
from the trusted runtime, finish game-only optimization before audit, and emit
ordinary relocatable objects (`embed-bitcode=no`). Audit rejects archives,
executables, shared libraries, LTO payloads, startup constructors, linker library
directives, protected external definitions and incomplete tool output. Do not
link additional game libraries or run LTO after that audit. Hash the exact
audited inputs into the build receipt. The private object stage must no longer
be writable by a compiler/game worker before it is linked.

Maintainer checks (no Python or extra library required by players):

```sh
cargo test --offline --features gui --lib port_host::
cargo test --offline --features gui --lib gui::cube2_input::
cargo test --offline --test port_services
cargo test --offline --test port_services -- --ignored
rustc --edition=2021 --test engines/native-services/client.rs -o /tmp/port-client-tests
/tmp/port-client-tests
cargo test --offline --features gui --test port_services_vulkan -- --ignored --nocapture
```

The last test requires hardware Vulkan. It renders offscreen, compares real GPU
pixels for textures/compiled fragment passes, indexed geometry, depth, MSAA and
all six cubemap faces/mips, tests queued viewport commands and retained redraws,
and opens no window/audio device. Native Slang validation additionally has an
ignored compiler test (`shaders::native_raster_tests::native_slang`) requiring
the pinned compiler; it verifies both WGSL and SPIR-V preparation and private
host-include denial. The C integration test uses
synthetic sparse ROM data, real pipes, both C/Rust bindings and positive/negative
object-import checks. Input tests use synthetic host events, not physical-device
delivery. Cross-target metadata builds check client portability; they do not
establish Windows/Android runtime behavior or port gameplay/frame times.

The ignored pipe test executes actual i686 C/Rust code against the 64-bit host.
It requires installed 32-bit Rust std and matching C libraries. Set
`GAMEPKG_TEST_I686_LINKER` to a compatible multilib compiler when the default
compiler lacks those libraries; no global toolchain changes are required.


## Halo extensions and reviewed native launch (2026-11-14)

These additions retain GPH1 ABI 1. Older SDKs return Unsupported.
Infrastructure availability does **not** enable any legacy engine profile.

### Checked build, private transport and game-only window

`preparation::Plan` is host-owned: target, checked root entry function, reviewed
integer task table, fixed-memory layout and optimization choice. It is not a
package-controlled recipe or exemption. `CheckedSources::verify` authenticates
and scans the complete package before generated bindings enter its source.
The compiler produces independently optimized game objects from the checked Rust modules;
there is no Cargo invocation, game library, C bridge or build hook. A private
`AuditedStage` copies and collectively audits all finished game objects. Only
those sealed bytes are linked to separately compiled trusted client/bootstrap,
local runtime and lockdown. Both stages use `lto=off`, `embed-bitcode=no`,
`panic=abort` and an explicit target; native optimization uses
opt-level 3 and the native CPU only for a matching host/target. Bounds traps and
allocation imports use an exact closed compiler-runtime vocabulary, never a
Rust namespace exemption. Their definitions remain reserved.
The i686 COFF audit accepts the MinGW stack probe alias `__alloca` only as
an exact compiler import in an all-i686 COFF object set. The definition
belongs to the reviewed compiler runtime.

`Plan::with_codegen_units(1..=512)` selects a host-reviewed game partition count
(default sixty-four), bound into the Plan identity and compiler arguments. Rustc may
merge small partitions. The builder inventories every retained `.rcgu.o`,
rejects unexpected outputs, links every sealed unit, and accepts an aggregate
copy only when it is byte-identical to the sole retained unit. The trusted
runtime remains a separate compilation using the same reviewed partition count.

`Plan::with_compiler_memory_bytes(bytes)` sets a reviewed **compiler** budget,
separate from manifest/game execution memory. Accepted values are 256 MiB through
48 GiB. Defaults are 24 GiB, or 2 GiB on a 32-bit Windows compiler host. Windows
also rejects values that cannot fit the compiler host's architecture. The Plan
identity and executable receipt bind the exact choice. Packages and creators
cannot select it through their manifest or source. Linux enforces an inherited
per-process address-space limit; Windows enforces the owned compiler job's
committed-memory limit. The large Takedawn maintainer fixture explicitly uses
8 GiB on a 64-bit compiler host after the 4 GiB build failed with allocation
exhaustion. This is not a game memory grant or an admission exemption.

Compiler stderr retains a bounded beginning and end so a warning flood cannot
hide a final failure; Linux errors also report the actual exit status. The
1 MiB per-stream capture limit remains. Machine-readable stdout preserves its
original prefix/truncation behavior; incomplete object-auditor output still
fails. A native compiler invocation can run for up to 180 minutes. Linux
enforces a 12 GiB per-file worker limit while writing, and both platforms check
compiled output sizes before reading them. The sealed object set is capped at
12 GiB in total. An audited ELF object can be up to 12 GiB on a 64-bit host.
COFF input must fit in its 32-bit offset range. On 32-bit hosts the per-object
audit cap is 512 MiB. These are intermediate compiler limits. The signed
game memory limits and the final executable size checks still apply.

Rust/MinGW may attach symbol-hiding `.drectve` metadata to separate units.
Raw directive sections still fail object admission. The builder recognizes
only unique `-exclude-symbols:` entries for valid Rust symbols defined by the
same object (with exactly one i686 ABI underscore added to locate its definition).
Default libraries, exports, startup hooks, unknown directives, malformed ranges,
relocations or overlaps fail. Only the validated metadata is zeroed and renamed
to a non-executable removed `.gpkmeta` section; code, relocations and symbol
records remain byte-identical. The ordinary audit checks the resulting object,
including that this inert section has exact flags and contains only zeros.
`ExecutableReceipt::raw_objects` binds each original compiler object, while
`game.objects` binds each exact audited/linked object. `object_normalization`
records `None` or `RustMingwSymbolExclusionsV1`; absent provenance is rejected.
This closed transformation adds no game import, source or dependency exemption.
Fixed-width SDK decoders use bounded array reads rather than `unwrap` error
formatting, so splitting units does not require formatting-runtime imports.

The final `ExecutableReceipt` binds source-kit, SDK/admission policy, compiler,
auditor, target, toolchain/configuration/CPU, ordered shader/pipeline hashes,
shader-preparation identities, exact audited object hashes, generated runtime
identity and final executable bytes. An `Artifact` cannot be constructed from a
creator receipt or arbitrary executable. `publish` writes provenance and an
integrity record; it does not provide a cache-to-admission shortcut. Rebuilding
is currently required to reconstruct an admitted Artifact. Production port
selection still requires a deliberately reviewed complete kit and pinned recipe.

`Owner::start` requires that Artifact, signed permissions, opened read-only ROM
files, an optional read-only asset capability and an optional `SaveGrant`.
`Process` verifies/copies the executable into private staging and connects only
inherited service pipes. Linux has namespaces, a private temporary root, no
network/display/GPU/audio/input devices, no ROM/save paths, no inherited extra
FDs, resource limits and a child-local syscall filter before game entry.
Windows uses the existing strict inherited-handle list, zero-capability
AppContainer and process/memory job. No legacy direct-display launch is reused.
There are at most eight active processes; stopping kills/revokes immediately and
moves process reaping/private-directory cleanup to a bounded background reaper.

`broker::channel_with_services(Session::worker_services(), wake)` routes file,
flush/replace, network and blocking deadline operations directly to the private
worker. GPU/window/input/audio requests retain the eight-request owner queue.
`Owner::dispatch(budget)` never performs those file/network waits. Revoked
connections cannot return pending I/O data or dispatch new media operations.
An already executing filesystem operation may complete; its exclusive save
lease remains held through worker completion. There is no rollback promise for
an in-flight write or an already published atomic replacement.

`player::run` is the main-thread Linux/Windows game-only window entry. It accepts
an Artifact and exact prepared Pipeline descriptors; ordered pipeline/sample
hashes must match the receipt. It creates hardware Vulkan only, host input,
controller and output-audio providers, and the private child. It has no library
controls in the game viewport. Focus/Alt+Enter, mouse lock/visibility, keyboard,
IME and touch use the owned native window; controller rumble resolves logical
player IDs through its provider and returns Unsupported when unavailable.
Controller normalization preserves full signed axes and unsigned triggers,
with no host game deadzone; physical equivalence/latency remains unqualified.
The window and process close together. Polling is nonblocking; frame presentation
uses FIFO without changing the game's clock or adding a frame watchdog.

Player policy checks the child cap + GPU cap + a conservative 218 MiB reservation
for image (128 MiB), copied transport/scratch (64 MiB), networking (16 MiB),
media (8 MiB) and diagnostics (2 MiB) against the signed hard limit. These are
managed resource ledgers,
not a claim to measure total driver/kernel/GUI RSS. The low-level host APIs must
also be supplied a reviewed budget by their caller. Ordinary manifest limits
remain unchanged; adding native Halo layouts does not bypass them.

### Child-local runtime, separate ABI namespace

`gamepkg_local_call` executes inside the confined child. It never forwards an
arbitrary syscall or a pointer to the broker. Bootstrap alone installs
`local_runtime::Runtime`, fixed windows and reviewed task entry table before
calling checked game code. Games receive opaque IDs and offsets, not native
addresses, handles, Runtime objects or function pointers. At most 32 fixed
windows, 256 reviewed task entries, 32 tasks and 128 synchronization objects.
Window bases/lengths are 64 KiB aligned, nonoverlapping and end at or below
4 GiB; all commit/protect/decommit ranges are 4 KiB aligned. Modes are
0 inaccessible, 1 read-only, 2 read/write; commit accepts 1/2 only. Executable
pages and arbitrary address selection are never available. Transfer copies are
bounded to 1 MiB. The separate opaque-window copy operation moves at most
64 MiB without transferring the payload through the ABI. Mapping access,
copy, protection and destruction share a lock.

The reviewed `LocalLayout::TakedawnMemory` build preset supplies three separate
opaque backing windows, in listing order: main RAM 64 MiB, contiguous RAM
64 MiB, and flash 1 MiB. The existing `TakedawnRam` preset still supplies only
one 64 MiB window. Both work on the four x86/x86_64 Linux/Windows build targets;
they do not require the i686-only fixed guest-address Halo presets. The native
bootstrap reserves these banks at host-selected internal addresses before game
entry. All pages initially remain inaccessible. Local op 1 returns only IDs and
lengths; commit/protect/decommit and copied transfers retain their existing
validation, modes and bounds. All 129 MiB plus per-page metadata are charged
before reservation, even before pages are committed. Task/queue/sync quotas,
the signed hard limit, outer process cap and compiler-only budgets are unchanged.

The port, not this preset, defines guest-address views, repeated main-RAM
mirrors, the tiled alias of the contiguous bank, flash initialization and the
allocator. Aliases reference the same opaque backing ID/offset; no extra native
mapping or authority is needed. The port's existing 32-span ceiling is not
increased. There is no GPU/audio/input register aperture, arbitrary mapping,
executable page, BIOS image, device model or successful unmapped-device fallback
in this preset. A device address is not a window ID. This is CPU-memory backing
only, not qualification of complete Xbox memory/device behavior or title startup.

`Plan` identity includes the chosen preset; SDK identity includes its reviewed
bootstrap implementation. Existing executable receipts therefore require the
current SDK identity rather than silently reusing old artifacts. The preset is
host build metadata, never an option admitted from a game's manifest/request.

`local_memory_copy(source, source_offset, destination, destination_offset, bytes)`
uses local op 7 with an exact 40-byte request and empty reply. It checks both
live window IDs, overflow, complete ranges, committed readable source pages and
committed writable destination pages before any write. Overlap has memmove
semantics, including the same window. Zero length still validates live IDs and
offsets, but touches no pages. Invalid ranges/lengths return Invalid; missing,
revoked or inaccessible resources return Denied. No address or slice escapes
the runtime lock; copying allocates no payload-sized temporary buffer. The
64 MiB ceiling does not enlarge the existing read/write transfer limits. This
child-local CPU memory operation cannot copy GPU or external process memory.

`local_memory_fill(window, offset, bytes, value: u8)` uses local op 8 with an
exact 32-byte request: window ID, offset, byte count and value as four LE u64
words. Wire values above 255, trailing/truncated requests, overflow and lengths
above 64 MiB return Invalid. A valid call returns an empty reply. The runtime
checks the live window ID, full destination range and every committed writable
page before any byte changes. Missing/revoked IDs or inaccessible pages return
Denied; failed validation leaves all bytes unchanged. Filling uses no payload-
sized temporary allocation, exposes no pointer/slice and holds the same lock as
protection, decommit and destruction through the complete fill. Zero length
still checks the live ID and in-window offset but touches no page. Neither
ordinary read/write limits, window budgets, executable-page restrictions nor
file/network/device permissions change. SDK identity includes this operation,
its typed client and exact decoder, so older native receipts must be rebuilt.

`local_memory_read_segments(segments: &[[u64; 3]], output: &mut [u8])` and
`local_memory_write_segments(segments: &[[u64; 3]], input: &[u8])` use local ops
9 and 15. Each descriptor is `[opaque window ID, offset, byte count]`, with
1..32 descriptors, positive lengths and at most 1 MiB aggregate payload.
The wire header is count u32 followed by exactly count descriptors of three
LE u64 words. Read requests end after that header; read capacity must fit the
entire payload and the reply concatenates ranges in descriptor order. Write
requests append exactly the aggregate payload and return an empty reply.
The complete write request, including its `4 + 24 * count` byte header, must
fit the existing 1 MiB ceiling (1,047,804 payload bytes at 32 descriptors).
Typed bindings require exact payload/output lengths; raw read capacity may be
larger. Zero descriptors/lengths, bad sizes, overflow and truncated/trailing
requests are Invalid before access. Unknown/cross-runtime/revoked IDs or
uncommitted/inaccessible pages are Denied.

Every descriptor and every affected page is validated before any output read
or destination write, while holding the same runtime lock as protection,
decommit and teardown. A failed range changes neither RAM nor caller output.
Destination aliases are permitted: later descriptors win in list order. Read
ranges may also alias and concatenate repeated bytes. The copied input/output
cannot alias a mapped native range; no host pointer, callback or slice escapes.
These operations make a bounded transfer atomic with respect to other runtime
memory/protection operations, not a multi-call game transaction. They introduce
no larger read/write, descriptor, window or memory budget. The port must still
validate its own explicit guest mapping, split accesses at actual alias
boundaries, and reject holes/overflow rather than masking arbitrary addresses.
SDK/cache identity includes the new decoder, runtime and generated bindings.

Reserve all windows before game execution. Reservation collisions fail instead
of replacing another mapping. Decommit/recommit yields fresh zero bytes.
Reserved bytes, per-page protection metadata, possible stacks (64 KiB..8 MiB)
and 2 MiB bookkeeping/scratch are charged before reservation. An additional
1 MiB of mailbox payload capacity plus 64 KiB of queue bookkeeping is reserved
up front, even when no mailboxes are created. Atomic copied records additionally
reserve 64 KiB of payload plus 8 KiB of bookkeeping at runtime creation. The generated
bootstrap currently reserves a 1 GiB local ledger with up to 32 one-MiB stacks;
the outer signed process limit must independently accommodate its chosen layout.
Halo's Xbox window is 0x80000000 + 0x20000000 bytes; optional Custom Edition
window is 0x40440000 + 0x01700000 bytes. The latter is an explicit host Plan,
not an arbitrary game address request.

`LocalLayout::TakedawnRam` reserves one 64 MiB window at the host-selected
0x20000000 backing address, on all four native targets. This matches the pinned
kit's physical RAM size, not the total game/process memory ceiling. It is
initially inaccessible and non-executable; the reviewed loader must commit
pages before access. Only its opaque ID and size reach game code. Port-owned
safe code translates guest addresses and explicit RAM mirrors into checked
offsets, rather than forming native pointers. Reservation collisions fail and
the allocation participates in the existing ledger, process ceiling, SDK and
Plan receipt identities. This preset neither enables Burnout nor grants device,
file, network, display or executable-page access.

All fields below are LE, with exact request length and the normal negative
statuses. The local namespace has its own 1 MiB request/reply maximum:

| Local op | Request | Result |
| --- | --- | --- |
| 0 version | empty | u32 1 |
| 1 windows | empty | repeated ID u64, size u64; no base addresses |
| 2 commit / 3 protect | window ID, offset, bytes u64; mode u32 | empty |
| 4 decommit | window ID, offset, bytes u64 | empty |
| 5 read | window ID, offset u64; length u32 | copied bytes |
| 6 write | window ID, offset u64; bytes | empty |
| 7 copy | source ID, source offset, destination ID, destination offset, bytes (five u64) | empty |
| 8 fill | window ID, offset, bytes, value 0..255 (four u64; at most 64 MiB) | empty |
| 9 gather read | count u32; 1..32 (window ID, offset, length) u64 descriptors | concatenated bytes, at most 1 MiB |
| 10 spawn | reviewed entry index u32, argument u64, initially-suspended bool u32 | task ID u64 |
| 11 join | task ID u64 | result u64; Busy until complete; consume once |
| 12 cancel | task ID u64 | cooperative cancellation; empty |
| 13 resume | task ID u64 | prior initial-suspension bool u32 |
| 14 current task | empty | logical task ID u64, cooperative-cancel bool u32; ID 0 is bootstrap |
| 15 scatter write | count u32; 1..32 descriptors, then exact copied payload; whole request <=1 MiB | empty; all ranges checked before mutation |
| 16 task finished | exact task ID u64 | completion snapshot bool u32; non-consuming |
| 17 page states | window ID u64, offset u64, count u32 (1..256), reserved-zero u32 | count copied page-state bytes; no access/protection changes |
| 20 event | manual-reset bool, initially-signaled bool u32 | sync ID u64 |
| 21 recursive mutex | empty | sync ID u64 |
| 22 event set/reset | sync ID u64, signaled bool u32 | empty |
| 23 bounded wait | sync ID u64, milliseconds u32 (0..1000) | empty or Busy on timeout |
| 24 mutex unlock | sync ID u64 | empty; actual owning thread only |
| 25 sync release | sync ID u64 | empty; held mutex returns Busy |
| 26 typed mutex lock | mutex ID u64, milliseconds u32 (0..1000) | empty; events reject without consuming their signal |
| 30 mailbox create | maximum message bytes u32 (1..65536), queue depth u32 (1..32) | mailbox ID u64 |
| 31 mailbox send | mailbox ID u64, copied message bytes (possibly empty) | empty; Busy if full |
| 32 mailbox receive | mailbox ID u64, milliseconds u32 (0..1000) | oldest message bytes; Busy if empty at deadline |
| 33 mailbox release | mailbox ID u64 | empty; drops queued messages and wakes receivers with Denied |
| 34 record create | initial copied bytes (1..4096) | record ID u64; version starts at zero |
| 35 record snapshot | record ID u64 | version u64, complete copied payload |
| 36 record compare/exchange | record ID u64, expected version u64, replacement bytes | next version u64; Busy on stale version |
| 37 record release | record ID u64 | empty; revokes every copied ID |
| 42 memory compare/replace | window ID u64, offset u64, length u32 (1..65536), exact expected bytes then replacement bytes | bool u32 (0 stale/no write, 1 replaced); all destination rights checked first |

`local_task_finished(task: u64) -> Result<bool, i32>` uses local op 16:
exactly eight LE request bytes and at least four reply-capacity bytes. The
reply is u32 0 (pending, including the initial suspended gate) or 1 (finished).
The typed binding requires exactly four bytes and rejects other boolean values.
The snapshot takes the runtime task-registry lock and does not wait for task
completion, resume/cancel a task, consume a result, release its task slot or expose an
OS thread ID/handle. Only local op 11 join consumes a completed task/result.
A finished task stays joinable, including after repeated completion queries.
Missing/cross-runtime/revoked/joined IDs return Denied; malformed/trailing
requests and short reply capacity are Invalid before observation. Closing the
runtime denies further queries. Completion can change immediately after a
snapshot; callers must handle that race without assuming continued execution.
SDK/cache identity includes the decoder/runtime/binding. No quota or native
entry/callback authority changes. This allows copied-message clients to detect
a stopped worker while retaining its exit result for the supervisor.

`local_memory_page_states(window: u64, offset: u64, pages: u32) ->
Result<Vec<u8>, i32>` uses local op 17. The exact 24-byte LE request contains
window ID, offset, page count and a reserved-zero word. Offset must be 4 KiB
aligned and the complete 1..256-page range must fit this runtime's window.
Reply capacity must cover exactly the requested number of state bytes (larger
capacity is allowed). The typed binding requires that exact reply length and
rejects values outside 0..3. State bytes are **0 uncommitted, 1 committed but
inaccessible, 2 read-only, 3 read/write**. These are not protection-mode values;
local op 3 uses 0 no-access, 1 read-only, 2 read/write and still rejects mode 3.
The C header defines the corresponding operation, maximum and state constants.

This is a copied snapshot of private page metadata, under the same mutex as
commit, protect, decommit, reads/writes and runtime shutdown. It reads no page
contents, makes no page readable/writable, allocates no window, and exposes no
host address or handle. Mutating the returned Vec changes no runtime state.
Another caller may change protections immediately after observation; the query
is not a lock, lease, transaction or promise of subsequent access. Callers must
serialize their own setup and still handle errors from every later operation.
A saved snapshot remains ordinary copied bytes after runtime revocation; it
cannot renew a RAM grant. Keep blocking port setup on child worker threads.

Malformed/trailing requests, nonzero reserved words, unaligned/out-of-window/
overflowing ranges, zero/oversized counts and short reply capacity are Invalid.
Missing/cross-runtime/closed IDs are Denied. Failed FFI calls preserve caller
output; aliased request/reply buffers are copied before decoding. Queries of
uncommitted/no-access pages are allowed because they return only their metadata;
actual reads/writes remain denied. Per-call output is at most 256 bytes inside
existing transfer/bookkeeping reservations, with no quota or memory-budget
increase. Older runtimes can return Unsupported; never guess protections or
fall back to native memory queries. SDK/cache identity includes runtime, decoder,
typed binding and C contract. This supports exact private-page restoration for
port import patching and future checked virtual-memory queries; it does not
implement a kernel resolver, arbitrary VirtualQuery or game/device admission.

Atomic records provide bounded shared game state without importing platform
mutexes into game objects. The safe helpers are `local_record_create`,
`local_record_snapshot`, `local_record_compare_exchange` and
`local_record_release`. At most 64 records and 64 KiB of payload exist per
runtime. Payload size is fixed at creation. Snapshots copy the whole payload;
insufficient capacity returns Invalid. Compare/exchange validates the exact
payload size and expected version under the runtime lock, then replaces all
bytes and increments the version atomically. Stale versions return Busy;
version overflow returns Failed. Neither error changes data. These calls do
not wait for a game callback or hold a lock across game execution or broker
requests. Callers must bound their own retries.

All record fields use LE encoding. Create and compare/exchange require at
least eight reply bytes before any effect. Snapshot requires room for the
eight-byte version and the entire payload (at most 4104 bytes). Release takes
exactly eight request bytes and returns an empty reply. IDs are opaque,
runtime-local and never reused. Copying an ID does not retain an object;
release or shutdown revokes every copy, and subsequent calls return Denied.
Records contain copied bytes only, with no pointers, callbacks, OS handles,
device access or durability guarantee. Release immediately reclaims their
payload quota. These are child-local operations, not window-owner waits.

Mailboxes are confined-child services, never broker/window-thread blocking
operations. They hold copied bytes, no pointers, callbacks, task references or
OS endpoints. `local_mailbox_create/send/receive/release` are the safe generated
client helpers. A short receive buffer returns Invalid without consuming the
message. Empty messages are valid; FIFO ordering is per queue, not a transaction
across multiple mailboxes. Sends never wait. Receives check runtime revocation,
mailbox release and the actual calling reviewed task's cooperative cancellation
before consuming a message, and wake on send, cancellation, release or shutdown.
Unreviewed bootstrap callers have no task cancellation flag. IDs are opaque,
process-local and never reused. Foreign runtime IDs and released IDs are Denied.

At most 16 live mailbox objects and 1 MiB of declared `maximum * depth` capacity
exist per runtime. Released objects remain charged until their final in-flight
receiver drops its reference; release cannot evade the budget by racing waits.
Creation fails Busy rather than silently reducing capacity. Mailbox errors do
not acknowledge a save or promise durability. The port must sequence requests,
replies, save publication and bounded worker shutdown itself.

No forced suspension/termination of a running thread, arbitrary callback/APC,
OS priority API or signal handler is provided. Initial suspension is a safe
entry gate. Cancellation before entry returns zero; a running cooperative task
returns its own result. Panic is Failed in the standalone runtime; admitted game
builds use abort. Finished unconsumed jobs retain their quota. Dropping the
owning runtime revokes cloned contexts, wakes waits and prevents further entry;
it does not block the window thread waiting for an uncooperative task. Session
shutdown kills the containing process. Guest TLS, scheduling/APC queues, address
translation and executable-page compatibility decisions belong in the port.

### Independent output streams and microphone

Stream format is 0 PCM16, rate 48000. Layout 0 mono, 1 stereo, 2 explicit 5.1
(FL, FR, FC, LFE, BL, BR). No guessed speaker mapping or silent surround downmix.
A host-declared output layout and distinct valid device-channel map are required
for 5.1; otherwise Unsupported. Stereo remains the default; mono duplicates
into front L/R. Mixing saturates after summation and uses existing volume/mute.
The default desktop mixer reserves 2 MiB; each of at most 16 streams is charged
12 bytes per queued frame + 256 bookkeeping bytes, capacity 1..48000 frames.
Streams start paused. Submission is atomic, up to 1600 complete frames, and
Busy means no prefix was accepted. Underflow emits silence without inventing
consumed frames. IDs are per-session and never reused.

| Remote op | Request | Result |
| --- | --- | --- |
| 130 stream create | format, rate, layout, capacity-frames u32 | ID u64 |
| 131 submit | stream ID u64, interleaved LE PCM16 | empty |
| 132 stream status | ID u64 | submitted/consumed/cancelled/queued u64, running bool/channels u32 (40 bytes) |
| 133 stream control | ID u64, action u32 | 0 pause, 1 start/resume, 2 flush preserving running state, 3 cancel and pause |
| 134 release | ID u64 | discard this stream and revoke its ID |
| 136 gain | ID u64, gain u32 (0..128, default 128) | empty |
| 137 record create | capacity-frames u32 (1..48000) | output recording ID u64 |
| 138 record read | ID u64, max-frames u32 (1..1600) | interleaved stereo LE PCM16, Busy if empty |
| 139 record release | ID u64 | revoke this output recording |
| 140 capture create | format, rate, layout (0/1), capacity-frames u32 | pending capture ID u64 |
| 141 capture read | ID u64, max-frames u32 (1..1600) | interleaved LE PCM16; Busy if pending/empty |
| 142 capture status | ID u64 | captured/read/lost/queued u64, 32 bytes; Busy while starting |
| 143 capture release | ID u64 | discard pending data, revoke even a pending/failed capture |

Output consumed means dequeued into the host mixer, **not physically heard**.
Gain applies when frames are dequeued, including frames already queued. It
does not rewrite the samples or change stream accounting.

Output recording taps this session's mixed 48 kHz output streams after
stream gain and saturation and before device resampling. It captures
silence through underflow and duplicates mono for stereo PCM. Surround
recording is Unsupported. One tap per session shares the mixer budget.
It grants no microphone, system audio, file, or device access. Overflow
discards queued data and fails subsequent reads until the tap is released.
Flush/cancel affects only that stream's queued frames; samples already mixed
into a resampling frame or submitted to the device cannot be recalled. Teardown
revokes all streams. Port-owned decoders must retry Busy in order and release
movie streams on skip/close; no FFmpeg or engine logic is trusted host code.

Microphone requires BOTH signed `permissions.microphone = true` and separate
user approval (`Settings.allow_microphone`, default false) passed by the host to
`Desktop::configure_microphone`. Audio output/networking never grant capture.
At most one device globally; acquisition/startup/driver teardown run off the
window thread. Host input must support the explicit mono/stereo 48 kHz format or
return Unsupported. Default 2 MiB capture budget includes a 1 MiB worker stack,
4096 overhead and queue (4 bytes/frame + 256). Starting another capture while a
previous worker is stopping returns Busy. No microphone was activated by tests.

Capture overflow/device failure explicitly fails and discards pending data;
reads never present a truncated recording as continuous audio. Revocation,
window close, release and disconnect immediately revoke IDs/discard queued data;
the worker stops the device asynchronously (20 ms poll plus provider teardown).
Pending driver setup cannot publish data into a revoked session. Failure to
initialize remains observable through the pending ID until released.

### Viewport and border sampling

Operation **129** has the same target/query/clear/depth/count header and per-draw
state fields as 123. Immediately after each stencil reference, add six f32:
x, y, width, height, minimum-depth, maximum-depth. Then append the original
buffer/index/scissor/texture fields. The viewport is attachment-relative in
physical pixels, finite, positive, wholly contained; both depth bounds are in
[0,1]. Reversed depth bounds are explicitly supported. Scissor remains separate.
Legacy draws retain a full-target 0..1 viewport. Validate the complete pass
before submission; malformed viewports cannot partially draw or clear a target.

Operation **135** extends the 36-byte 122 sampler descriptor with a border-color
u32: 0 transparent black, 1 opaque black, 2 opaque white. Address mode 3 is
ClampToBorder on any of U/V/W. This requires the real Vulkan device feature;
otherwise Unsupported. Other colors/modes are rejected. Arbitrary Halo ARGB
border colors are **not** implemented or approximated by clamp-to-edge. They
require a precise port-side template transformation with filter/mip/cube edge
differential tests, or a reviewed generic extension before enabling affected
content. Do not silently discard that state.

RGBA8 color, existing depth/stencil formats, MSAA, cube/mip, readback, copy and
synchronization contracts above remain unchanged. Other renderer formats,
depth resolves, volume textures and coherence patterns need an actual port use
inventory and exact qualification; NVI1 validation is not an execution proof.

Both pinned Slang backends now complete conversion into validated WGSL. The
reviewed Naga patch implements f32 scalar/vector NaN/Inf classification using
integer bit masks, then reparses/validates the writer output. Precise FP stays
fixed; no approximate comparison or backend fallback is used. These synthetic
checks do not qualify Halo NV2A translation, SMAA, sharpening or whole scenes.

### Persistent save quota and interface discovery

`SaveGrant::open` uses a private title/creator/schema namespace, nofollow directory
capabilities, a sibling host-only exclusive lock and a file quota. On Unix new
scope directories are 0700 and the lock is 0600. `grant_save_quota` accepts
0..1 TiB; ordinary save directory grants default to 1 GiB. Growth is checked
before mutation against the complete private tree plus unlinked/replaced open
files, counting each inode once. The walk rejects links, hardlinks and special
files, and is bounded to 100,000 entries. It runs on the I/O worker. This is
logical file-length accounting, not physical block allocation measurement.
Quota exhaustion returns Busy. Legacy blobs remain in their original
namespace and may be exposed as a separate read-only snapshot. Atomic
flush-and-replace status -6 semantics remain unchanged.

A save-enabled native compatibility manifest may set `hard_save_bytes` in
`[limits]`. The Linux Build and Play path uses that aggregate logical byte
allowance. Accepted values range from 1 byte through 64 MiB. The quota includes
staged files during replacement. Manifests without the setting retain 1 MiB
for native service games and 2 MiB for native tick games. Wasm save limits and
the separately verified read-only previous-save snapshot retain their own
bounds. `recommended_storage_bytes` remains an estimate of total storage.

Disposable map caches must **not** be granted through persistent saves. Keep a
Windows in-process cache inside the child memory budget; a Linux external RAM
cache uses the separately charged volatile capability described below. Do not
route it through SaveGrant.

Networking remains signed on/off. Remote op50, sub-op **14** with no arguments
returns up to 256 IPv4 records: interface index u32, flags u32, ordinary 32-byte
socket address with port zero. Flags: 1 up, 2 running, 4 loopback, 8 broadcast,
16 multicast. Results are sorted/deduplicated snapshots, not stable handles.
Linux uses bounded getifaddrs/if_nametoindex; Windows uses a 1 MiB bounded
GetAdaptersAddresses snapshot, marks Ethernet/Wi-Fi broadcast and operational
interfaces up/running. No adapter name, MAC, device pointer or game OS call.
Insufficient reply capacity/malformed requests fail without a partial snapshot.
Games can bind existing UDP to a discovered address or use connected UDP plus
local-address query to discover a selected route without sending a packet.

OpenCE's SSDP/UPnP XML/HTTP and mapping renewal/removal remain game logic. Existing
bound UDP, send-to/receive-from and TCP can carry discovery and router HTTP; the
HTTPS helper deliberately does not downgrade to HTTP. Raw sockets, interface
configuration, multicast membership/TTL control and arbitrary socket options are
not exposed. Pinned p2p buffer-size requests and any required multicast receive
behavior need explicit adaptation/qualification before full multiplayer claims.
No internet multiplayer or router mapping was tested by the interface fixture.


`Owner::Grants.volatile_cache_bytes` optionally supplies **one Linux-only**
disposable RAM file, 1 byte..1 GiB, enumerated as grant kind **4**. The host uses
memfd with fixed length and irreversible grow/shrink seals; there is no directory
or persistent filename. Existing file read/write/size operations use its opaque
ID with checked 64-bit offsets and the normal 16 MiB message bound. Out-of-range
RAM reads/writes are Invalid. Resize/open/rename/list/save operations cannot
reinterpret it as a directory or persistent save. File-close or host revocation
releases the grant; final session/worker teardown closes the backing object.
A replacement session receives fresh zeroed memory and a new ID. No disk fallback.
Linux memfd is RAM-backed and may be swapped by OS policy; it is not a persistent
cache on disk. The full capacity is charged to both Owner and player launch
checks in addition to the child process cap, not just pages touched so far.
On Windows the grant is Unsupported; keep the port's in-process cache inside
its already charged child/runtime memory budget. Bootstrap code cannot use this
generic capability as permission to expose a shared-memory pointer to game code.

Interface snapshots share the eight-per-second network acquisition budget and return Busy when exhausted. This prevents repeated discovery from creating unbounded host work.


The pinned Halo inspection confirms GL_RGBA16F HDR back buffers/tone mapping,
3D texture uploads, depth+stencil MSAA blits and sampler LOD bias. These are
remaining host/template contracts, not features covered by existing RGBA8,
D2/cube and color-copy APIs.
Do not enable the affected renderer paths by silently substituting formats,
slices, depth samples or bias values.


The checked game frontend exposes `local_task_status()` for cooperative
cancellation. A running task can observe its own numeric identity/cancel flag;
no TaskContext, native thread ID or callback pointer enters game source. The
runtime resolves only its bounded reviewed jobs against the current thread.

Windows auditors install `llvm-nm.exe` with the matching official Rust component's
`libgcc_s_seh-1.dll` (x64) or `libgcc_s_dw2-1.dll` (i686), and
`libwinpthread-1.dll`, side by side. Both archives and the complete curated
payload inventory are pinned and verified. Only those exact executable/DLL
paths and notices are extracted; other compiler/tools are not installed into
the auditor directory. The cache/receipt binds DLL identities as well as nm.
Neither PATH-provided DLLs nor an unrelated MinGW install are prerequisites.


The i686 Halo layout requires a large-address-aware PE and user address space
above 2 GiB. It cannot fit a default 2 GiB-only 32-bit Windows process. Actual
32-bit GNU PE fixtures carry LAA and pass fixed-window tests under isolated Wine;
this is not real Windows operating-system or Halo gameplay qualification.

## Bounded session diagnostics

The generated client exposes `diagnostic(level: u32, code: u32, values: &[u64])
-> Result<u64, i32>`, using op 63. All fields are LE: level/code/count (12 bytes),
then exactly `count * 8` bytes. Levels 0/1/2/3 mean debug/information/warning/error;
code is game-defined and session-scoped. Count is 0..32. The reply requires
8 bytes of capacity and contains the new sequence, starting at 1. Bad severity,
length, count or reply capacity returns Invalid before any event is recorded.
No format strings, paths, console descriptors or native logger are exposed.

The additive ABI 1 op 64 exposes `diagnostic_bytes(level: u32, code: u32,
bytes: &[u8]) -> Result<u64, i32>`. Its LE header is level/code/byte count
(12 bytes), followed by exactly that many bytes, at most 128 KiB. The reply is
the same 8-byte sequence. Empty payloads are valid byte events. Data is copied
and retained exactly, including NUL, non-UTF-8 and control bytes; it is never
interpreted as a format string, command or console escape sequence. Malformed
requests, invalid levels, excessive lengths and insufficient reply capacity
fail before mutation. Closed clocks deny recording. Older SDKs may return
Unsupported; games must propagate the error, not use direct console I/O.

Each session owns at most 128 copied events and 1 MiB aggregate payload, plus
fixed metadata. Numeric values cost eight bytes each; byte events cost their
exact length. Both types share ordering, sequence numbers and eviction policy.
The player charges a conservative 2 MiB reservation for the retained payload, incoming copy,
metadata and escaped summaries against the signed hard limit. Drained event
data belongs to the host caller, which must bound its own retention.
An accepted event evicts oldest records until both limits fit and increments
a saturating eviction counter. Sequence overflow returns Failed before mutation;
it never wraps/reuses a sequence. Timestamps are host session-monotonic ns.
The host-only `Session::take_diagnostics()` drains events in order and returns
then resets the eviction count since the previous drain. No game operation can
read host logs or another session's records. Closing the session clock denies
new events; Session destruction drops the ring. No filesystem/network/device
permission or persistent log is granted by these diagnostic services.

`Session::diagnostic_summary()` formats the retained values as bounded, host-
generated decimal/hex text without draining them. Byte summaries escape every
control/non-ASCII byte and show at most the first 256 input bytes per event,
with an explicit retained-byte remainder count. The full payload stays available
to the host drain. Events expose `bytes: Option<Vec<u8>>`: None for numeric
events; Some (including empty) for byte events. Draining resets payload charges
and the eviction counter, but never resets sequences. Existing `Owner::diagnostics()`
error reporting includes that summary alongside bounded runtime diagnostics.
There are no game-controlled format strings, terminal escape sequences or
unbounded output buffers. Rendering the error summary performs no file or
console I/O. Event admission does not perform a blocking wait or backend call
on the window owner thread. Normal private-pipe queue/backpressure/revocation
rules still apply; games must propagate transport errors rather than falling
back to direct stdout/stderr. The service and both bindings participate in SDK
identity and cache receipts; it does not exempt any game source or object.

## Caller-owned secure clearing

The generated Rust SDK exposes `zeroize(bytes: &mut [u8]) -> Result<(), i32>`.
It clears exactly that exclusively borrowed slice using volatile byte stores
followed by a sequentially consistent compiler fence. Lengths 0..1 MiB are
accepted; a larger slice returns Invalid (-1) before any write. Empty slices are
valid. There is no allocation, transport frame/opcode, handle, pointer exposure,
lock, permission change, host process/window call or OS access. The bounded loop
runs inside the confined child on its calling thread. Safe game code passes
ordinary owned/borrowed bytes; direct volatile/pointer operations remain banned.

This preserves explicit clearing of a port's temporary key buffers when their
last use would otherwise let optimization remove the writes. It covers the
specified slice only, not other copies, registers, swap, dumps or prior
allocations. It does not clear the caller's read-only ROM/key source or grant
access to any other memory. A copied guest buffer needs its own checked write
if the game intends to clear that separate guest allocation. The helper is not
a host crypto implementation and does not move game algorithms into trusted
code. It changes SDK/cache identity while retaining ABI 1, existing object-import
allowlists, quotas and source admission before binding injection.

`tools/check_zeroize_codegen.rs` compiles a dead local key buffer at optimization
level 3 for Linux32/64 and Windows32/64 and verifies that all 64 volatile zero
stores survive. The local-client tests independently verify the 1 MiB boundary,
slice canaries, oversized rejection before writes and empty input. Codegen and
Wine checks do not establish gameplay or real Windows confinement.

## Raw performance clock and anchored calendar

ABI 1 op **65** (`GAMEPKG_PORT_CLOCK_SAMPLE`) accepts an empty request and needs
at least 32 reply bytes. The exact 32-byte reply contains four LE u64 fields:

| Offset | Field | Units |
| --- | --- | --- |
| 0 | counter | Raw host performance-counter counts, 0..i64::MAX |
| 8 | frequency | Counts per second, 1..i64::MAX |
| 16 | uptime_ms | Host boot/monotonic milliseconds |
| 24 | utc_100ns | 100 ns intervals since 1601-01-01 UTC |

The safe Rust SDK `clock_sample() -> Result<ClockSample, i32>` exposes those four
named fields. It rejects short/oversized replies, zero/out-of-range frequency or
out-of-range counter as Failed (-3). A malformed request or insufficient reply
capacity is Invalid (-1) before acquiring a calendar anchor or reading clocks.
The operation is serviced by `broker::channel_with_timing` / `channel_with_services`
on the private connection worker. Direct window-owner dispatch returns Busy
(-2); it performs no platform clock read. Closing the session denies new worker
samples (-4). Accepted in-flight reads may complete. An older host without the
operation may return Unsupported; never substitute direct OS calls or a guessed
frequency. No new resource handle, permission, allocation quota or wait limit is
introduced.

ABI 1 op **66** (`GAMEPKG_PORT_PERFORMANCE_SAMPLE`) is the non-calendar variant:
empty request, capacity at least 24, exactly three LE u64 fields at offsets
0/8/16 (`counter`, `frequency`, `uptime_ms`), with the same units and valid ranges
as op 65. `performance_sample() -> Result<PerformanceSample, i32>` validates
that exact reply shape and ranges. It uses the same worker-only routing, error,
session revocation and unsupported-platform contract. It never reads wall time,
initializes the calendar anchor or advances the calendar's last-sample state.
Repeated op 66 calls before/after op 65 leave calendar state unchanged. The first
successful op 65 still establishes its own anchor. Use op 66 for original
performance-query paths that must not start an absolute-calendar origin early.
Malformed requests fail before reading clocks; short/oversized replies and
invalid counter/frequency values fail in the SDK without any OS fallback.

The audited host source uses `CLOCK_MONOTONIC` nanoseconds with frequency 1 GHz
on Unix; uptime is the same count truncated to milliseconds. On Windows it reads
`QueryPerformanceCounter`, `QueryPerformanceFrequency` and `GetTickCount64` with
their original raw units. Games receive copied numbers, never platform handles,
pointers, environment data or permission to invoke those functions. Other host
platforms return Unsupported. Android and additional architectures have not been
qualified by the executed x86 tests.

UTC is sampled once on the first successful calendar request, using the host's
available SystemTime precision, then advanced by raw counter deltas. It does not
follow later wall-clock jumps. Each result is calculated from the original
anchor with wide checked arithmetic, so rounding does not accumulate and long
uptimes do not overflow an intermediate count*10^7 product. Counter regression,
frequency changes and unrepresentable calendar arithmetic fail without changing
the prior anchor. UTC before 1601 is not fabricated. Pure conversion tests cover
sub-100ns rounding independently of Windows SystemTime's 100ns granularity.
This is an anchored calendar, not a continuously corrected wall-clock service.

**These counts are not op 60's session nanoseconds.** Do not pass a raw counter,
boot uptime or absolute UTC value as an op 61 deadline. Use op 60 and its shared
wake generation for session deadlines; convert relative counter differences
using the returned frequency. Existing one-waiter/one-second bounds, lost-wake
behavior and revocation are unchanged. No rendering, GUI or blocking wait runs
on the window owner as a side effect of sampling.

`src/port_host/timing/platform.rs` participates explicitly in SDK/cache identity
alongside the timing module, client and C header. Tests cover worker-only wire
routing with no owner dispatch/wake, malformed frames/replies, revocation,
epoch/rounding/overflow/regression, real platform reads on Linux32/64 and under
Wine on Windows32/64, and separately checked/audited game clients on four targets
with actual confined Linux execution. These are clock/service tests, not gameplay,
physical input/audio, real Windows AppContainer or frame-pacing qualification.

## Independent child-local performance clock

`gamepkg::local_performance_sample() -> Result<PerformanceSample, i32>` uses
**local operation 38**, not broker operation 66. Empty request, capacity >=24,
exactly 24 reply bytes: LE u64 counter/frequency/uptime_ms at offsets 0/8/16.
The units and SDK validation match operation 66. This never reads wall time,
initializes the calendar anchor, waits, dispatches a window call, or acquires the
remote client connection mutex. A timer worker can therefore sample while the
main thread awaits an owner-dispatched window request. It does not replace the
session deadline/wakeup clock, or provide an arbitrary OS-call selector.

Only the audited, separately linked child runtime reads the platform clock.
`performance_clock.rs` is included in SDK identity and staged after the complete
game-source scan; game objects still import only the approved local trampoline.
On qualified Linux GNU x86/i686 and x86_64 it uses CLOCK_MONOTONIC with the native
C-long timespec ABI, 1 GHz counts and truncated milliseconds. Windows uses
QPC/QPF and GetTickCount64. Other child targets return Unsupported (-5), rather
than assume a timespec layout. The broker retains its existing libc sampler on
other Unix targets. These additional targets have not been runtime-qualified.

Malformed requests/short capacity return Invalid (-1) before sampling; unavailable
bootstrap, closed runtime, or a cancelled reviewed task return Denied (-4).
Platform failures return Failed (-3); no guessed frequency or remote/OS fallback
is attempted. An accepted read may finish concurrently with cancellation/closure.
The clock allocates no handles and changes no quotas. Runtime shutdown, including
disconnect teardown by the trusted launcher, remains required; remote connection
loss alone is not a separate local-clock revocation mechanism.

Local tests cover exact ABI/ranges, untouched output on rejection, cancellation,
closed runtime, and sampling while a real owner request holds the client mutex
(Linux). Linux x64/i686 execute directly; Windows x64/i686 execute under Wine,
which does not qualify actual Windows AppContainer behavior or gameplay.

## Independent child-local deadline and wakeup lane

The trusted child runtime owns one timing lane, separate from the remote client
mutex and the session-nanosecond clock. Safe Rust entry points are
`local_clock_state() -> Result<LocalClockState, i32>`,
`local_clock_wait(deadline, frequency, generation) -> Result<LocalClockWait, i32>`
and `local_clock_wake() -> Result<u64, i32>`. `LocalClockState` contains a
`PerformanceSample` and u64 generation; `LocalClockWait` adds u32 reason.

| Local operation | Exact request | Successful reply |
| --- | --- | --- |
| 39 state | empty | 32 bytes: counter/frequency/uptime_ms/generation, four LE u64 |
| 40 wait | 24 bytes: absolute raw-counter deadline/frequency/observed generation, three LE u64 | 36 bytes: state followed by LE u32 reason (0 deadline, 1 wake) |
| 41 wake | empty | 8 bytes: new nonzero LE u64 generation |

Capacities must accommodate the complete reply; malformed requests and short
capacities reject before sampling, waiting or advancing generation. ABI version 1
is unchanged; these operation numbers are local, not broker operations 39–41.
No pointer, native handle, callback, timer name or OS selector crosses the ABI.

Use the counter and frequency from operation 39. Deadlines are absolute in that
raw counter domain, **not** session nanoseconds, UTC, boot milliseconds or guest
TSC. Counter/deadline/frequency are unsigned u64 values limited to i64::MAX
(counter/deadline may be zero). Frequency must be nonzero and equal to the
platform sample. At most one second ahead is accepted. A past deadline returns
immediately, except that an already changed generation takes precedence and
returns reason 1. A future generation is Invalid (-1). Generation starts at zero,
increments for each accepted wake, and never wraps; exhaustion is Failed (-3).
A wake before a wait is retained by the generation comparison. Spurious wakes
are rechecked. A deadline result is never intentionally early: remaining counts
convert to nanoseconds with u128 ceiling arithmetic, and the clock is resampled
after every native wake. Late scheduling remains possible; this is not a frame
rate or maximum-latency guarantee.

Capture the observed generation **before inspecting the work queue**, then wait
using that observation after deciding there is no work. A later state sample may
refresh the counter/frequency, but must not overwrite that observation before
the wait: doing so would consume a notification without rechecking its work.
On return, retain the returned generation and inspect the queue again. Several
notifications can coalesce into one changed-generation result; the queue remains
the source of work, not the difference between generation values.

There is one blocking waiter per runtime, including its reviewed worker tasks.
A second waiter returns Busy (-2), even for a past deadline. State and wake calls
remain available while it waits. Runtime cancellation wakes the cancelled task
with Denied (-4) without incrementing generation; other tasks may continue their
wait. Shutdown revokes the lane and wakes it with Denied. A failed native signal
returns Failed; a generation already published before that failure is retained.
An accepted operation may finish concurrently with cancellation or shutdown.
Wait slots are released on ordinary errors as well as success. The trusted
runtime supports an explicit worker/owner-dispatch distinction: owner-dispatch
waits return Busy. The local trampoline runs only in the confined child; never
call blocking Runtime waits from the app's window owner thread.

Platform failures, counter regression or a changing frequency are Failed (-3).
State/wait sampling on unsupported clock targets returns Unsupported (-5), as
for local operation 38; wake only advances the runtime's generation.
Linux uses the monotonic condition-variable wait. Windows lazily allocates one
unnamed, noninherited high-resolution waitable timer and one private wake event.
Unavailable high-resolution timer support returns Unsupported, denied creation
returns Denied, and exhausted native resources return Busy. There is no Sleep,
period-resolution change, spin loop or remote-clock fallback. Microsoft documents
the high-resolution timer flag for Windows 10 version 1803 and later:
[CreateWaitableTimerExW](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-createwaitabletimerexw).
The waiter retains both objects through cancellation/shutdown; only after the
native wait ends can its final owners close them. Event resets and wake publication
share the lane mutex. Only the single waiter arms/cancels the timer; the timer is
cancelled after each native wait, including a failed wait. An unexpected native
wait result is Failed, with a 1,100 ms guard for each <=1-second timer arm.

The lane and single Windows object pair fit the existing 2 MiB child-runtime
bookkeeping/scratch reservation. No task, memory, record, transfer or host-budget
limit is increased. `local_timing.rs` and `local_timing_windows.rs` are staged only
after full source admission and participate in SDK/cache identity. Game objects
still undergo their separate optimization and import audit before trusted linking.
The lane has runtime lifetime; it neither needs a game-owned resource ID nor
retains a game closure. Disconnect teardown must still close the runtime.

Qualification includes exact ABI/decoder rejection, ceiling arithmetic,
regression/frequency checks, generation exhaustion, pre-wait wakes, one-waiter
contention, spurious signals, cancellation/reuse, teardown and native-object
retention. Linux x64/i686 and Windows x64/i686 component execution pass; Windows
execution here is Wine only. A real blocked Unix owner request holds the remote
client mutex while all three local operations complete. The source-checked fixture
`tests/fixtures/service_local_deadline.rs` builds/audits all four targets and runs
inside actual confined Linux x64/i686 children before the parent services any
owner request. Run its ignored `service_preparation` test explicitly. None of
these checks establishes hardware wake precision, physical controls, audible
audio, production timer/DPC scheduling or playable-title frame pacing.


## Typed child-local mutex acquisition

`local_mutex_lock(mutex: u64, milliseconds: u32) -> Result<(), i32>` uses local
operation **26**, ABI 1. Its request is exactly 12 bytes: LE u64 synchronization
ID followed by LE u32 timeout, 0..1000 ms. Success has an empty reply; no output
buffer is written. Short/trailing requests or excessive timeouts are Invalid
(-1) before acquisition. Missing, stale, foreign-runtime or closed IDs are
Denied (-4). Only an actual recursive mutex is accepted: both manual and auto
reset events return Invalid without consuming or changing their signal. The
kind check and acquisition use the same retained synchronization object.

Ownership is the actual runtime thread, never a game-supplied owner number.
Recursive acquisition increments checked depth; each acquisition needs local
operation 24 unlock on that same thread. Another thread's zero-time acquisition
returns Busy (-2), and bounded waits return Busy at their deadline. Operation 25
cannot release a held mutex. This does not abandon or unlock a mutex when its
owner is cancelled. Port cleanup must still release each acquired depth.

Operation 26 checks reviewed-task cancellation before acquisition and while
waiting. Cancellation or runtime closure wakes the waiter with Denied; it does
not acquire the held mutex. Unlock remains available for cleanup by its owner.
Existing generic operation 23 still accepts events and mutexes with its previous
semantics; it does not add operation 26's cancellation check. Use typed acquisition
for record-backed shared structures that require a mutex. No new resource kind,
quota, OS handle, callback, native pointer or import exemption is introduced.
The child-local blocking call must stay off the app window owner thread; direct
Runtime owner dispatch returns Busy. Existing sync quotas and one-second bound
remain unchanged. These SDK sources participate in cache identity.

Checks cover exact wire shape, untouched output, wrong kinds without signal
consumption, recursive ownership, wrong-thread unlock, held-release refusal,
stale/foreign IDs, cancellation and closure. Linux x64/i686 run directly;
Windows x64/i686 component execution here uses Wine. This is not real Windows
confinement or gameplay qualification.

## Linux compiler scheduling for large checked sources

The Linux shared-service builder now supplies one private jobserver lane to
rustc. This limits concurrent optimizer work, rather than reducing optimization
or raising the compiler memory/deadline limits. Codegen partitions remain
separately optimized with LTO/embedded bitcode off, and **every** resulting object
still passes import auditing before trusted transport/runtime linking. Source
and dependency admission still precedes all host binding injection. There is no
new game operation, creator flag, Cargo dependency or profile exemption.

The host creates a unique 0700 directory and a 0600 FIFO, keeps it open read/write
with CLOEXEC and leaves it empty. Rustc has its implicit work token; no additional
tokens are supplied. The compiler sees only a read-only bind of this private
control directory at `/compiler-control` and a fixed `CARGO_MAKEFLAGS` value,
after clearing inherited environment. It inherits no new host descriptor.
The keeper and directory stay alive until the compiler exits or fails, then
are dropped. Creation/open failure is an error; a compiler jobserver connection
warning is rejected rather than accepted as an unrestricted fallback. See
[rustc's jobserver contract](https://doc.rust-lang.org/rustc/jobserver.html).

This control is owned by the trusted builder, not a system/thread API for game
code. The game source receives neither its path nor a FIFO capability, and the
game runtime does not mount it. It changes SDK/cache identity through both the
preparation source and `compiler_jobs.rs`. Compiler optimization arguments,
source/object hashes, receipts, confinement and existing budgets remain bound.
Unit checks cover private permissions, empty token state, CLOEXEC, command
construction, diagnostic rejection and cleanup; actual confined preparation
checks exercise the lane with the pinned Rust toolchain.

This implementation changes the **Linux-host** builder, including its Windows
cross-target builds. It does not add a jobserver to the native Windows-host
compiler. Native Windows large-source memory behavior still requires separate
qualification; Wine/cross-target results do not establish it.

## Port-side interrupt-priority integration checkpoint

The ports review branch provides `service-kernel-irql/state.rs`, layered over
existing GamePKG copied records, actual reviewed task identity, typed recursive
mutex acquisition and checked RAM writes. Its IQL1 controller owns a bounded
33-slot task table, raised/masked counts and wrapping transition count. It is
bound to the title root and KSV3 identity; it supplies no host pointer, native
thread API or direct OS/device operation. SDK identity and all source/dependency/
object/confinement limits remain unchanged. The app's pinned ports submodule is
not updated by this component checkpoint.

The exact port contracts, record layout, literal hook signatures, partial-error
semantics and bounded abort cleanup are in the ports
`host-integration/engines/takedawn/service-kernel-irql/README.md`. Cross-down must
run the old-level safe point while gated, drop counted ownership, unlock, then
publish the new guest PCR byte before the deferred release point. CLI and raised
holds remain separate; enabling after STI requires the reviewed following-
instruction shadow at the native call site. Hooks remain checked port code,
not trusted host callbacks, engine exemptions or arbitrary function pointers.

The component fixtures compare ordered state/RAM/return witnesses with 25 pinned
original C functions and execute actual reviewed workers for ownership and
normal/cancelled cleanup. An intentionally reordered PCR/release control must
fail. Full fixture source admission precedes binding injection; each of eight
separately optimized objects is audited before trusted runtime linking on
Linux32/64 and Windows32/64. Actual confined Linux execution is distinguished
from Windows component execution through Wine and cross-target compile/link
checks. These are controlled callbacks, not real pending-device delivery,
physical input, audible audio, hardware visuals, gameplay or measured pacing.

Do not enable Burnout from these results. Real pending IRQ/safe/release hooks,
native finite handler routes, worker stacks, timer/DPC integration, original
diagnostics, full renderer/media/bootstrap/launcher wiring and complete title
qualification remain required. Halo also remains blocked until its own migrated
source and runtime pass admission. This component does not consume the old
legacy direct-service path or change either profile gate.

The ports `service-worker-stacks` component supplies the next prerequisite:
up to sixteen original 256 KiB **guest** slices with root/KSV3-bound actual-task
leases, generation checks and explicit exhaustion. It uses existing copied
record operations and grants no memory pages, native pointer, executable memory
or protection exemption. Supply the reviewed bootstrap base/size to one shared
pool; keep all callback RAM access checked. The exact WKS1/576-byte contract,
cleanup semantics and token adoption rules are in that component's README.
16,384 original allocator witnesses per target and four competing reviewed
workers pass; all eight optimized fixture objects remain audited. Confined Linux
runs and Windows component checks through Wine do not qualify actual IRQ/DPC or
gameplay. The timer must retain its lease for its worker lifetime; device
callbacks release only after state restoration. Pending bits must be restored
on stack exhaustion. No app SDK/recipe identity, limit, context format, pinned
submodule or engine admission change accompanies this port-side prerequisite.

For checked port composition, IRQL Task views now permit one opaque copied-record
hook context through `irql_hook_bind` / `irql_hook_context`. Unbound use reports
Unsupported; zero, stale and replacement bindings reject. Each fresh Task view
starts unbound. This is a port-side value, not a native pointer, function callback,
SDK operation, type assertion or lifetime extension; the literal consumer still
validates exact record/root/controller associations. IQL1 serialization and the
four-ID native context remain unchanged. Original IRQL differential, source and
all-object checks remain qualified. A separate controlled IRQ composition draft
passes source/object smoke checks, but original-C posting/race/device/timer/DPC
qualification and full bridge context integration remain unfinished. Neither
legacy profile is enabled by these results.

## Checked port IRQ composition boundary

The ports `service-kernel-irq` library composes the existing local SDK, raw gate,
IQL1, WKS1 guest stacks and v5 registers in checked game source. IRQ1 is a
384-byte root/KSV/IQL/gate/pool-bound record. It provides original Posted/Gated/
Ungated posting results, coalesced lines, ordered batch delivery, actual task
ownership, second gate attempt, safe/release paths, explicit exhaustion/error
requeue, register/FS restoration, statistics and owned close/cancel. Exact API,
record layout, literal route/observer signatures and remaining work are in the
ports component README/WORK_IN_PROGRESS. No native function pointer, host address,
new SDK operation, game dependency or engine/media trust exemption is introduced.

Original-C default witnesses cover full CPU/ordered RAM/stats/IRQL/stack state.
Actual reviewed workers cover deterministic posting/release windows, new posts
inside a callback, cancellation, active-close denial, options, revocation,
malformed resources, readonly PCR, source/all-object enforcement and quotas.
Instrumented race/fault copies have separate exact provenance from unmodified
production sources. Windows execution is Wine component testing; actual confined
Linux32/64 fixture execution is not real graphics/input/audio/gameplay/pacing.

One reproduced cleanup bug combined full sync quota with a stale abort CAS:
allocating a new event for retry stranded counted ownership. IQL abort/cleanup,
worker stack release/close and IRQ active/pending/close now retry bounded existing
records without allocating events. Generic event availability after cancellation
was not the issue. SDK sync/record/task limits, compiler budgets, no-post-audit
LTO, source/dependency/import checks and process confinement remain unchanged.
Long-wait diagnostics explicitly preserve original signed32 Xbox/Windows LONG
conversion across targets, avoiding Linux64's narrowing of wider-long comparisons;
this adds no gameplay duration limit or arbitrary wait rejection.

Actual finite device handlers, production diagnostic/reload/registration paths,
DPC/timer/spin/TIB roles, full native context/bridge, renderer/media/bootstrap/
launcher wiring and title qualification remain required. The new library does
not admit either legacy profile or change the pinned ports submodule, app SDK/
recipe identity or installed application binary.

The ports review branch now supplies `service-kernel-dpc/notify.rs` and its
`NOTIFICATIONS.md` contract. `Notifier::notify_new` binds a queue's assigned
actual consumer to its root/KSV3/IQL/raw gate. `notify_queued` wakes only with
zero caller gate depth and prompt on; `notify_release` additionally requires
outside ISR, IRQL below 2 and pending work on a non-consumer. These are distinct
original predicates. Another task's gate hold is not this task's depth.
`notify_insert` notifies only successful new insertion after queue unlock;
duplicate/null/full insertion never wakes. All resource associations and actual
task ownership remain checked; no record, quota, SDK operation, native callback
or game import exemption is added. The kick reuses the existing local clock
wake generation and does not execute deferred routines inline.

The production timer must register its Sleeper, guest stack and TIB and signal
readiness before producers run. The notifier constructor does not prove that
readiness. Original spin wake remains a separate role; do not reuse the timer's
single sleeper for it. Insertion followed by a failed notification is a partial
operation requiring actor/session error handling, not blind retry. Stop producers
and actually join the consumer before closing queue/controller/root. Production
observer/context, native DPC callback/drain, statistics and timer/device/launcher
integration remain unfinished; neither legacy profile is admitted.

Notification fixtures compare 512 original-C policy cases per target and actual
worker/gate/wake behavior, including pre-wait wake retention and lifecycle/error
controls. All fixture source precedes injected bindings and all eight optimized
game objects are audited before trusted linking, with exact identities retained
in receipts. Actual confined Linux
fixtures, Windows component execution through Wine and Windows compile/link
artifacts are distinct from real title gameplay, hardware visuals, physical
controls, audible audio and measured pacing. App SDK/binary identity and the
pinned ports submodule remain unchanged by this port-only component.

The ports `service-kernel-dpc/callback.rs` now supplies a separate checked native
DPC calling-convention component, documented in `CALLBACKS.md`. Its nonowning
Callback value binds the assigned actual consumer, root/KSV3, IQL controller
and WKS pool. Calls require that worker's live retained guest-stack lease and
valid ESP range. The helper writes KDPC arguments, pushes the original five
words, begins profiling before entering priority 2, invokes a finite static
checked route, ends profiling and restores priority. Successful stdcall return
must consume 20 bytes; otherwise the actor aborts. Registers/RAM/current guest
FS changed by a successful DPC are preserved, unlike IRQ register rollback.

Unknown nonzero routes are Unsupported before mutation. Errors after mutation
attempt observer completion, priority restoration and existing bounded abort
cleanup; no acknowledgement or fake task completion follows. The caller retains
the timer's stack/TIB and must release/join on every exit. Original queued drain
still needs its zero-time gate try; this helper does not supply that timer loop.
Literal route/profiling providers remain checked port sources, not injected
host engine code, callback pointers or admission exemptions. The fixture's
deterministic providers must not be used as production title handlers.

256 pinned original-C callback cases compare CPU/full-RAM/result/profile order
per target, with controlled lookup/profiler/IRQL providers. Actual worker tests
cover return/observer/body/write failures and partial priority entry, abort,
outer-hold release, retained stack lifetime and actual join. The exact provider
scope is distinct from original IRQL/gate qualification and from production
callbacks. Complete source/object gates remain unchanged, including the
no-alias check. Confined Linux32/64 execution, Wine components and audited
Windows artifacts do not qualify real gameplay, physical controls, audible
audio or frame pacing. Production finite handlers/profiler, queue drain,
timer/event/spin/TIB/context, renderer/media/bootstrap/launcher and full source
closure remain required. SDK/binary identity, engine profile gates and pinned
ports submodule remain unchanged by this port component.

## Port-side retained DPC drain checkpoint

The ports review branch now defines `service-kernel-dpc/TAKE.md` and `DRAIN.md`.
`queue_nonempty` reports entries even to the assigned timer, unlike the pending
notification predicate. `queue_take_begin` transfers one still-held recursive
mutex depth to an actual-consumer-bound observation. `take_depth`/`take_entry`
validate its exact version/root/KSV3/task before original head/Inserted
publication. `take_publish` advances, clears and attempts unlock on every result;
`take_abort` performs actual-owner cleanup even after cancellation/revocation.
Wrong-task use cannot unlock or mark an active observation closed. A failed
flag clear can leave head advanced; callers must stop/report that partial prefix.
No new SDK object, callback pointer, quota or record format is introduced.

The checked `Drain` binds the existing Callback/queue/controller/pool to its
assigned timer and retains lag state across passes. `callback_ready` exposes
the existing nonmutating identity/lease/register/stack preflight. `drain_before`
returns pending/ran, preserves original host-lag and guest-clock guard order,
uses a zero-time dispatch-gate try, observes depth while still queue-locked,
then publishes/unlocks before latency statistics and finite native execution.
Holes/null routines are consumed without counting a callback; newly queued work
can run on the next iteration. The clock provider delegates to existing local
host-counter/shared-skew services. There is no new instruction meter, arbitrary
callback cap, render limit, host syscall or engine trust exemption.

Providers remain checked port sources. Real profiler/histogram/configuration,
finite title routines, timer/event/spin/TIB/native-context and renderer/media/
bootstrap/launcher integration remain required. Every observation is cleaned
before IQL abort on error; failures never acknowledge unfinished callbacks or
fabricate task completion. Release the retained lease on every worker exit and
actually join before resource close. Neither native profile is admitted, and
SDK/binary identity and the pinned ports submodule are unchanged.

Qualification covers 1,024 original queue operations per statistics mode/target,
64 separate actual SDK ownership/cancellation lifetimes per target and 384
retained original-C run/guard/drain witnesses per target. Controlled providers
compare CPU/full-RAM/ring/clock/profile behavior, real foreign-gate contention,
self-requeue, partial guards and nine fatal-prefix cases. Ownership, cancel
cleanup, publication, guard, count, retained lag and observer-order mutations
must fail. All sources precede bindings and all eight optimized game objects
per target precede trusted linking. Notification and Callback consumers are
requalified with current source hashes. Confined Linux32/64 fixtures, separate
maintainer lifetime shells, Wine components and Windows compile/link artifacts
are distinct from production title gameplay, real Windows confinement, hardware
visuals, physical input, audible audio or measured frame pacing.


## Typed child-local event observation and waiting

ABI 1 adds local operations **27** and **28**. These execute entirely in the
trusted child-local runtime, independently of the remote service transport and
window-owner dispatch. They accept synchronization IDs from existing operation
20; they do not create a new resource kind or enlarge the 128-object quota.

| Local operation / Rust helper | Exact request | Success reply |
| --- | --- | --- |
| 27 / `local_event_wait(event, milliseconds)` | LE u64 event ID, LE u32 timeout 0..1000 ms | empty |
| 28 / `local_event_state(event)` | LE u64 event ID | LE u32 manual, LE u32 signaled; each exactly 0 or 1 |

Requests reject truncation/trailing bytes; snapshot capacity below eight bytes
is Invalid (-1) before observation. A mutex passed to either operation is
Invalid without acquisition or mutation. Zero, stale, foreign-runtime, released
or closed IDs are Denied (-4). Checked-task cancellation also returns Denied;
bootstrap has no reviewed-task cancellation flag. Type, lifetime and signal
checks retain the same object and hold its value lock. Auto-reset signals are
consumed exactly once on a successful wait; manual-reset signals persist until
explicit reset. Timeout returns Busy (-2), including an unsignaled zero-time
poll. Snapshot consumes nothing and allocates no quota: it reports only the
state at that instant, without a lease or guarantee about a subsequent wait.
Release, cancellation and runtime close wake blocked typed waiters. Cancellation
notification synchronizes with the wait value lock so it cannot vanish between
the flag check and the atomic condvar sleep. An already-cancelled task does not
consume an available signal. Existing generic wait operation 23 is unchanged.

Blocking waits belong only to reviewed child workers. Calling the low-level
Runtime wait with owner dispatch selected returns Busy. They never block the
app window owner thread. No raw OS event, callback pointer, dependency or game
import exemption is exposed. Errors leave caller output buffers unchanged.
These SDK sources change build identity and invalidate old native cache receipts.

Tests cover exact framing, output capacities, manual/auto semantics, wrong-kind
non-acquisition, two actual competing waiters, quota exhaustion, cancellation,
release/close, foreign/stale IDs and the cancellation/sleep lock boundary. The
checked deadline fixture additionally uses the new event helpers before owner
dispatch, after source admission and auditing every emitted optimized object.
Linux x64/i686 execution is distinct from Windows x64/i686 Wine component tests
and Windows artifact preparation. These are infrastructure checks, not physical
input, audible audio, hardware visuals, production timer scheduling or gameplay
frame-pacing qualification. Both unmigrated title profiles remain rejected.

Port integration still owns guest event maps, timer slots, callback routing and
original publication order. An opaque SDK event ID is not a guest pointer. Do
not assume snapshots provide a multi-object atomic wait or guest-state CAS, and
do not replace original timer flush/shadow-state ordering with a snapshot. The
shared timer/DPC worker must be actually joined once before closing its borrowed
resources; the existing consuming join must not be called independently by two
components. Timer/event adapters and full native source closure remain required.


## Port-side timer, shadow-map and event coordination

The reviewed ports branch now contains the checked
[timer table](https://github.com/centrexity/GamePKG_Ports/blob/58f303b/host-integration/engines/takedawn/service-kernel-timers/README.md)
and [shadow/event adapters](https://github.com/centrexity/GamePKG_Ports/blob/e1c43c0/host-integration/engines/takedawn/service-kernel-shadow/README.md).
These are **game sources**, not trusted host implementations or profile
exemptions. They use existing local records, typed mutexes/events, reviewed
workers and checked guest memory. SDK/binary identity and the pinned ports
submodule are unchanged; every new source kit still needs its own exact receipt.

The port retains the original 32 timer slots and publishes periodic rearm or
one-shot removal before unlocking and calling a DPC. Its queue supplies a typed
witness only after the shared worker's actual consuming join; multiple owners
can close without joining that native job twice. The logical shadow map retains
4,096 guest/token slots, but tokens confer no native authority and do not raise
the 128-active-sync-ID limit. Scoped guards prevent early close, validate page
ownership, and expose a creator-only deletion lane after KSV3 revocation.

Typed event adapters preserve initialization order, inline-event double-checks,
manual/auto semantics and lifetime revocation. Post-set timer reset leaves the
guest signal word unchanged; post-callback signaling precedes writing that word.
Partial-prefix failures are explicit and cannot be blindly retried. Game code
never casts a guest VA into a native handle. The port's cancellation review
identifies a legacy mixed host/guest timer-struct path: it requires a checked
port contract, not a new raw native timer-queue API. No full guest wait/object
bridge, actual timer callback or complete worker/bootstrap migration is claimed.

Qualification includes original-C timer/map/header/state comparisons, hostile
metadata/type/stale/lifetime/fault controls, actual reviewed event workers with
single-consumer auto reset, exact quota/leak accounting, every optimized game
object on four targets and actual confined Linux x64/i686 fixtures. Windows
component runs use Wine; Windows artifacts are compile/link qualification.
These are distinct from real Windows confinement, title gameplay, physical
controls, audible audio, GPU fidelity and measured pacing. Both native profiles
remain rejected pending full source/service/builder/launcher closure.


## Native timer firing composition checkpoint

The ports branch now provides
[`timer_fire_pass`](https://github.com/centrexity/GamePKG_Ports/blob/b5e1a04/host-integration/engines/takedawn/service-kernel-timers/FIRING.md)
as checked game code, composing the existing timer scan, native DPC calling
convention and typed event shadows. It verifies the same queue/actual worker
before mutation, runs callbacks after slot publication/unlock, and resolves the
current event mapping only after native code returns. Fatal prefix errors abort
the actor without rolling back side effects or acknowledging a completed pass;
retained-stack release, actual single join and owner cleanup remain required.
No SDK operation, raw native object, dependency or profile exemption is added.

Nine actual-SDK scenarios and five required-failing mutations check replacement/
removal/rearming, null cases, wrong-queue readiness, callback/signal errors and
abort/cleanup. A separate actual producer acquires the timer mutex from inside
the callback protocol, proving that lock is not retained across native execution.
Witnesses match across all four targets; source admission and every optimized
object precede trusted linking, with actual confined Linux x64/i686 runs.
Original timer-table/callback C checks are requalified separately. Windows
component execution uses Wine; Windows artifacts are compile/link checks.

This composition uses controlled finite native routes and fixture clock/IRQ/
profiler providers. It is not independent original whole-loop fidelity, actual
production handlers, title gameplay, real Windows confinement, physical input,
audible audio, GPU fidelity or measured pacing. The complete worker/bootstrap,
cancellation/wait/object contracts and source/build/launch closure still gate
both titles. SDK/binary identity, installed app and pinned ports submodule remain
unchanged by this port-only implementation and contract update.


## Shared completion evidence for one native worker

The generated Rust SDK provides `local_task_join(task: u64) ->
Result<LocalTaskJoin, i32>`. This wraps the existing ABI 1 local operation 11:
one little-endian u64 task ID in, exactly one little-endian u64 return value out.
`LocalTaskJoin::task_id()` and `LocalTaskJoin::result()` return copied u64 values.
The receipt's fields are private; checked game source cannot construct it. It
contains no native object, pointer, lease or authority to keep a resource alive.
A successful join consumes the runtime job exactly once. Busy (-2) preserves it;
missing, already joined or wrong-kind IDs return Denied (-4). A malformed
successful reply returns Failed (-3). The result is the worker's return value,
not a claim that its application-level work succeeded.

Use this receipt when several port controllers own records used by one worker.
Retain it after the actual consuming join and let each controller validate its
assigned task and ownership before accepting the same borrowed receipt. A
`local_task_finished` snapshot is insufficient. Existing scalar join wrappers
remain available, but discarding the typed receipt prevents later reconstruction.
Acceptance must not bypass creator/root checks or cleanup ordering. Record and
stack cleanup still require their own contracts; the receipt extends no lifetime.

Current qualification covers full-width IDs/results, Busy and duplicate joins,
wrong-kind IDs, private-field construction rejection, complete source scanning,
every emitted optimized game object, and actual confined Linux x64/i686 execution.
Windows x64/i686 artifacts are built and component tests run under Wine; this is
not Windows desktop gameplay, physical input, audible audio or measured pacing.
Queue/schedule/timer consumers exercise one join from either controller or the
SDK, wrong-job evidence, borrowed/foreign roots and cleanup after configuration
revocation. Three intentionally broken consumers must fail. No admission,
dependency, import or confinement exemption was introduced. Both unmigrated title
profiles remain blocked and the pinned ports submodule is unchanged.

The full SDK identity is
`fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f`;
the Linux app reports native SDK `544b35bf60db343f`. Previous native receipts must
be rebuilt rather than relabeled. This section supersedes earlier checkpoint
statements that the installed SDK remained unchanged.


## Worker-local guest TLS consumer

The reviewed ports `service-thread-tib` consumer implements original per-worker
guest TIB/TLS allocation using the existing serialized port-owned heap actor and
GamePKG-owned tasks, mailboxes and non-executable RAM. It adds no host operation,
SDK identity change, dependency or import exemption. Its `thread_tib_new` accepts
the applied bootstrap's guest TLS template/extent and returns a guest TIB address;
it exposes no native TLS address. Worker configuration, stack, CPU.fs_base and
production timer/IRQ integration remain port responsibilities. Errors after
allocation can retain their write prefix; a failed worker must not enter game
code with a fabricated completed TIB. See the ports `service-thread-tib/README.md`
for the exact arguments, ownership, error and test contracts.

The original-C comparison covers seven modes, two allocations per actual SDK
worker, a separate heap actor, both guest addresses, allocation counters and
complete 64MiB RAM hashes. Four deliberate fidelity regressions must fail.
Four-target source/object checks and actual confined Linux x64/i686 execution
pass with SDK fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f;
Windows component execution uses Wine. No full title startup/gameplay, physical
controls, audible audio, graphics fidelity or measured frame pacing is claimed.
Both unmigrated title profiles and the pinned ports submodule remain unchanged.


## Retained timer-worker startup consumer

The reviewed ports `service-timer-worker` module composes existing GamePKG-owned
tasks, copied context IDs, checked RAM, a retained guest stack and per-worker TIB
via the serialized port heap actor. Its `Worker::worker_start`, `worker_parts`
and `worker_finish` contracts are documented in the ports README. Only the actual
reviewed worker may start; bootstrap task 0 rejects. Simultaneous safe borrows
feed the existing callback/timer APIs while retaining one slice. Terminal cleanup
errors remain visible on repeated finish calls. Local finished state is not a
join receipt or an assertion that shared actor teardown succeeded.

Ten startup/lifetime scenarios and six required-failing variants pass on all
four targets with complete pre-injection source checks, every one of eight
optimized game objects audited before trusted linking, and actual confined
Linux x64/i686 execution. Windows component runs use Wine. SDK identity remains
fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f; no schema, quota,
permission or admission exemption changes. Production context/handler/worker-loop
and full source/build/launcher closure remain required. This does not qualify
title gameplay, physical input, audible audio, GPU fidelity or measured pacing;
both unmigrated profiles remain blocked and the pinned submodule is unchanged.


## Ordered retained-worker dispatch consumer

The ports `service-timer-worker/dispatch.rs` now connects retained worker context
to pending-DPC drain, a production host-owned millisecond clock sample after
drain, and timer/callback/event firing. It checks the exact actor/root/stack
generation and table/queue binding. `dispatch_actor` permits the existing typed
IRQ layer to attach its copied context before production hooks run. Actual
attachment and handlers remain required. `Worker::worker_cleanup` exposes only
actual-owner checked RAM and cached lease identity after cancellation/revocation;
normal CPU/lease access remains denied. Dispatch teardown aborts gates before
stack release and preserves terminal cleanup errors. Exact port contracts are
in `service-timer-worker/DISPATCH.md`; these APIs add no generic host operation,
SDK identity change, dependency, quota or engine/media exemption.

Eight real-clock actual-worker compositions, six required-failing variants and
ten startup regressions pass on four targets. All emitted optimized game objects
are audited after complete source scanning and before trusted linking. Actual
Linux x64/i686 children execute confined; Windows components run through Wine.
These synthetic-memory tests use finite test routines and inert IRQ/profiling
hooks. They do not establish original whole-loop fidelity, production title
handlers/gameplay, physical input, audible audio, GPU fidelity or measured pacing.
Context/IRQ/vblank/spin/pause/warp/deadline/wait and full source/build/launcher
closure remain required. Both unmigrated profiles stay blocked. SDK remains
fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f.

## Bound native interrupt-registration import

The reviewed ports `service-kernel-context` contract connects ordinal 98
(`KeConnectInterrupt`) to the same checked `Connections` registry used by ISR
callers. The registry and `KernelContext` remain dependency-free game code in
`service-kernel-pool/kernel_pool.rs`; they are not trusted host implementations.
KIC1 is a strict 64-byte copied record binding root, KSV3 and live registry IDs.
KPL4 adds its full u64 context ID at byte 24, starts ordinal slots at byte 32
and retains the bounded refusal counter after the slots (maximum 1548 bytes).
Root288, KSV3/64, CPU v5/316 and four-ID worker transfer remain unchanged.

`Imports::imports_scan_with_context` provides complete checked planning;
`image_imports::imports_image_with_context` additionally requires the actual
authenticated image and its published bounds and derived data region. The latter
is the production adapter. Context-free KPL2/KPL3 paths continue to reject 98;
`kernel_supported` classification alone never establishes context readiness.
Publication and invocation revalidate the same live root/KSV/registry binding.
Context IDs are not leases: stop and join users before releasing owners.
Creator-only cleanup and accepted-prefix/error semantics are specified in the
ports `service-kernel-context/README.md`. Other unsupported imports still reject.

Three complete-plan/actual-worker cases, five required-failing variants and
authenticated synthetic image mode 11 pass. Complete source checks precede SDK
injection, and all eight optimized game objects are audited before trusted
linking on Linux/Windows x64/i686. Actual confined Linux children execute the
cases; Windows component execution uses Wine, while image qualification on
Windows covers source/object/link checks. Current ISR, initializer, clock,
allocator/memory and legacy import regressions pass. Initializer admission now
retains all 316 CPU bytes plus the existing 36 RAM/state bytes per witness.

This does not establish actual title entry, Windows confinement/gameplay,
physical controls, audible audio, GPU fidelity or measured pacing. Production
factory adoption, real IRQ/device/profiler handlers, disconnect semantics, whole
timer loop and full source/build/launcher closure remain required. No legacy
native profile, engine/media exemption, quota or admission bypass is enabled.
The pinned ports submodule and installed app SDK remain unchanged; SDK identity
is fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f. Checked port
source kits must rebuild with new exact-source/object receipts.


## Checked guest ISR consumer

The ports `service-kernel-isr` component now uses existing copied records, checked
RAM and actual reviewed task/retained-stack identity for a 32-vector guest
registry and original ISR calling convention. It exposes no host function pointer
or native interrupt object. Caller-held single-CPU gate ownership, checked TIB
and stack, static finite routes, IRQL16/profile/restore order, EAX & 1, partial
prefix errors and creator-only cleanup are specified in its README. No SDK API,
root/KSV/CPU format, quota, game dependency or import exemption changes.

128 original-C cases and ten native error/lifetime modes pass on four targets,
with eight required-failing controls, complete pre-injection source scanning,
every one of eight optimized game objects audited before trusted linking and
actual confined Linux x64/i686 execution. Windows components use Wine. These
controlled routes/profile/IRQ fixtures are not production device handlers.
A separate four-target review confirms the original disconnect struct's host
layout mismatch; ordinal 100 remains unimplemented pending explicit contract
review. No main import or unmigrated native profile is enabled. Production
registry/context and ABI wiring, real generated/device/profiler handlers, timer/
vblank/spin/pause/warp/wait loop, full source/build/launcher closure and actual
gameplay/GPU/input/audio/pacing qualification remain required. SDK stays
fe309935dd602e2cf098e50065f6035de33dac88b51d45a2bc1ba47cf026742f.

## Authenticated port startup ownership

Ports `service-kernel-context/startup.rs` composes existing GamePKG-owned records
and tasks into a checked game-side import startup owner. It retains services,
registry, context and map across partial setup errors, publishes ready state only
after the authenticated complete planner succeeds, and closes map/context/
registry/services in order. It adds no trusted host code or SDK operation.
The exact APIs and six phases are in ports `service-kernel-context/STARTUP.md`.
RAM, image authentication/loading, CPU bootstrap, heap/data, renderer, providers
and the full native session lifecycle remain explicit coordinator obligations.

Its bounded task list accepts actual SDK joins, including a join consumed by
another owner. A finished snapshot, wrong-task proof or repeated evidence cannot
release an outstanding lifetime obligation. Every real user must be registered
before shutdown can proceed; copied IDs are not tracked automatically or treated
as leases. Partial startup cannot be replayed, and cleanup after revocation
preserves terminal errors without treating them as successful gameplay.

The owner uses the actual authenticated-image adapter, with no context-free
fallback, reduced source/object scan or engine/media exception. Full source,
remaining kernel/manual/generated/media, device handlers, timer/pacing loop,
renderer and build/launch integration still require completion. Both unmigrated
native profiles remain blocked, the SDK and pinned submodule stay unchanged,
and the already installed app SDK needs no binary change for this port-only API.

The startup fixture passes all four target builds with both separately optimized
game objects audited before trusted linking. Actual confined Linux x64/i686 each
pass 22 synthetic image cases and two independently authenticated private ISO
cases. Five broken copies build under the same rules then fail actual Linux64
behavior. Partial-record quota exhaustion restores original capacity; no quota
or import vocabulary expands. Windows scope remains source/object/link only.
The private title still rejects before publication: 107 main functions remain
unsupported with bound context, versus 108 without it. These fixture results do
not requalify full generated-source closure or gameplay/input/audio/GPU/pacing.

## Live periodic clock registered before launch

Ports `service-kernel-context/tick_startup.rs::startup_tick` connects the actual
periodic KeTickCount worker to the authenticated startup owner. The checked Tick
APIs now split allocation, opaque argument, exact actual-task assignment and
resume. The coordinator uses Startup's suspended spawn so its join obligation
exists before any periodic write. Partial assignment retains the actual job for
cancel/join before releasing metadata. Creator/root and full cached metadata/KSV
checks precede resume; no SDK, TCK1/64, root/KSV/CPU/four-ID format or quota changes.

`Tick::tick_join_task` returns actual SDK consuming-join evidence, while
`tick_accept_join` accepts the exact proof consumed by Startup or another owner.
A finished or already consumed job without witnessed proof cannot authorize
cleanup. The existing sole-owner start/close APIs remain compositions; shared
owners must retain/share the proof explicitly. Exact contracts, partial failures
and the separate terminal-error rules are in ports
`service-kernel-context/TICK_STARTUP.md`.

A reproduced clock race now rechecks the pinned KSV after settings lookup even
when that lookup errors. Revoked/replaced binding is Denied before writing,
not a fresh unconfigured Unsupported result. The deterministic maintainer seam
and a required-failing old-guard copy are separate from admitted game code; the
seam rejects even when packaged unused. No production fault hook or audit
allowlist entry is added. The full native session, timer/DPC/IRQ/pacing loop,
remaining imports/source/renderer/media/build/launch and real gameplay still
require completion. The installed matching SDK and pinned ports submodule remain
unchanged; this is port code and contract work, not an enabled title release.

This live-clock checkpoint passes four-target source/object/link qualification,
64 actual confined Linux startup sessions (30 synthetic and two private image
checks per architecture), and eleven broken confined integration variants. The
standalone Tick C/native/lifecycle suite also passes, including deterministic
KSV unlink/replacement on all four targets (Windows via Wine), a compiled
old-guard required-failing control and actual confined writer progress while the
broker is held for >=60ms. Windows image tests remain build-only. These scopes
do not qualify original title entry, physical devices, audible audio, GPU
fidelity, actual Windows confinement or measured frame pacing. The private title
still has 107 unsupported context-bound main imports and stays blocked.

## Retained timer-job startup and shared join forwarding

Ports `service-kernel-context/timer_startup.rs::Launch` now coordinates the
same actual suspended SDK job across authenticated Startup, DPC Queue, Timers
and pacing Schedule. Strict creator/live/unassigned preflights precede spawn;
Startup tracks the job before queue/table/schedule assignment. Partial setup
retains the exact gated job and published prefixes, including the timer table's
private join obligation before a fallible lock/publication. Resume separately
revalidates all owner IDs and the full u64 queue/table/schedule job association.

One actual consuming join may originate from Launch, Queue, Schedule or Startup.
Launch verifies and forwards that same private SDK evidence to every applicable
owner, remembering successful forwarding prefixes for retry. Same-root/creator
Startup objects with different KSV/KPL4 lifetime identities cannot substitute
for the original owner. Creator-only `Startup::startup_lifetime` and
`startup_has_task`, plus Schedule's observed actual joined task, support cleanup
after KSV revocation without reviving a closed owner or minting another join.
No copied ID or finished snapshot is a lease or completion proof.

Exact APIs, nine progress observations, partial/error semantics and caller-owned
argument/record cleanup ordering are in ports
`service-kernel-context/TIMER_STARTUP.md`. The launcher supplies no timer body
or fake readiness: the reviewed worker must still establish its retained stack/
TIB, sleeper, Dispatch and real IRQ/device/profiler hooks before enabling async
producers. Controlled finite callbacks and inert test providers qualify startup
composition only. Complete original loop, source/import/renderer/media/build/
launch and gameplay remain required. SDK, quotas, root/KSV/CPU/four-ID codecs,
source/object/confinement rules, pinned submodule and blocked title profiles
remain unchanged. The installed matching host SDK needs no binary update for
these checked port APIs.

The retained-launch checkpoint passes 84 actual confined Linux startup sessions
(40 synthetic plus two independently authenticated private-image checks per
architecture), four-target full source admission and both optimized object audits,
and 17 required-failing compiled integration variants. Existing DPC, timer,
pacing (32 repeats) and dispatch original-C/native/adversarial suites pass on
current exact sources; Windows component execution uses Wine. Image Windows
scope remains build-only. These controlled routes/inert IRQ/profiler providers
do not qualify full title/device/graphics/input/audio/Windows confinement/pacing.
The context-bound main import gap remains 107 and both title profiles stay blocked.

## Explicit timer-initializer import binding (KIC2)

Checked port code now classifies ordinal **113 KeInitializeTimerEx** as eight
argument bytes and invokes the existing checked `timer_initialize` operation.
It requires `KernelContext::kernel_context_new_with_shadow(&Memory,&Connections,
&Shadow)`. The new KIC2 record is exactly 72 bytes: little-endian u64 magic
`0x3243494b`, root, KSV3, ISR1 registry, shadow-map ID, then four reserved zero
words at 40/48/56/64. The existing strict KIC1/64 layout stays unchanged and
cannot admit 113. `kernel_context_shadow(&Memory)` returns a borrowed map;
it cannot close its owner. Stop and join all users before owner release.

`kernel_context_ready(&Memory,context_id,ordinal)` separates classification
from actual per-import readiness. Main-table and lookbehind planning use it
before patching, and invocation rechecks it before caller/CPU mutation. KPL4,
worker four-ID, CPU v5/316, SDK, host quotas and admission policies are unchanged.
A prepared plan revalidates the live map through its context before publication;
these copied IDs are not leases or an atomic lifetime transaction.

The one checked `Shadow`/`Guard` and event implementation now lives alongside
`Connections` in port `service-kernel-pool/kernel_pool.rs`. The old shadow path
reexports `Shadow`, and event functions forward to that implementation. No game
engine code enters the trusted host; no SDK operation or dependency is added.
The original map and event mutation tests now mutate that actual implementation.

The timer bridge reads both u32 arguments, retains the original 40-byte zeroed
header/type/size writes before event allocation, selects manual reset only for
kind exactly zero, and uses the same event map as other timer callers. A null
timer returns success without allocation. Success sets EAX=0 and advances ESP by
12; all other CPU bytes remain unchanged. Allocation, RAM, kind/lifetime and
publication failures remain explicit, with accepted guest/event prefixes retained;
callers must stop/report rather than retry or invent CPU success.

The actual four-ID worker fixture checks kinds 0/1/2/0xffffffff, exact header and
boundary sentinels, event signal consumption, null input, legacy-context planning
refusal, forged KPL4 legacy binding, full-u64 shadow identity, reserved fields and
shadow revocation between planning/publication. Seven required-failing context
copies include missing-shadow readiness and truncated-ID mutations. Linux32/64
run the admitted confined child; Windows32/64 component execution is Wine, with
separate source/object/link admission on all four targets. Map/event, timer-table
and retained timer/dispatch regressions pass on these exact sources. Legacy pool
and complete-import comparisons also pass. None is title gameplay, real Windows
confinement, device input, audible audio, graphics or measured frame pacing.

The authenticated Startup factory still creates KIC1. It must explicitly own or
bind the shared map and coordinate its event cleanup before using this new path;
raw-table component qualification does not migrate that factory. Its private
main-import gap remains 107 (108 without context); ordinal 113 remains a readiness
gap there despite its implemented classification. Complete generated-source,
handler/loop, renderer/media, builder/launcher and actual gameplay qualification
remain required. Both title profiles, catalog recipes, public source archive,
pinned submodule and launch defaults remain unchanged and blocked. Rebuild checked
source kits and receipts for the changed exact game-source/object identities.
The installed matching host SDK does not require a binary update for these APIs.

## Authenticated KIC2 shadow ownership and timer import

The explicit `Startup::startup_prepare_with_shadow` path now owns the one shared
shadow map, binds it into KIC2, and hands out only live borrowed maps through
`startup_shadow`. Its ready/spawn checks reject a substituted same-root map.
The existing KIC1 prepare path keeps its original allocation requirements.
All actual timer/Tick joins precede event drain, map/mutex release, registry and
service teardown. Source/object/process enforcement and profile gates stay intact.

Exact APIs, partial setup, borrowed ownership and cleanup/error semantics are in
ports `service-kernel-context/SHADOW_STARTUP.md`. The authenticated retained timer
composition calls import 113 from an actual assigned worker and uses the same map
as Dispatch. 102 actual confined Linux image sessions and 21 compiled bad copies
pass; four target source/object/link checks pass. Windows image scope is build-only.
Full-title import gaps are 106 with KIC2, 107 with KIC1 and 108 without context;
each complete title attempt still rejects before native entry. No original title
playback, graphics/physical input/audible audio or measured pacing is established.
SDK/quotas/public source archive/defaults/catalog/pinned submodule/blocked profiles
are unchanged. Complete source/import/handler/loop/renderer/media/build/launcher
and gameplay qualification remain required; the host installation needs no update.

## Bounded scratch memory and atomic copied updates

The Rust SDK now provides
`local_memory_compare_exchange(window: u64, offset: u64, expected: &[u8],
replacement: &[u8]) -> Result<bool, i32>`. This is **child-local operation 42**,
a separate namespace from broker key-state operation 42. No pointer, mapped
slice, native object or GPU object escapes. Both slices must have identical
nonzero length, at most 65,536 bytes. The wire request is LE window u64, byte
offset u64, byte count u32, then exactly both copied slices. Reply is exactly
one LE u32: 0 stale, 1 replaced; malformed booleans/reply sizes fail.

The complete destination must be committed and writable before comparison,
even when its bytes would differ. The runtime memory lock covers validation,
comparison and the whole write, excluding other reads/writes/copies, protection,
decommit and runtime destruction. False has no write. Invalid bounds, overflow,
lengths or malformed packets return Invalid; absent/closed windows or denied
pages return Denied. This does not pin a window across later calls or provide a
version counter: callers needing ABA detection must compare a nonwrapping
version in their own copied format. It neither waits for a deadline nor blocks
the application's window owner; execution is inside the confined child.

The opt-in **host-only** `LocalLayout::TakedawnServices` preset provides the same
main 64 MiB/contiguous 64 MiB/flash 1 MiB windows as `TakedawnMemory`, followed
by a separate 1 MiB scratch window. All four reservations and per-page metadata
are charged before entry. Old presets and guest aliases are unchanged. A reviewed
port must explicitly consume the fourth descriptor and keep its scratch backing
out of the guest-address alias layout. The port now supplies the explicit four-window `Layout::layout_services`
adapter described below; the original three-window reader stays strict.
A full-title consumer must still adopt the reviewed path explicitly.
Only the host chooses bases/layout/entry functions; game metadata cannot request
arbitrary executable memory, devices or native addresses.

Larger mutable port data belongs in an explicitly owned bounded memory range.
Opaque records retain **1..4096 bytes each, at most 64 records and 64 KiB total**;
there is no larger-record fallback or quota exemption. A failed dense handle-table
prototype demonstrated this distinction and is preserved privately rather than
published as admitted code. The checked metadata component below now moves the full 16,383-slot
tagged table onto scratch and retains partial setup ownership with bound
root/range/page identities. Typed resource adapters must still validate real
resource lifetimes and stop/join borrowers before releasing backing. No native handle may be reconstructed from a guest integer.

SDK identity is now
`1e15c059ca66907d45e603cec9f993ca54b9ffa66bae72c4a36cee67b944fd3f`.
Native source kits, audited objects and cache receipts must rebuild. Earlier
fe309935 component checkpoints remain historical, not relabeled as this SDK.
The image qualification tool derives its SDK from all four audited receipts
rather than stamping a fixed historical value.

Both native SDK suites pass on Linux32/64 and Windows32/64 through Wine (34 runtime
tests per target; 22 binding tests on Linux and 14 on Windows, with Unix-only client
tests omitted there). Tests cover concurrent single-winner CAS and
512 successful RMW increments, 64 KiB updates, stale unchanged bytes, overflow,
foreign IDs, malformed/trailing/truncated packets, short replies, cross-page
read-only/no-access denial, revocation and unchanged record bounds. A signed
fixture passes all four checked-source/two-object/link targets and actual confined
Linux32/64 execution, including the unchanged full 64 KiB record quota alongside
scratch. Windows confinement/device/gameplay and measured frame pacing are not
established. Existing authenticated startup is requalified separately on this SDK;
its full title remains blocked and the KIC2 main-import gap remains 106.

## Scratch-backed tagged handle metadata

`service-kernel-handles` now preserves all 16,383 original tagged slots over a
bounded 256 KiB scratch range, with an 80-byte owner record and actual SDK mutex.
Typed copied identities, first-free/same/fresh behavior, atomic page versions,
partial initialization and reverse cleanup are checked. Per-page binding and
pre-publication liveness rechecks prevent continuing across observed KSV unlink;
accepted prefixes remain owned for cleanup. Exact APIs and formats are in ports
`service-kernel-handles/README.md`.

Four-target original-C comparisons, native/Wine fault and required-failing
controls, all-eight-object admission/audit and actual confined Linux32/64 checks
pass. Fault SDK facades and the C oracle cannot be packaged as game code. These
are metadata tests, not native resource ownership or title gameplay. Typed event/
file/thread adapters and context/import/startup wiring still remain. SDK/quotas/
installed app/pin/archive/catalog/defaults/profile gates stay unchanged; the main
import gap remains 106. No original controls/audio/graphics/pacing claim is made.

## Typed port event ownership component

Ports `service-kernel-events` now layers a checked 2,624-byte EVD2 ledger over the
scratch-backed tagged table. `Events::events_begin` reserves a row/actual-task
guard; `EventCreation::event_create` retains an actual SDK event, and
`event_publish` transfers ownership before fallible tag/output publication.
Null outputs retain owned orphans without consuming guest tags. Failed writes
retain accepted prefixes. Explicit transaction close releases an untransferred
event or leaves its transferred ownership with the domain. Typed state/set/reset,
same/fresh aliases and last-alias native close validate ledger, metadata kind and
actual SDK event authority. Creator-only domain shutdown journals/retries partial
close and retains revocation errors after actual cleanup. See the ports component
README for exact APIs, byte layout, sequencing and obligations.

This is checked game code using existing SDK services, not a trusted-host engine
or a ready kernel import. No SDK, quota, installed binary, pinned submodule,
public archive/catalog, renderer/default or profile-gate change is involved.
Four-target native/Wine components, ten interference cases per target, fourteen
required-failing native copies, eight separately optimized/audited game objects
per target and actual confined Linux32/64 fixtures pass. Maintainer runtime/fault
facades cannot be packaged as game code, even unused. No actual title, device,
audible audio, Windows confinement or measured frame-pacing claim follows.
Pulse/NT status mapping, new context/import/startup wiring and typed
file/thread ownership still remain; the main import gap stays 106.


## Typed event wait lifetimes

The ports event domain now uses EVD2 (2,624 bytes), strictly rejecting the older
2,112-byte EVD1 format. It appends 32 wait slots without enlarging any host quota.
`Events::events_wait_begin` retains an actual-task guard and privately owns an
80-byte WLT1 record. `EventWait::event_wait` publishes its ACTIVE lifetime before
releasing that guard and calling the typed SDK event wait for 0..1,000 ms. Other
tasks can signal the event; last-alias close and domain shutdown return Busy
while the lifetime is ACTIVE. Completion records the actual result after a late
KSV/cancellation check. Explicit `event_wait_close` can publish completion after
cancellation/revocation without reacquiring a cancellation-sensitive mutex.
It never repeats a wait to recover a failed completion CAS.

The domain owns accepted wait records and journals RETIRING before record release,
then clears the slot. Retry tolerates a missing record only after that explicit
retirement journal. Exact LE formats and cleanup obligations are documented in
ports `service-kernel-events/README.md`. These copied records are not immutable
native authority or joined-task proof; callers must stop/actually join borrowers.
This is one bounded SDK wait, not a completed Xbox timeout/alert/pulse translator.

The full event suite passes four-target native SDK execution (Windows through
Wine), ten interference cases per target, fourteen required-failing native copies,
all-eight-object admission/audit per target and actual confined Linux32/64 tests.
Actual waiting jobs cover signaling, cancellation, revocation, close prevention
and consumed joins. The 32-slot capacity control uses synthetic registered leases,
not 32 physical waiting jobs. Global record exhaustion, duplicate wait IDs and
unchanged synchronization quotas are checked. The installed SDK and app binaries
are unchanged. Full-title import gap remains 106; gameplay, device/audio/GPU and
measured pacing remain unverified by these component fixtures.

## Authenticated startup owns the typed object graph

Ports `KernelContext::kernel_context_new_with_objects` adds strict KIC3: exactly
80 LE bytes, magic 0x3343494b, root/KSV/ISR/shadow IDs at 8/16/24/32, HTR2 table
and EVD2 domain IDs at 40/48, reserved zero at 56/64/72. KIC1/KIC2 remain exact
64/72-byte formats. `kernel_context_objects` returns structurally bound copied
IDs, not native authority or leases. Actual typed consumers must validate full
table/domain representations and SDK kind/liveness. KPL4 and worker codecs stay
unchanged; no additional kernel import becomes ready from the new context alone.

`Startup::startup_prepare_with_objects` explicitly opts into owning the table,
event domain and KIC3 beside its existing owners. It retains unbound/partial
owners immediately, validates the exact owned pair when issuing ready borrows,
and requires consumed joins before shutdown. It drains typed events/wait records
before clearing accepted scratch backing. Incomplete cleanup retains obligations;
completed revocation cleanup retains its error. Exact APIs and byte contracts:
ports `service-kernel-context/OBJECT_STARTUP.md`.

Four-window preparation is explicit: `Layout::layout_services` exposes the
fourth opaque scratch ID without mapping it into guest aliases. Existing
three-window layout/allocator entry points stay strict. The checked game-owned
allocator adds private version-2 operation 19 through
`HeapClient::heap_contiguous_services`; actual-task membership, monotonic
sequence, primary-owner, exact topology/window and single-initialization checks
remain mandatory. Version 1 operation 19 stays invalid. This does not add an SDK
opcode or change host quotas/identity, shader handling or the installed binaries.
The public archive/catalog, app ports pin and blocked native profiles stay put.

KIC3 qualification passes 34 actual confined Linux positive startup sessions and
seven source-valid/audited broken-copy controls, plus all four optimized-object/
link targets. It covers real guest tags/aliases and tracked-worker orphans. The
legacy image suite is requalified on that exact source with 102 confined positive
sessions and 21 broken-copy controls. Handle/event, topology, allocator protocol,
shared-heap and legacy-context regressions also pass; standalone Windows component
execution uses Wine, while KIC3 image Windows scope is build-only. There is no
new physical input/audio/GPU/gameplay or measured-pacing evidence. The remaining
main import count is still 106, and no native title/profile is enabled.

## Actual checked native event imports

Ports now implement 189 NtCreateEvent (16 argument bytes) and 225 NtSetEvent
(8 argument bytes) in the actual checked `kernel_invoke` path. Both require live
KIC3 typed object bindings; KIC1/KIC2/context-free admission stays denied. Creation
preserves low-byte BOOLEAN conversion, four-byte tags, ignored ObjectAttributes,
null-output owned orphans and zero output on actual native-capacity failure.
Signaling writes PreviousState zero before the attempt and distinguishes ordinary
invalid handles from revoked ownership. CPU return changes are limited to EAX/ESP;
fatal/control-plane errors preserve CPU state and explicit accepted prefixes.
Exact status, write-order, lifetime and qualification contracts are in ports
`service-kernel-event-imports/README.md`.

The table/event implementations were consolidated beside that dispatcher in the
**checked game-owned ports kernel_pool.rs**, with the original public type paths
re-exporting those same types. Every game source/object remains checked/audited;
no engine code moved into trusted app code, SDK or transport. No enforcement,
quota, source/dependency allowance, SDK identity or builtin-profile gate changed.
New bounded LE serializers avoid rejected compiler helpers without adding an
object-import exception.

The native SDK/original-C comparison covers all four targets (Windows through
Wine), 1,764 exact result bytes, CPU frames, protected guest writes, actual event
capacity, inaccessible/corrupt table metadata, three SDK interference cases per
target and twelve broken native copies. Ordinary invalid tokens return NT status;
metadata/service failures remain errors and leave the CPU frame unchanged.
Signed fixtures audit eight separately optimized game objects per target and run
in actual confined Linux32/64 children. Authenticated image mode 68 separately
invokes both imports and matches those original-C bytes. Windows image scope
remains build-only. No game window, physical controls, audible audio, GPU fidelity
or measured pacing is established here.

The private ISO's actual KIC3 remaining-import set is now **104**, removing only
189/225 from the previous 106. KIC2 remains 106 and KIC1 107. NtClose/NT waits,
pulse, file/thread ownership and the full source/native loop/renderer/media/
builder/launcher/gameplay migration remain required; complete native profiles
stay blocked. The installed CLI/GUI already supply the matching host SDK.

## Counted child-local semaphores

The reviewed Takedawn source requires counted semaphores for NtCreateSemaphore
and NtReleaseSemaphore. Events cannot substitute for counts. The shared child
runtime now supplies generic typed semaphores, with no game engine logic in the
host. This does **not** implement those NT imports, their tagged ownership,
PreviousCount writes or NT wait/status conversion; those remain checked port work.

Generated Rust API:

- `local_semaphore_create(initial: u32, maximum: u32) -> Result<u64, i32>`
- `local_semaphore_release(id: u64, permits: u32) -> Result<u32, i32>`
- `local_semaphore_wait(id: u64, milliseconds: u32) -> Result<(), i32>`
- `local_semaphore_state(id: u64) -> Result<LocalSemaphoreState, i32>`

`LocalSemaphoreState` has copied `count: u32` and `maximum: u32` fields. Creation
requires `1 <= maximum <= 0x7fffffff` and `initial <= maximum`. Release requires
positive permits whose complete addition fits the maximum; it atomically changes
the count and returns the previous count under the same value lock. Failure does
not partially add permits or notify waiters. A successful typed wait consumes
exactly one permit. The semaphore shares the existing **128 total synchronization
object** quota with events and mutexes; the counter allocates no per-permit memory.
A count of 0x7fffffff has the same object storage as a count of one.

| Local op | Exact request, little endian | Result / minimum reply capacity |
| --- | --- | --- |
| 43 create | initial u32, maximum u32 (8 bytes) | opaque nonzero u64 / 8 |
| 44 release | ID u64, permits u32 (12 bytes) | previous count u32 / 4 |
| 45 wait | ID u64, milliseconds u32 (12 bytes) | empty / 0 |
| 46 state | ID u64 (8 bytes) | count u32, maximum u32 / 8 |

Trailing/truncated requests and short reply capacities reject before mutations.
Modes/counts are fixed-width on all four targets. Invalid counts, excessive waits,
wrong object kinds or over-release return Invalid (-1). Busy (-2) means the
unchanged quota is full or no permit was available by a wait deadline. Stale,
foreign-runtime or released IDs, runtime close and cancelled reviewed tasks
return Denied (-4). Internal lock failures return Failed (-3). Older SDKs return
Unsupported (-5); ports must not replace this with an OS call or emulate a count
with an event. Typed helpers validate exact reply length and count ranges.

Waits are worker-only and bounded to 0..1000 ms. They remain inside the confined
child, independent of the private broker transport; the window owner never waits
for a semaphore. Cancellation, object release and runtime close wake waiters.
The value lock pairs flag checks with the condition-variable sleep, and retained
object references keep storage valid through waking/close. A copied state or ID
confers no lease. Existing local op 25 revokes a semaphore; future calls through
that ID deny, and future objects never reuse it. Cancellation does not consume a
permit. Cleanup/revocation may still use existing op 25 after cancellation.

Only the typed semaphore wait consumes permits. Legacy untyped op 23 rejects
semaphores; event/mutex operations reject them, and semaphore operations reject
events/mutexes without changing state. The old operations' scope is not widened.
The local version remains 1 with additive operations; exact SDK identity, rather
than version 1 alone, must establish helper availability. These source/header
changes automatically change `build_identity()` and invalidate old cache receipts.
No source/dependency/object-audit, confinement, memory budget or native-profile
admission rule is relaxed. Ports still need a typed owner and complete release,
wait, shutdown and NT compatibility adapters before enabling actual imports.

Semaphore qualification uses `tools/check_local_semaphores.py --wine --evidence
<new-directory>` (maintainer tooling only) and the ordinary checked fixture
`tests/fixtures/service_local_semaphore.rs`. The former runs runtime, exact ABI
and malformed-reply tests on Linux32/64 and Windows32/64 through Wine; six altered
runtime copies must compile and then fail. Counts, wrong-kind isolation, shared
quota, concurrent release/acquisition, cancellation notification/value-lock
pairing, release/close wakeup and reply validation are covered. Windows component
execution through Wine is not physical Windows or Windows confinement evidence.

`local_semaphores_execute_without_owner_dispatch_and_keep_every_object_audited`
in `tests/service_preparation.rs` scans the signed dependency-free game before
binding injection, rejects even unused packaged runtime/provider implementations,
optimizes and audits every emitted game object before trusted linking on all four
targets, and executes confined Linux32/64 children. It completes local semaphore
operations and 33 reviewed-worker lifetimes before the owner services its final
diagnostic. Exact SDK/source-kit/compiler/target/object/executable identities are
sealed in the resulting receipts. The full existing preparation suite is also
requalified, including private pipes, read-only ROM/save grants, memory topology,
clock/deadline, join evidence and object auditing. Existing checked event imports
and authenticated KIC3 image startup are requalified against the changed SDK.
No physical controls, audible audio, rendered game frames or frame pacing are
established. The main KIC3 import gap remains **104**; native titles stay blocked.

### Checked Takedawn consumer checkpoint: counted ownership and KIC4

The reviewed ports feature branch adds untrusted, dependency-free `Semaphores`,
`SemaphoreCreation` and `SemaphoreWait` over SDK ops 43..46/25. SSD1 and SWL1 hold
full-u64 identities, creation/retirement prefixes and actual wait completion;
shared HTR2 kind 3 distinguishes them from kind-1 events. The host SDK, quotas,
installed identity and admission gates do not change for this port-only work.
Exact game-owned contracts live in ports `service-kernel-semaphores/README.md`,
`service-kernel-semaphore-imports/README.md` and
`service-kernel-context/SYNC_STARTUP.md` under `host-integration/engines/takedawn`.
They are reviewed integration inputs, not trusted host implementation or an
updated app submodule pin.

KIC4 explicitly owns/binds the shadow, tagged table, events and semaphores.
Actual NT imports 193/222 require the complete live KIC4 graph; older contexts
remain Unsupported. Startup retains partial owners and actual join obligations,
drains semaphore/event domains before table backing, and rejects substituted
copied identities. Later guest-write or observed revocation errors preserve any
accepted native count change and do not manufacture normal CPU success.

The original-C comparison uses pinned original function bodies with a synthetic
provider for documented Windows semaphore semantics. Rejecting invalid creation
or excessive/nonpositive release on Linux is an explicit correction to the
upstream POSIX shim's unchecked creation/clamping, not equivalence to that shim.
Source hashes and comparison limits are pinned in the ports witnesses.
Authenticated private-image startup still refuses 102 remaining main imports;
KIC3 refuses 104. Neither Burnout nor Halo is enabled by this checkpoint. Complete
generated-source admission, NT close/waits and other adapters, native entry/loop,
renderer/media, builders/launchers and actual gameplay qualification remain.

## Atomic child-local mixed-object waits

`local_wait_many(objects: &[LocalWaitObject], all: bool, milliseconds: u32)
-> Result<LocalWaitResult,i32>` adds bounded waits to the audited local runtime.
`LocalWaitObject { id: u64, kind: u32 }` uses kind 1 event, 2 recursive mutex,
3 counted semaphore and 4 reviewed task completion. These copied fields are
requests, not trusted type assertions or lifetime leases. Every actual object is
looked up in this child, retained, revalidated and locked before use. No native
thread handle, pointer, callback, system wait or cross-process endpoint escapes.

Local **op 47** has exactly three LE u32 fields `(count, all, milliseconds)`, then
`count` entries `(ID u64, kind u32)`, with no native padding. Count is 1..64,
all is exactly 0/1, milliseconds is 0..1000, IDs are nonzero and distinct. Requests
are exactly `12 + 12*count` bytes (24..780); reply capacity must be >=8 before
any acquisition. The eight-byte reply is `(index u32, reason u32)`, reason 0 normal
acquisition or 1 acquired abandoned mutex. Duplicate aliases of the same opaque
ID reject, even if the caller presents different kinds. Trailing/truncated
requests, invalid counts/modes/kinds/deadlines and short reply capacities reject
before any object state changes. The typed helper also rejects malformed reply
length, out-of-range index/reason, nonzero normal WaitAll index or abandonment
reported for a non-mutex entry. There is no OS fallback on Unsupported.

WaitAny acquires the lowest ready **caller-array** index, independently of numeric
ID/lock order. WaitAll acquires everything together and returns index 0 normally;
if abandoned mutexes were acquired it reports the lowest such caller index.
Failed/expired waits never reset auto-events, decrement permits, acquire mutexes
or consume task joins. Manual events stay signalled. A successful acquisition
resets each selected auto-event, consumes one selected semaphore permit and
increments each selected recursive mutex once. Every selected recursion increment
is preflighted before the first mutation. Mutex ownership uses the actual runtime
thread, never game-supplied owner IDs. All distinct value/completion locks use one
full-u64 ID order. No state lock or game callback is acquired while holding them.

A reviewed task's normal return (or maintainer-test unwind) marks only mutexes
still owned by that actual thread abandoned, clears their recursion and notifies
waiters. The explicit wait-set result acquires/reports/clears abandonment.
Legacy unit-returning local wait/mutex-lock operations return Denied for an
abandoned mutex **without acquiring it**; they cannot silently report ordinary
success. Checked ports must recover or stop/report their own protected metadata,
not discard the abandoned indication. User game objects compile with panic=abort;
a fatal process abort is not recoverable task return or mutex recovery evidence.

Task completion entries observe `JoinHandle::is_finished` under a retained
revocable observation lock. They neither join nor return the task's result.
The ordinary consuming join revokes those views and wakes pending wait sets;
a pending mixed set then denies before consuming ready synchronization objects.
The final host-only tail after the task-exit notification uses a sleeping recheck
of at most 1 ms, bounded by the original deadline. No game completion flag is
substituted for actual native thread completion. Completion never authorizes
Startup teardown: the existing consumed-join proof is still mandatory.

Only the confined child blocks. The broker/window owner never waits for this
operation. Signal, release, mutex unlock/abandonment, task exit/join, cancellation,
object revocation and runtime close notify a child-local epoch/condition variable.
Waiters snapshot its epoch before value inspection, release all values, then
compare-and-sleep under the epoch lock; notifications cannot vanish in that gap.
No path takes an object/state lock while holding the epoch lock. Known reviewed
task cancellation or runtime/object revocation returns Denied, including when a
permit is available. Busy is an unsatisfied bounded deadline (or recursion/quota
constraint); Invalid is malformed input/wrong sync kind, Failed internal failure,
Denied stale/foreign/wrong-namespace/revoked/cancelled, Unsupported older SDK.
A success concurrent with a later cancellation/revocation is an accepted prefix;
no rollback is invented. Retained locks/Arcs protect storage through the result.

The unchanged 128 synchronization objects, 32 task limit, stack/process budget and
opaque-record quotas apply. A call holds at most 64 copied entries/retained views
and a bounded guard list; existing fixed local bookkeeping reservation covers
these allocations for reviewed workers. Revocation removes a registry slot; an outstanding wait retains the old storage
until return. Each reviewed worker (plus bootstrap) can retain at most 64 entries
in its one synchronous call; repeated revoke/recreate cannot accumulate an
unbounded backlog. The fixed bookkeeping reservation covers this bounded retained
storage as well as the guard lists. It allocates no persistent wait object
or unbounded per-count storage. No new game dependency or admission exemption is
introduced. SDK identity changes invalidate old native receipts automatically;
rebuild every game object and trusted runtime for the exact SDK before reuse.

This is generic wait infrastructure, **not** NT import admission. Ports retain
handle resolution, event/semaphore/file/thread ownership and active-wait journals,
NT timeout/time-warp conversion, status mapping and guest CPU writes. They must
release table guards before calling this operation, retain every participant's
shutdown obligation, and verify cancellation/accepted prefixes. Alertable APC
execution, raw file waiting and game-specific pseudo handles are not provided by
op 47; do not silently ignore a required alertable/traversal path or turn a file
into a guessed event. Any remaining required service needs its own reviewed
contract and tests before the title or NT import can be enabled.

Maintainer qualification is `tools/check_local_waitsets.py --wine --evidence
<new-dir>`, plus `local_wait_sets_are_atomic_and_keep_every_object_audited` in
`tests/service_preparation.rs` (optional harness evidence path
`GAMEPKG_SERVICE_WAITSET_EVIDENCE`). The signed dependency-free fixture is scanned
before bindings, separately optimized and collectively object-audited before
trusted linking; even unused runtime/reply-provider source is denied. Linux32/64
execute actual confined workers without owner dispatch; Windows32/64 signed
artifacts are build-only. Native Windows components execute through Wine, not
physical Windows confinement. Synthetic concurrency/notification tests are not
gameplay, physical input, audible output, GPU fidelity or measured frame pacing.
The contract follows [Windows multiple-object acquisition rules](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitformultipleobjects);
NT game policy remains in the reviewed port.


### Takedawn checked RTL consumer checkpoint

The independent ports feature branch now supplies dependency-free checked-memory
adapters for actual imports 269/279/289/290/301. Their exact argument, CPU,
byte-order, C-locale folding, NUL, length-wrap, accepted-write-prefix and fault
contracts are documented in the independent ports [RTL contract](https://github.com/centrexity/GamePKG_Ports/blob/feature/takedawn-shared-services/host-integration/engines/takedawn/service-kernel-rtl/README.md). These are game
code, not a new trusted host layer. They use the existing SDK identity
`9801f83896bc4dc53f135f9fe169f27305323fe82f1b8fae5f578092792745d8`.

The authenticated KIC4 image fixture exercises all five through the prepared
import plan; four occur in the private main table, reducing its verified pending
count to 98. This does not admit that title, its complete source kit or renderer.
NT waits/close, combined typed ownership and alert/APC semantics, file/thread
adapters and full handler/loop/media/build/launcher/gameplay migration remain
required. App pinned ports inputs and blocked profile policy are unchanged.
Native Linux components/contained startup, Windows components under Wine and
Windows artifact-only checks do not establish physical controls, audible output,
original graphics or measured frame pacing. Existing local SDK installation
needs no rebuild for this port-only adapter batch.


### Takedawn combined event/semaphore ownership checkpoint

The independent ports [mixed-owner contract](https://github.com/centrexity/GamePKG_Ports/blob/feature/takedawn-shared-services/host-integration/engines/takedawn/service-kernel-mixed-waits/README.md)
now defines a checked EVD2/SSD1 transaction over existing op 47. It validates the
complete typed tag array, retains every participant before releasing its table
guard, performs one atomic all/any acquisition and preserves native outcomes
before fallible liveness/DONE work. Partial preparation and close keep explicit
ownership obligations; there is no serial WaitAll or fabricated rollback.

The port's HTR2/KSH1 guard cleanup also retains native-depth release separately
from count retirement. It releases the actual depth before the count-only CAS;
failed unlock retains the count, and stale retirement after successful unlock
cannot claim completion or unlock another guard. Count-only comparisons and
shadow root-page linking retry only uncommitted version conflicts, bounded to
64 attempts; native actions, reads and other SDK errors keep their strict
semantics. This is checked game code, not a host-runtime exemption or new capability.

Authenticated KIC4 mode 82 uses the actual create/set/release imports and owned
domains, checks CPU/caller state, mixed acquisition and alias removal. It does
not admit actual NT wait/close, mutex/file/thread owners, async overwrite,
alert/APC semantics or complete title execution. The KIC4 pending import count
remains 98. SDK identity, quotas, installed app binaries, app ports pin, public
source archive/catalog, defaults and blocked profiles remain unchanged. Native
components/Wine, confined Linux startup and Windows artifact checks establish no
physical controls, audible sound, original rendering or measured frame pacing.


### Takedawn retained logical-wait consumer

The independent mixed-owner API adds `mixed_wait_slice(&mut self,&Memory,all,ms)`
and `mixed_wait_timeout(&mut self,&Memory) -> Result<(),i32>` over unchanged
op 47. A slice retains every original ACTIVE lease only when the actual SDK
returns Busy and the subsequent live check succeeds. Mode stays fixed; success,
other SDK errors and post-call liveness failures prohibit another acquisition.
Final timeout publishes Busy without a native call; failed publication or live
checks remain errors, including Busy. Close retains cleanup obligations and never
replays acquisition. The one-shot API still finalizes Busy immediately.

These methods are checked port code, not new trusted runtime capabilities.
Guest timeout/time-warp, NT status/CPU, alert/APC and other resource ownership
remain required. SDK identity, installation, pinned ports and title gates are
unchanged. Component/Wine and confined fixture tests do not establish gameplay,
physical Windows execution, controllers, audible audio or frame pacing.

## Non-consuming child-local mutex state and typed retirement

SDK identity:
`49a313ff062e83e3bcbaadb125f4ace190847b0321cc25877e45f92023f0234a`.
Rebuild checked game objects and trusted runtime together; prior SDK receipts are
not current cache entries. The C/Rust local ABI remains version 1. Older local
runtimes return Unsupported for the new operations; there is no platform fallback.

`local_mutex_state(id:u64) -> Result<LocalMutexState,i32>` is **op 48**: exact
8-byte LE ID request, reply exactly 16 bytes, four LE u32 fields in this order:
`locked`, `owned_by_caller`, `abandoned`, `depth`. Booleans are exactly 0/1.
Locked is equivalent to depth != 0; owned implies locked; abandoned implies
unlocked. Depth may be any nonzero u32 for a locked mutex, including MAX (a later
recursive acquisition then returns Busy). Ownership is relative to the actual
calling runtime thread, never a supplied task number. No thread identifier,
address, OS handle, callback or writable view is returned.

The query retains and locks the actual synchronization object, rechecks runtime,
object revocation and actual reviewed-task cancellation, and verifies the real
mutex kind. It neither acquires/unlocks nor changes recursion or abandonment.
Events and semaphores return Invalid without consuming their state. Copied state
is not an ownership grant, lease, lifetime extension, or guarantee of later state.
Malformed requests/short reply capacity and wrong kinds are Invalid; stale,
foreign, closed or cancelled callers are Denied. Internal mutex inconsistency
is Failed rather than invented repair. The Rust client rejects malformed reply
lengths, noncanonical booleans and impossible field relationships. Disconnected
runtime denial precedes request parsing; malformed-request Invalid assumes a
live installed local runtime.

`local_sync_release(id:u64,kind:u32) -> Result<(),i32>` is **op 49**: exact 12-byte
LE ID/kind request and empty reply. Kind 1 event, 2 mutex, 3 semaphore; 0, task
kind 4 and all other values are Invalid. The caller's kind is a request, not a
trusted assertion. The actual registry object and kind are checked under its
value lock before removal. Wrong kinds change nothing. A held mutex returns
Busy, even to its owner: this operation does not replace op 24 unlock, release a
recursive depth, or synthesize abandonment. Inconsistent mutex bookkeeping is
Failed. Missing/foreign/closed objects or a closed runtime are Denied.

Typed retirement remains available after **task cancellation** for cleanup in a
live runtime; it grants no post-session access. Registry removal marks actual
storage revoked and notifies all waiters through the existing notification
epoch. Retained wait/query storage remains alive until its last runtime-owned
reference is dropped. It neither joins a task nor removes another namespace's
object. Quotas and accounting are unchanged. Retain an owning journal before
calling it; an unknown protocol error can follow an accepted retirement and is
not permission to invent rollback or discard the outstanding cleanup obligation.

Both operations run only in the confined child local runtime. They do not send
blocking work to the window owner or broaden the private broker transport. They
add no game dependencies, executable memory, raw GPU/platform objects or engine
exemption. The legacy generic release remains unchanged.

Qualification: `tools/check_local_mutex_state.py --wine --evidence <new-dir>`
runs four-target runtime/binding/client checks and 21 required-failing altered
copies. Actual public cancellation cleanup is tested separately from explicit
synthetic internal-state corruption and deterministic cancellation/revocation
prefix tests. `tools/check_local_mutex_ffi.py --wine --evidence <new-dir>` checks
real C/Rust local calls, unaligned reply buffers, untouched error outputs, full
64-bit IDs on i686, recursion, abandonment and typed retirement. This is **local
ABI interoperability**, not new private-broker-pipe or game execution evidence.

The signed `local_wait_sets_are_atomic_and_keep_every_object_audited` fixture now
uses both operations after complete source scanning and separate all-object
auditing; even unused runtime, malformed-reply and FFI providers are rejected.
Linux32/64 execute actual confined children; Windows32/64 signed artifacts are
build/audit-only. Native Windows components here use Wine. None establishes
physical controls, audible audio, original graphics or measured frame pacing.

The checked port still needs real typed mutex/file/thread ownership and NT/APC
integration. Toolkit mutant bridges are 192/221, but the current private main
import table does not import those two directly; required waits 233/234 remain
pending. This host prerequisite does not implement those imports, change the
98-pending KIC4 result, migrate a full renderer/media/launcher, or enable Burnout
or Halo. Prioritize demonstrated active paths when completing the port adapters.

The independent checked EVD2/SSD1 consumers now use typed op 49 for retiring
journal-owned IDs. Their new creator-cancellation regression reproduced the old
false-completion path (Denied query despite a live native object) and verifies
actual native absence after close/join. The provider triggers actual public task
cancellation after cleanup-guard acquisition; it is never admitted as game source.
Root/table obligations of a cancelled creator can still require outer runtime
teardown. This consumer correction does not establish complete title shutdown.


Before maintainer port checks, verify the selected app library against current
SDK source identity: `python3 tools/check_native_sdk_identity.py --app-lib RLIB`.
The tool links an identity probe to that exact library and independently hashes
all source inputs with the production SDK's length framing. It rejects a stale
cached library before qualification; a filename or caller-supplied expected SDK
is not proof. This is maintainer tooling, not an end-user Python requirement or
an admission alternative. The production builder still binds its actual compiled
SDK and exact audited objects into receipts.

### Checked Takedawn critical-section consumer (KIC5)

The independent ports integration adds dependency-free CSD1 critical-section
owners and an explicit KIC5 startup path for active imports 291/277/294. It uses
the unchanged SDK `49a313ff062e83e3bcbaadb125f4ace190847b0321cc25877e45f92023f0234a`:
actual child-local recursive mutex ownership/state, bounded waits, records and
typed retirement. This is game compatibility code, not new trusted host code.
See the ports repository's
`host-integration/engines/takedawn/service-critical/README.md` for exact layouts,
retained creation/wait obligations, errors and qualification commands.

The original shadow semantics leave guest critical-section bytes untouched,
initialize lazily and recursively acquire by actual task. Null is a validated
no-op. Unsafe exhaustion becomes explicit Busy under unchanged host quotas;
foreign release and abandonment deny. Enter retains one ACTIVE logical lease
across bounded slices, releases map guards before blocking, and preserves an
observed native acquisition independently from any later service error. Cleanup
never infers native absence from a query denied by cancellation.

KIC5 explicitly binds a separate owned critical-section map/domain into the
startup graph. Earlier KIC1..4 APIs remain unchanged and reject these imports.
Authenticated private-image KIC5 mode 84 has 95 pending imports; KIC4 mode 79
retains 98. These are image/import diagnostics, not executed title gameplay.
App SDK identity, installed binaries, pinned ports submodule and admission gates
remain unchanged. The checked helper and image fixtures do not complete the
full generated Rust, NV2A renderer/media or Linux/Windows launcher migration.

### Native 2D tick games

Signed engine-free core packages with `native_tick = true` use the normal
native Build and Play actions on Linux x86-64. The package supplies a safe
`game_tick(input: u32, state: u64) -> u64` and dependency-free game source.
The game gets its own Vulkan window and process. The separately selected Wasm
build remains available. This profile uses embedded assets and does not
provide direct device access, networking, or ROMs.

The generated SDK maps a 640×480 logical canvas into the game window. It
provides solid shapes, clip/reset, nearest and linear image sampling, and
encoded straight-alpha sampling. The runtime delivers focused input events
in order, advances the monotonic tick clock, and stages window preferences
and audio controls per tick. Fullscreen, resize previews, presentation mode,
links and the file picker use trusted input when player authorization is
required. The player can pick an import or export using a signed file policy.
The launcher handles paths and transfers bounded contents over a private
connection. Achievements refer to signed manifest indexes and are recorded
in the library. Saves use an isolated quota and atomic replacement.

Build verifies the signed kit, scans dependency-free source, injects the
SDK, and audits game objects before the trusted link. The local cache
authenticates the receipt and checks SDK, source kit, target and executable
identities before Play. The game worker releases inherited descriptors and
installs its syscall restrictions before entering game code.
