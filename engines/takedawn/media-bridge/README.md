# Private Takedawn media integration

This is the host-owned disc transport used by the native Linux preview described
in [the Rust port notes](../rust-port/README.md). It contains no player assets,
generated game code, external executables or keys. Original disc files are read
in place through the verified Rust reader. Generation/conversion tools are
maintainer-only; players use the prepared source kit.

## Windows transport preparation

`windows_file.c` provides read-only Windows file handles over the same verified
media API. Real waitable handles preserve synchronous file completion. The core
supports reads, independent positions, explicit-offset reads, 64-bit offsets,
metadata, sharing checks, close/cleanup and read-only rejection. Directory
searches have independent verified-reader cursors and native Windows DOS
wildcard matching; they create no placeholder files. Rename/delete/create and
handle-based mutation calls also reject disc writes. Virtual paths
reject traversal, alternate streams and malformed components before any host
fallback. The underlying Rust reader remains bounded; the adapter keeps logical
positions beyond EOF without asking that reader to access bytes beyond a file.
The reviewed original kernel opens buffered synchronous handles. Explicit
asynchronous/unbuffered handle flags reject rather than silently changing their
meaning; they are outside the current adapter's supported call profile.

`tools/check_takedawn_windows_file.py` compares reads, seeks, zero-byte reads,
short reads, EOF and overflow with actual Windows file APIs over synthetic data.
It also checks offsets above 4 GiB, blocked-read signaling, metadata-only handles,
directories, sharing, integrity failures, path rejection and handle exhaustion.
Initial GNU 32/64-bit tests passed in a private maintainer Wine runner. Native
MSVC 32/64-bit CI subsequently exposed Wine cursor differences: zero-byte reads
and failed explicit-offset EOF reads must preserve the prior position. The
adapter follows actual Windows behavior, and the native oracle keeps checking
those cases. Wine results alone cannot qualify this adapter. Native MSVC 32-bit
and 64-bit tests pass in workflow run
[40024528035](https://github.com/centrexity/GamePKG/actions/runs/40024528035),
alongside the Linux Rust stream tests. This is an adapter qualification fixture,
not a full Windows game test. It follows the
[Windows file-position contract](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfilepointerex);
backing-filesystem maximum offsets are separate from API arithmetic overflow.
The expanded directory/mutation oracle also passes on native MSVC 32/64-bit in
[40025766359](https://github.com/centrexity/GamePKG/actions/runs/40025766359).
It compares 17 literal/wildcard/case/Unicode search patterns with actual Windows
directory results, along with independent cursors, closed search rejection,
metadata, disc mutation rejection, integrity failures and resource cleanup.
Search objects remain distinct from waitable file handles, including after
close. Wildcard translation references the .NET runtime's
`System.IO.Enumeration.FileSystemName.TranslateWin32Expression`; the MIT notice
is retained in `DotNET-RUNTIME-LICENSE.txt`. Native Windows oracle behavior is
decisive where a reference algorithm or Wine differs. Matching uses Microsoft's
[RtlIsNameInExpression](https://learn.microsoft.com/en-us/windows/win32/devnotes/rtlisnameinexpression).

`adapt_windows_file.cmake` stages the exact pinned original `kernel_file.c`,
routing all 53 calls across 19 reviewed Win32 APIs through this adapter. The
stager never edits upstream and refuses changed revisions. The maintainer gate
also reverses the routing and demands byte-for-byte original source equality,
checks in-place/modified-input rejection, and compiles the staged original:

```sh
python3 tools/check_takedawn_windows_kernel.py --source /path/to/pinned/xboxrecomp
python3 tools/check_takedawn_windows_kernel.py --source /path/to/pinned/xboxrecomp --target i686-pc-windows-gnu
```

Both GNU cross-target compile gates pass locally. Native MSVC 32/64-bit staging,
compile and expanded directory/mutation tests all pass in
[40025914890](https://github.com/centrexity/GamePKG/actions/runs/40025914890).
CI checks out the exact upstream commit and uses its canonical source bytes.
Python remains a maintainer test tool.

`windows_stream.c` connects startup reads and SDL streams to those same handles.
It supports complete XBE reads, 64-bit sizes/seeks, partial SDL objects, EOF,
overflow/write rejection and unchanged outputs on failed loads. It keeps CRT
`FILE` objects out of DLL boundaries. `windows_entry.c` admits the selected ISO
through `GAMEPKG_TAKEDAWN_DISC_HANDLE` before calling the original main. The
strict decimal handle is duplicated with read-only access and without inheritance;
the verified reader owns that duplicate. Successful startup closes the child's
original inherited copy. Invalid, empty or failed handle admission stops startup
without falling back to a pathname. The legacy maintainer path remains available
only when the handle variable is absent, using the Unicode Windows environment;
extraction/data-folder overrides reject. These asset-free tests, including
malformed Unicode and admission failure/recovery, pass on actual MSVC 32/64-bit
in [40028221745](https://github.com/centrexity/GamePKG/actions/runs/40028221745).

`attach.cmake` selects `windows_attach.cmake` for Windows. Pinned staging connects
the original startup loader/report, SDL font read, file runtime and separate HDD
cache directory. Linux attachment behavior is retained. The original Windows
runtime now links successfully with the complete 486-unit Rust game archive and
its standard library. All 24,032 Rust exports remain in the 59,878,912-byte native
executable. Build commands compile no generated C game bodies and invoke no
Python. The original upstream checkouts remain unchanged. Official dependency
versions, source commits and download digests are in `../windows-dependencies.json`.

The first isolated Wine 9 diagnostic confirms the runtime identity, ISO
admission, a 4,308,992-byte XBE load, original intro execution, and Vulkan device
creation on an RTX 3090. Wine needs its display connection even for this
headless Vulkan check; omitting it produces a CPU fallback and does not qualify
Vulkan. Both boots were externally bounded and intentionally ended, with
private saves, a read-only ISO and no network.

The same native executable now passes the original Xbox-layout race control
assertions and PS2-layout Crash impact/results/Continue assertions under Wine.
It creates and reloads the original 59,904-byte profile plus 54-byte metadata,
preserving save hashes, and captures 45 seconds of non-silent 48 kHz stereo.
Captured race frames show the original car/world/HUD. A separate fresh-prefix
desktop check loads that profile using keyboard input, reaches the main menu,
resizes the Vulkan window to 1024x768 and closes normally with exit status zero
and original renderer teardown. Its 37-second run presents at about 59 frames/s
on the RTX 3090; this is one diagnostic run, not a general performance claim.
The window test confirms focus, uses bounded noninteractive captures and always
releases its generated keys, including when closing destroys the window.

Maintainer reproductions (Wine/Python are not player requirements):

```sh
GAMEPKG_GAMEPLAY_WINE=1 bash tools/check_takedawn_gameplay.sh /path/to/game.exe /path/to/owned.iso /path/to/pinned/takedawn /path/to/private/reference-saves xbox race
GAMEPKG_GAMEPLAY_WINE=1 bash tools/check_takedawn_gameplay.sh /path/to/game.exe /path/to/owned.iso /path/to/pinned/takedawn /path/to/private/reference-saves ps2 crash
GAMEPKG_GAMEPLAY_WINE=1 bash tools/check_takedawn_session.sh /path/to/game.exe /path/to/owned.iso
python3 tools/check_takedawn_windows_window.py /path/to/game.exe /path/to/owned.iso /path/to/private/reference-saves
```

The updated private Windows executable retains all 24,032 Rust exports. A separate
isolated Wine boot transfers an inherited read-only ISO handle while deliberately
setting an invalid legacy filename. It verifies all 716 files, loads the XBE,
initializes Vulkan and reaches 1,721 presented intro frames before an intentional
35-second end. This qualifies the handle handoff, rather than a completed race
or the app's process owner. The earlier gameplay checks above used the first build.

Native Windows CI also runs the actual Rust disc-reader/media tests: 17 pass
on each MSVC architecture and 16 on Linux in
[40035038063](https://github.com/centrexity/GamePKG/actions/runs/40035038063).
They verify read-only duplication, caller ownership, rejection of non-file handles,
and continued reads from an opened ISO after its pathname is replaced. Entry tests
also cover malformed handle values, admission failure and source-handle closure.
Those actual Windows API/reader tests and Wine gameplay cover distinct scopes.
Actual Windows-host game/hardware checks, full campaign progression, physical
controllers and audible fidelity remain unqualified. The Windows app's native
builder, launcher/process owner, signed-package integration and kit promotion
remain required. The player kit stays at its verified Linux v8 pin.

The private v9 candidate packages the Windows transport/stagers, dependency pins
and notices alongside the prepared Rust. Its archive SHA-256 is
`d448d64f12640a0d71914b9fc5ab6842748510aa39e2f9ed2ca31f14e837f739`:
55,181,802 compressed bytes, 668,157,299 unpacked bytes, 4,249 files. The actual
Rust source-kit loader accepts it. Linux regression builds retain all 24,032
exports, build media dependencies offline with an empty Cargo home, and preserve
every packaged source hash. They pass the original race, PS2 Crash and profile
create/reload/45-second audio gates. Of 486 Linux Rust units, 471 reuse individually
verified identical source/compiler/flags/object checkpoints and 15 compile anew.
This candidate is not yet the normal signed-package app build or player pin.

## Earlier reference bring-up

The remaining notes retain earlier reference measurements. The current prepared
Rust game, signed-package Build/Play and Linux gameplay qualification live in
the Rust port notes linked above.

The static library reuses src/xbox_disc.rs, src/xbox_disc/media.rs and
src/memory_budget.rs by source path. Keep their tests enabled. media.h describes
the trusted C pointer and buffer contract. Nothing in this directory changes
the guest crate policy or grants packages raw filesystem access.

From the repository root:

```sh
cargo test --offline --release --manifest-path engines/takedawn/media-bridge/Cargo.toml
cargo build --offline --release --manifest-path engines/takedawn/media-bridge/Cargo.toml
cargo clippy --offline --release --all-targets --manifest-path engines/takedawn/media-bridge/Cargo.toml -- -D warnings
cc -D_DEFAULT_SOURCE -DCHECK_POSIX -std=c11 -O2 -Wall -Wextra -Werror -pthread \
  engines/takedawn/media-bridge/check.c engines/takedawn/media-bridge/posix.c \
  engines/takedawn/media-bridge/target/release/libgamepkg_takedawn_media.a \
  -Wl,--wrap=open,--wrap=close,--wrap=read,--wrap=write,--wrap=lseek,--wrap=fstat,--wrap=stat,--wrap=lstat,--wrap=access,--wrap=opendir,--wrap=readdir,--wrap=closedir,--wrap=fopen \
  -ldl -lm -o /tmp/gamepkg-takedawn-posix-check
/tmp/gamepkg-takedawn-posix-check '/path/to/supported.xiso.iso'
```

After configuring the full pinned private reference tree and its dependencies
as described in the parent README, reconfigure that same build with
`-DCMAKE_PROJECT_TOP_LEVEL_INCLUDES=/absolute/path/to/media-bridge/attach.cmake`.
Build the burnout3 target. The original reference executable is preserved;
the attached executable is named burnout3-gamepkg-media. The original game
generation and content are unchanged. The adapter currently supports Linux only.
The CMake attachment stages a copy of the exact pinned kernel path source:
general Partition1 paths go under save_dir/Partition1 and its Cache directory
exists before the title runs. UDATA/TDATA precedence is retained. The original
source is never edited; its SHA-256 must match before staging. Validate this with:

```sh
cmake -DINPUT=/path/to/pinned/xboxrecomp/src/kernel/kernel_path.c \
  -DWORK=/tmp/fresh-gamepkg-path-check \
  -P engines/takedawn/media-bridge/check_adapt_paths.cmake
```

WORK must be a fresh path; the gate never deletes or reuses an existing folder.

For a private diagnostic boot, set GAMEPKG_TAKEDAWN_DISC to the read-only image,
RECOMP_SAVE_DIR/XDG_DATA_HOME/XDG_CACHE_HOME to fresh private writable scopes,
and pass `--headless /gamepkg-disc/default.xbe`. Use an external process timeout;
upstream's --seconds does not exit its headless backend. Run with host filesystem
and networking isolation (the first local runs used bwrap with separate /state,
read-only runtime directories, a single /disc.iso binding and --unshare-net).
The adapter itself is not a sandbox. Do not run it against real saves or assume
it enforces GamePKG memory/time/network rules yet.

The first real boot confirms XBE/font/movie reads and visible Vulkan intro and
loading frames. The HDD adaptation corrects the observed LocalCache write
failures without making the DVD writable. Fresh boots confirm both cache files,
title/profile/main/mode/car menus and training video. A subsequent 180-second
bounded run drives the first race, showing the original car/world/HUD and crash
recovery. Its original BO3_DRIVE_LOG records 6,611 race frames, maximum62.7622m/s,
3,778 full-throttle frames,120 full-brake frames and1,799 boost-button+throttle frames.
Those original hooks are observational; no AI/autopilot or gameplay replacement
is enabled. Both left/right stick directions are scripted but not recorded in
the speed log, so steering equivalence remains a separate gate.

For these checks, BO3_DRIVE_LOG and BO3_CTL_LOG point into the private /state
scope. RECOMP_FB_DUMP records private diagnostic frames periodically. The full
first-race input sequence in milliseconds from the first pad poll is:

```text
15000:start:300,18000:start:300,22000:start:600,26000:a:400,30000:a:400,34000:up:200,36000:a:400,42000:down:200,46000:a:400,50000:a:400,54000:a:400,58000:a:400,62000:a:400,85000:rt:30000,115000:rt+lright:1500,120000:rt+a:15000,140000:lt:2000,145000:rt+lleft:1500,150000:rt+a:15000
```

This is an observed private diagnostic sequence, not a portable timing-based
acceptance test. That initial race run continues without autosave. The separate
event/frame-driven session gate below now verifies normal profile save/reload.
Further checks must complete progression/autosave/restore and audible audio, audit remaining fd/volume
operations, and compare against the original. Never route writes back into the
ISO or make disc paths a fallback for arbitrary host paths. Production budget,
deadline, permission and signed package integration remain open, as do the
native no-Python generator and measured optimizations across all current ports.

## Renderer test setup and remaining discrepancy

The attachment also stages the exact pinned nv2a_backend_smoke source via
adapt_smoke.cmake. Screen-space fixtures explicitly initialize their identity
composite matrix; the runtime's initial zero transform is unchanged. New solid,
background and fractional-quad color probes prevent matching empty CPU/GPU
frames from passing. Original exact comparisons, tolerances and pixel probes
are retained. The target must still have exactly its reviewed source list; the
attachment refuses to silently replace a target with additional sources.

Correct setup makes clipping, GPU occlusion and target ownership pass. The smoke
still fails checker/YUY2/blend comparisons at texture boundaries. Do not waive
those failures. CPU occlusion's20200 vs GPU20000 shared-edge gap is also open.
The omitted-matrix control proves the stronger probes fail when draws vanish:

```sh
GAMEPKG_SMOKE_OMIT_COMPOSITE=1 RECOMP_SMOKE_BACKEND=vulkan \
  /path/to/build/xboxrecomp/src/d3d/nv2a_backend_smoke/nv2a_backend_smoke
```

This command must return nonzero and show failed solid-quad probes. The existing
noalias test is the independent stale-target control; it still reports the
expected stale-target failure when alias checks are deliberately disabled.

For an independent, ideal affine-nearest check of the synthetic checker:

```sh
RECOMP_SMOKE_BACKEND=vulkan RECOMP_SMOKE_DUMP=/tmp/private-smoke- \
  /path/to/build/xboxrecomp/src/d3d/nv2a_backend_smoke/nv2a_backend_smoke
cc -std=c11 -O2 -Wall -Wextra -Werror \
  engines/takedawn/media-bridge/check_checker.c -o /tmp/gamepkg-checker
/tmp/gamepkg-checker /tmp/private-smoke-checker_cpu.bmp /tmp/private-smoke-checker_gpu.bmp
```

The current result is0 ideal mismatches for CPU,1392 for Vulkan. Both the smoke
and oracle currently return1. The oracle uses integer rational coordinates and
the known palette, not a second call to renderer code; BMP parsing is bounded
to the exact640x480 BGR24 fixture layout. This is an ideal sampling calculation,
not proof of console precision. A tested precise-position shader experiment
does not fix it and is removed; the actual game binary stays byte-identical to
the earlier race-tested build. No sampling bias or relaxed tolerance is added.

## Private save/reload and mixed-audio gate

From the GamePKG repository root, with the private engine built:

```sh
bash tools/check_takedawn_session.sh \
  /path/to/private/burnout3-gamepkg-media /path/to/supported.xiso.iso
```

This Linux maintainer tool requires bwrap, standard shell utilities, ffprobe and
ffmpeg. It is not a player dependency or installed game launcher; no Python is
required. It always creates a fresh evidence directory under /tmp and never
accepts a real save/library directory as an output. The child environment is
cleared. Each session has networking disabled and read-only system/ISO mounts.
Persistent file output goes into the private state; temporary space and the
renderer's host devices are also present. An external 100-second bound and explicit
event/frame timeouts catch crashes or stalls; a normal completed session is
deliberately signaled and reaped rather than treated as a timeout success.

The first process follows the original new-profile/save-Yes/slot flow. The gate
requires original save-guard completion, one59,904-byte Profile 1 and54-byte
SaveMeta.xbx, then records both full hashes. A second process discovers that
slot and reads all59,904 bytes; normal button input completes loading. Hashes
must remain unchanged. The captured frames are retained for actual screen QA.

The first process also records exactly45 seconds of original mixed PCM, capped
by the engine before stopping:48kHz signed16-bit stereo,2,160,000frames and an
8,640,044-byte WAV. All format/length fields and finite non-silent channel/mix RMS
are checked. Physical playback uses a dummy device, so this is not an audible
fidelity, surround or full soundtrack/film qualification. Private profiles,
recordings and frames are not publishable package contents.

The complete gate passes locally, including intentional clean termination of
both processes. It writes result.json and evidence paths, with no source/runtime
or original disc changes. Full campaign/autosave/restore, device semantics,
native no-Python generation, signed package/process integration, platforms and
measured optimizations across every current port still remain.
