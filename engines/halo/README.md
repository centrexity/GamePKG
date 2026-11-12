# Halo: Combat Evolved (OpenCE)

The reviewed `opence-ce95f7de` profile builds the pinned OpenCE source as a native executable with its own game-only window. The selector package contains the reviewed launcher; it contains no Halo assets, disc image, maps, or OpenCE source. The player supplies their own Xbox disc image or existing maps folder.

## Original disc in place

The native file picker accepts an individual ISO even when its folder contains other games. The canonical selection is remembered per signed game and creator. Folder selection remains available for existing maps.

Patch `0011-gamepkg-read-only-disc-view.patch` mounts a bounded, immutable view of the selected XDVDFS image. Map reads use the original read-only descriptor. The loading picture is derived from the image in memory. Map decompression uses the original engine loader with its disposable cache partition mounted in RAM. No map or image copies persist on disk. The original extraction path remains available to standalone upstream builds that were not given the GamePKG disc-view option.

The reader validates directory links, names, duplicate entries and extent bounds before publishing the view. File positions belong to individual open handles. Reads, enumeration and metadata go through the existing host ABI; save files and logs stay in the private runtime folder. The cache partition is separate from persistent profiles and saves. The asset-free native fixtures are documented in [tests/README.md](tests/README.md).

## Native targets

Linux defaults to `linux-i686`, including on a 64-bit Linux host. This is an ordinary native 32-bit process using SDL3 and hardware OpenGL; it uses no Wine, WSL, Proton, or CPU emulation. The local Rev 2 test reached gameplay on an RTX 3090 through GamePKG Build and Play, with the ISO remembered across restarts and play time recorded.

`linux-x86_64` is blocked after real-data tests exposed pointer truncation and XDK graphics-layout problems. Linking and rejecting a dummy map did not establish playability. Keep the unfinished LP64 experiments out of the reviewed patch set. Windows targets retain their existing cross-build records; current qualification uses Linux only.

## Input and multiplayer

The native engine owns input through SDL3. A virtual Xbox controller exercised the unchanged input backend: buttons, movement and aim sticks, firing, rumble, and disconnect/reconnect during a live match. A physical Bluetooth controller has not been tested in this local session.

The signed selector requests networking. An offline selector can omit that permission. Linux retains its read-only filesystem view, bounded RAM cache and memory cap in both cases; offline processes additionally receive a separate network namespace. Missing isolation tools stop launch rather than silently dropping protection.

Two native GPU-backed processes passed the upstream local-network comparison on Blood Gulch with movement, hits, deaths and synchronized state. Campaign loading was also exercised. This does not qualify every map, full campaign progression, co-op, internet traversal, or physical-controller behavior.

## Build integrity and remaining work

The catalog pins the source commit and license digest. Reviewed patches are applied after checkout. The incremental builder runs after source or target changes. Cache paths include the exact reviewed record and patch revision, so an app update cannot reuse an older engine that lacks the new disc reader. The cache partition receives 1 GiB of RAM within a combined 4 GiB maximum budget; the remaining 3 GiB is the process address-space cap. The recommended total is 3 GiB. A real GamePKG launch verified the live RAM mount, active gameplay, and zero persistent map or disc copies after exit. Graphics-driver caches remain writable under the private runtime home. GamePKG creates the cache base before launch, using directory handles that reject symlink redirects; real launches verified that NVIDIA cache files persist across restarts. Normal GamePKG launches require a game window: a graphics initialization error exits with a clear diagnostic instead of silently running without a picture. Explicit maintainer headless runs remain available.

The host implementation stays generic: commands, layouts, targets and launch environment come from `engine.toml`; host Rust does not name this game. The older source patches retain their existing pointer, layout, clock, thread, logging and directory ABI adaptations.

The Linux player build uses the host's Rust preparation driver and native compiler tools; it does not run Python. Maintainers export the reviewed Ninja graph, generated-header templates, dependency inventory and resource identities with `tools/prepare_opence_native_plan.py`. Resource bytes still come from the pinned source, and the original maps still come from the player's image. The driver verifies downloads, extracted dependency files, reconstructed resources and final link symbols. It drains compressed dependency streams through their trailers, rejects escaping archive links, and keeps generated outputs inside the source tree.

A clean source/dependency build and live gameplay passed with Python commands deliberately blocked. A second unchanged Build reused the compiled files. The preparation receipt binds dependency outputs to the plan, compiler, CPU and build environment. Fresh configuration discards older CMake options; compiler changes invalidate every native translation unit, including wrapped Ninja rules. The exporter checks those dependencies through Ninja's parsed graph. The existing Windows build records have not received this replacement or local gameplay qualification.

The host mute setting and zero audio volume select the reviewed silent audio driver while the engine continues decoding and advancing its audio. A native probe verified normal ALSA selection, muted playback and zero-volume playback without emitting sound.

Linux requires Clang, LLVM tools, CMake, Ninja, pkg-config, xz and wayland-scanner, plus the host's 32-bit graphics and audio drivers. The reviewed compiler sysroot and SDL source are downloaded and verified by the native preparation driver.

Optimization needs measured CPU/GPU frame times and unchanged graphics; no fixed frame-rate claim is made from these functional tests.

Patch `0012` parallelizes original HUD-pixel upscaling with SSE2 on supported CPUs and keeps a scalar path elsewhere. It preserves the Catmull-Rom tap order, original texel centers, channel order, clamping and rounding. The asset-free fixture compares the complete result and floating-point flags against the pinned original, including native 32-bit and sanitizer runs. It changes no artwork, layouts or rendering settings.

Upstream is AGPL-3.0-or-later. That license does not license Halo assets or Microsoft's trademarks. The selector text is Apache-2.0.
