# Halo: Combat Evolved (OpenCE)

The reviewed `opence-ce95f7de` profile builds the pinned OpenCE source as a native executable with its own game-only window. The selector package contains the reviewed launcher; it contains no Halo assets, disc image, maps, or OpenCE source. The player supplies their own Xbox disc image or existing maps folder.

## Original disc in place

The native file picker accepts an individual ISO even when its folder contains other games. The canonical selection is remembered per signed game and creator. Folder selection remains available for existing maps.

Patch `0011-gamepkg-read-only-disc-view.patch` mounts a bounded, immutable view of the selected XDVDFS image. Map reads use the original read-only descriptor. The loading picture is derived from the image in memory. Map decompression uses the original engine loader with its disposable cache partition mounted in RAM. No map or image copies persist on disk. The original extraction path remains available to standalone upstream builds that were not given the GamePKG disc-view option.

The reader validates directory links, names, duplicate entries and extent bounds before publishing the view. File positions belong to individual open handles. Reads, enumeration and metadata go through the existing host ABI; save files and logs stay in the private runtime folder. The cache partition is separate from persistent profiles and saves. The asset-free native fixtures are documented in [tests/README.md](tests/README.md).

Patch `0014` extends that read-only view to the original `bink` directory and restores prerecorded movies on the reviewed Linux target. A minimal, pinned FFmpeg decoder reads bounded ranges through the same disc descriptor. Video and decoded audio stay in memory. The original game renders the frames; SDL3 supplies timed audio through the selected device. Movie audio follows the app's mute/volume settings, and skipping or closing a movie releases its decoder and audio stream. Legacy targets keep their original backend. An existing maps-only folder can still play maps, but cannot supply disc movies that it does not contain.

The bridge uses the native compiler/library ABI in its own translation unit. It preserves the game's floating-point environment, starts timing after the first frame is submitted, presents the final frame, and carries fractional audio samples across loops. All five Rev 2 movies matched separate reference decodes for video and trimmed, interleaved stereo PCM samples locally; the complete intro also played through the normal GamePKG launch on the private RTX 3090 display. This does not establish a complete campaign playthrough or subjective audio quality. See [the decoder notices](FFMPEG-NOTICE.md).

## Native targets

Linux defaults to `linux-i686`, including on a 64-bit Linux host. This is an ordinary native 32-bit process using SDL3 and hardware OpenGL; it uses no Wine, WSL, Proton, or CPU emulation. The local Rev 2 test reached gameplay on an RTX 3090 through GamePKG Build and Play, with the ISO remembered across restarts and play time recorded.

`linux-x86_64` is blocked after real-data tests exposed pointer truncation and XDK graphics-layout problems. Linking and rejecting a dummy map did not establish playability. Keep the unfinished LP64 experiments out of the reviewed patch set. Windows targets retain their existing cross-build records; current qualification uses Linux only.

## Input and multiplayer

The native engine owns input through SDL3. A virtual Xbox controller exercised the unchanged input backend: buttons, movement and aim sticks, firing, rumble, and disconnect/reconnect during a live match. A physical Bluetooth controller has not been tested in this local session.

The signed selector requests networking. An offline selector can omit that permission. Linux retains its read-only filesystem view, bounded RAM cache and memory cap in both cases; offline processes additionally receive a separate network namespace. Missing isolation tools stop launch rather than silently dropping protection.

Two native GPU-backed processes passed the upstream local-network comparison across all 13 original multiplayer maps in both hosting roles. A longer `a30` co-op comparison covered 184 jointly active seconds in each hosting role without relaxed thresholds. All ten campaign levels reached their active loop in loading checks. These checks do not qualify full campaign progression, internet traversal, or physical-controller behavior; the automated `a10` intro/cryopod bot scenario remained inconclusive.

The movie-enabled build passed a matched Clang 18/SDL comparison for 124 jointly active seconds in each co-op hosting role. An earlier comparison mixing Clang 21 and Clang 18 game/SDL builds exceeded the position comparator in one role while its movement-drift check passed; the reverse role passed. Holding SDL identical also passed both roles. Mixed-build interoperability remains under investigation, so this preview does not claim complete co-op qualification. The original comparison thresholds remain unchanged.

Directed controller input completed the opening's normal and inverted look calibration and its A/B confirmation in a one-host test. Two-peer opening comparisons also exposed an aim mismatch with identical unoptimized builds; campaign progression and co-op aim remain unqualified. Patch `0015` independently fixes a confirmed omission: the host's scripted suspension flag now reaches each client's player through persistent unit snapshots. The original engine setter runs on transitions, preserving its velocity and airborne effects without resetting them on every snapshot. Asset-free snapshot/transition tests pass on native 32-bit and sanitized 64-bit code. The normal app built and launched this revision without Python, played the complete intro, remembered the in-place ISO after GUI restart, and reused the unchanged build. With this revision on both peers, a30 co-op passed 124 jointly active seconds per hosting role; Blood Gulch multiplayer with damage, deaths and respawns passed 64 seconds per role under the original comparator thresholds. These checks do not establish full campaign completion. Network version 33 requires matching updated peers for this scripted state; real version 32/33 peers explicitly refused one another in both hosting roles.

Patch `0016` makes the original campaign recognize network co-op when each machine has one local player. The old helper checked only the local split-screen setting. Read-only inspection of the original mission confirms that its cooperative branch wakes `fast_setup`, while its normal solo branch wakes `tutorial_setup`; the patch changes the mode check and preserves those scripts. The normal app built and launched this revision, remembered the in-place ISO after GUI restart, and reused the unchanged build. An a30 comparison passed 124 jointly active seconds per hosting role, and Blood Gulch multiplayer with damage, deaths and respawns passed 64 seconds per role on this same revision. A directed opening co-op probe with only observational logging passed 604 paired seconds under the unchanged state-comparison thresholds; both ordinary controllers voted to skip the intro and moved through the starting room without a recorded death. This does not establish full opening progression. A separate unskipped-intro control with identical SDL libraries reproduced the old revision’s 167-degree aim divergence; this revision also failed at the same tick with 168 degrees. Correct co-op mode and cinematic view-state synchronization are independent issues.

## Build integrity and remaining work

The catalog pins the source commit and license digest. Reviewed patches are applied after checkout. The incremental builder runs after source or target changes. Cache paths include the exact reviewed record and patch revision, so an app update cannot reuse an older engine that lacks the new disc reader. The cache partition receives 1 GiB of RAM within a combined 4 GiB maximum budget; the remaining 3 GiB is the process address-space cap. The recommended total is 3 GiB. A real GamePKG launch verified the live RAM mount, active gameplay, and zero persistent map or disc copies after exit. Graphics-driver caches remain writable under the private runtime home. GamePKG creates the cache base before launch, using directory handles that reject symlink redirects; real launches verified that NVIDIA cache files persist across restarts. Normal GamePKG launches require a game window: a graphics initialization error exits with a clear diagnostic instead of silently running without a picture. Explicit maintainer headless runs remain available.

The host implementation stays generic: commands, layouts, targets and launch environment come from `engine.toml`; host Rust does not name this game. The older source patches retain their existing pointer, layout, clock, thread, logging and directory ABI adaptations.

The Linux player build uses the host's Rust preparation driver and native compiler tools; it does not run Python. Maintainers export the base Ninja graph, generated-header templates, dependency inventory and resource identities with `tools/prepare_opence_native_plan.py`, then extend it with `tools/prepare_opence_movie_plan.py` using the verified FFmpeg archive, configured source and verbose build log. The latter checks all decoder compiler nodes and the separate bridge ABI through Ninja's parsed graph. Resource bytes still come from the pinned source, and the original maps and movies still come from the player's image. The driver verifies downloads, extracted dependency files, reconstructed resources and final link symbols. It drains gzip/XZ dependency streams through their trailers, rejects escaping archive links, and keeps generated outputs inside the source tree. Unused FFmpeg test-map metadata is excluded from extraction; the engine source tree still rejects game maps and movie files.

A clean source/dependency build and live gameplay passed with Python commands deliberately blocked. A second unchanged Build reused the compiled files. The preparation receipt binds dependency outputs to the plan, compiler, CPU and build environment. Fresh configuration discards older CMake options; compiler changes invalidate every native translation unit, including wrapped Ninja rules. The exporter checks those dependencies through Ninja's parsed graph. The existing Windows build records have not received this replacement or local gameplay qualification.

The host mute setting and zero audio volume select the reviewed silent audio driver while the engine continues decoding and advancing its audio. A native probe verified normal ALSA selection, muted playback and zero-volume playback without emitting sound.

Linux requires Clang, LLVM tools, CMake, Ninja, pkg-config, xz and wayland-scanner, plus the host's 32-bit graphics and audio drivers. The reviewed compiler sysroot and SDL source are downloaded and verified by the native preparation driver.

Maintainer recipe export uses a separate ordinary-i686 FFmpeg build, never the game's XDK alignment flags. Configure the verified FFmpeg 9.0.2 archive with the following options, replacing the sysroot path with the reviewed compiler sysroot:

```sh
./configure --prefix=../install32 --arch=x86_32 --target-os=linux \
  --cc='/usr/bin/clang-18 --target=i686-linux-gnu -m32' --sysroot=/path/to/reviewed/sysroot \
  --disable-autodetect --disable-everything --disable-programs --disable-doc --disable-debug \
  --disable-network --disable-avdevice --disable-avfilter --disable-x86asm --disable-inline-asm \
  --enable-static --disable-shared --enable-decoder=bink,binkaudio_rdft,binkaudio_dct \
  --enable-demuxer=bink --disable-protocols
make -j2 V=1 > /path/to/ffmpeg-build.log 2>&1
```

Export the base plan from the configured, patched OpenCE checkout, then extend that output before reviewing or committing it:

```sh
python3 tools/prepare_opence_native_plan.py --source /path/to/engine --output /tmp/native-plan
python3 tools/prepare_opence_movie_plan.py --source /path/to/engine --output /tmp/native-plan \
  --ffmpeg-source /path/to/ffmpeg-9.0.2 --ffmpeg-archive /path/to/ffmpeg-9.0.2.tar.xz \
  --ffmpeg-build-log /path/to/ffmpeg-build.log
```

These commands are for maintainers. The player build consumes the resulting compiled-in data and invokes native compiler tools directly.

Optimization needs measured CPU/GPU frame times and unchanged graphics; no fixed frame-rate claim is made from these functional tests.

Patch `0012` parallelizes original HUD-pixel upscaling with SSE2 on supported CPUs and keeps a scalar path elsewhere. It preserves the Catmull-Rom tap order, original texel centers, channel order, clamping and rounding. The asset-free fixture compares the complete result and floating-point flags against the pinned original, including native 32-bit and sanitizer runs. It changes no artwork, layouts or rendering settings.

Upstream is AGPL-3.0-or-later. That license does not license Halo assets or Microsoft's trademarks. The selector text is Apache-2.0.


## Scripted camera and clock synchronization

Patch `0017` carries the original seven scripted view vectors while cooperative
player input is disabled. The local camera uses the inverse of the original
player-specific aiming conversion, preserving vehicle-up and seat-camera rules;
a full animated parent-node inverse uses a different coordinate basis. Ordinary
active local camera input remains local.

Scripted position handoffs remain authoritative until the client reaches the
host's exit position. This prevents delayed pre-exit predictions from pulling
the player back into a vehicle. The tolerance is 1/32 world unit, and dead,
replaced, absent and non-cooperative players retain their original isolation.

The client clock retains its host-projected ahead budget and can execute one
extra complete simulation tick at most every 250 ms to recover time lost during
loading. Catch-up requires a fresh host message, valid measured round trip, a
positive tick request and a gap beyond the two-tick quantization allowance. It
never rewinds time or skips simulation. Normal host, solo and menu timing is
unchanged.

Native Linux tests identified a persistent client lag of about eleven ticks;
matching input records differed only by angle quantization. Bounded catch-up
removed that persistent lag. The preceding camera/clock candidate passed 124
paired seconds on the second campaign map in each hosting role under unchanged
comparison thresholds. The corrected vehicle-basis candidate passed 182/184
paired seconds in the first map with simulated receive delay and packet loss;
both client roles completed a four-unit first walk without a death or deep
fall. A preceding clock candidate also passed a bounded host-stall reproduction.
These checks do not establish complete campaign play, Windows playback or a
whole-game frame-time guarantee.
A later maximum-stick looking run with the final camera candidate passed one
hosting role and recorded a short remote-aim disagreement in the other despite
aligned clocks. Its original failed comparison is retained. Patch `0018`
replicates the seven angular-state vectors to remote cooperative avatars during
active play, preserving the host's aiming integrator state. The writer excludes
the active local owner; scripted owner cameras still receive their correction.
This uses the existing optional payload without adding wire fields. Multiplayer
and solo behavior remain outside the cooperative correction path.

With this patch, the same continuous-stick stress test passed 124 paired seconds
in each hosting role under unchanged thresholds, with zero measured position
drift. The reversed role retained two brief aim mismatches (largest 22 degrees)
within the comparator's existing allowance. This is a scoped improvement, not a
claim of perfect synchronization or complete campaign qualification.
The same revision also passed 184 paired seconds of the first-map opening in
each hosting role with 100 ms receive delay and 1% packet loss. Both clients
completed a 4.023-unit first walk without a death or deep fall after movement
began. Brief opening-state mismatches remain in the retained comparisons.

Network version 34 separates these extended snapshots from older peers. Native
version 33/34 test peers explicitly refused each other in both hosting roles.
The asset-free codec and runtime regression fixtures are described in
[tests/README.md](tests/README.md).
