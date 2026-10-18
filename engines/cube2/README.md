# Cube 2: Sauerbraten engine integration

This is the GamePKG port of the original Cube 2 engine. The `cube2-r6889` native profile is registered in the 0.1.15 host: a signed dependency-free selector can be installed, built once and launched in its separate Vulkan player window. Keyboard/text input and persistent edited maps work locally. Two real engines have connected through the host-owned multiplayer transport using signed loopback endpoints. Independent background ticking is verified. The selector supports user-chosen join/hosting addresses and host-selected master-server browsing. Two real engines have also joined using only a discovered server grant and remained connected with their windows idle.

## Source provenance

The engine source is from the official repository:

- URL: https://svn.code.sf.net/p/sauerbraten/code/src
- Revision: **6889** (2026-03-10)
- Engine license: Zlib; preserved in `upstream/readme_source.txt`.
- Bundled ENet license: preserved in `upstream/enet/LICENSE`.

`upstream/` contains the unmodified source subset required for the client. Upstream precompiled Windows libraries, bundled system headers, IDE projects and the separate RPG game are excluded. `upstream-lock.json` records every included file's SHA-256. Adaptations live separately in `adapters/`; build staging applies explicit, single-match transformations and does not edit upstream files.

Game maps, models, textures, fonts and audio have individual licenses. They are not included here. The intended port uses a separately selected original game data installation in place, read only. GamePKG must not publish a modified media archive or imply that the engine's Zlib license covers the media.

## Local development

The Rust host now embeds, verifies and compiles the pinned source kit in isolated compiler namespaces. Its development session starts the real engine with a hard address-space limit, bounded frame deadlines, private PID/network/filesystem namespaces, read-only data/packages mounts and bounded temporary filesystems. The engine receives no desktop display connection. A Rust gameplay test verifies rendering, input and worker cleanup. The production path verifies the signed selector and cache receipt before playback, applies the lower of signed and player memory ceilings, and uses the selected frame deadline after a bounded startup.

Players will not need Python. The following maintainer tool is an independent bridge development aid:

```sh
python3 tools/build_cube2_engine.py --verify-only
python3 tests/cube2_sources.py
python3 tools/build_cube2_engine.py
```

The local build needs a C++ compiler, Make, SDL2, SDL2_image, SDL2_mixer, zlib and OpenGL development libraries. It builds the bundled pinned ENet source, not an arbitrary engine supplied by a package. Its output is `target/cube2-bridge-build/sauer_client`. This is a development executable, not a supported GamePKG launch path or a sandbox by itself.

## Frame bridge

The development bridge runs SDL with its offscreen video driver and Mesa software rendering. Cube 2 renders internally using OpenGL; only bounded RGBA frames cross into the host. This avoids connecting the engine to the user's display server. The production player can present these frames through its existing Vulkan renderer without giving the engine a desktop window or GPU handle.

One input packet authorizes one engine tick and frame. Input is exactly 16 bytes: `GPK1`, a little-endian held-key mask, then little-endian signed relative mouse X/Y deltas. Bits 0–15 map to W/S/A/D, Space, Shift, Enter, Escape, Tab, E, R and weapons 1–5. Bits 16 and 17 are primary and secondary mouse buttons. Mouse deltas are bounded to ±2048; unknown button bits and malformed packet magic stop the bridge. EOF terminates the engine. Output is `GPF3`, little-endian width, height, RGBA byte length, audio frame count and channel count, followed by tightly packed RGBA pixels in OpenGL's bottom-up row order and interleaved signed 16-bit little-endian stereo PCM at 48 kHz. The reader retains `GPF2` mono compatibility. Audio is limited to 4,800 sample frames per channel (100 ms) per update; an SDL dummy mixer feeds a fixed-size ring and cannot open a host audio device. Frame dimensions are validated against the original engine's 10000-pixel coordinate range; actual allocation is constrained by the graphics driver and session memory/time limits. Engine diagnostics go to stderr, never into the frame channel.

## Supported integration

Signed installation/build receipts, original-data selection, keyboard/text input, private edited-map persistence, host audio, background ticking and both native-window close and in-game Quit are implemented and tested. The engine keeps mandatory namespace, syscall, time and memory limits. Ordinary guest image and save limits are unchanged. The reviewed networking broker supports private direct multiplayer and user-selected join/hosting endpoints; host-selected master-server browsing is implemented. Graphical editing receives middle/side mouse buttons and both wheel axes through bounded host events.

## Verification during development

`cargo test --offline --all-features --lib cube2::` checks embedded source provenance, adaptation anchors and the Rust protocol's byte order, row orientation, alpha normalization, input bounds and malformed/truncated output handling. The ignored native compiler test exercises the Rust build path in compiler namespaces with no network or home directory:

```sh
cargo test --offline --all-features --lib \
  cube2::build::tests::compile_native_engine_without_python_or_network -- --ignored
```

With a complete original data installation available locally, `tests/cube2_bridge.py` exercises the actual engine on the original `complex` map. It mounts only the selected game data read-only in private filesystem/PID/network namespaces, clears the environment, supplies no display connection and uses software OpenGL. It checks fixed frame dimensions, a nonblank frame, changed frames after movement/mouse input, firing input delivery, malformed packet rejection and EOF shutdown. The Rust session additionally verifies real mixed audio, preference restoration, bounded save snapshots and graceful close. It writes a local ignored screenshot under `target/`; the screenshot and game media are not release assets.

```sh
GAMEPKG_CUBE2_DATA=/path/to/original/sauerbraten python3 tests/cube2_bridge.py
```

This Python bridge test does not prove production package installation, host-controlled networking, preference persistence, host audio playback or player-window integration. The separate Rust session test verifies bounded configuration snapshots and scoped blob persistence. Those remain requirements of the complete port.

The Rust-only session can also be exercised with original local game data:

```sh
GAMEPKG_CUBE2_DATA=/path/to/original/sauerbraten \
  cargo test --offline --all-features --lib \
  cube2::session::tests::original_game_runs_through_the_rust_session -- --ignored
```

This test builds the engine with Rust, starts the original `complex` map under a 2 GiB address-space limit, receives frames and sends movement/mouse/fire input, creates and saves an edited map with a changed map title, types a name through the original console, invokes its Quit handler, then starts a second engine from the archive and verifies the map bytes and edited title reload. It confirms the worker is reaped and the original installation is unchanged. It does not require Python.

## Configuration persistence and data handles

The host restores a validated `GPB2` archive into the engine's private `/save` filesystem before initialization. `GPS1` still requests a snapshot without advancing a game tick; the response is now `GPS2`, with explicit little-endian entry counts, byte lengths and reserved fields. The archive permits at most 256 files, 16 MiB per file, and 32 MiB including metadata. Preferences (`config.cfg`, `init.cfg`, `servers.cfg`), map files and companions under `packages/` (`.ogz`, `.cfg`, `.BAK`, `.jpg`, `.png`), and `.dmo` files under `demo/` or `demos/` are eligible. Names use ASCII letters/digits, underscore, hyphen and dot, with at most six path components and 240 bytes. Absolute paths, parent/current components, hidden names, encoded separators, backslashes, duplicate entries, file/directory collisions and other file types are rejected. The C++ adapter and Rust reader both validate these rules.

Snapshot traversal opens directories and regular files with `O_NOFOLLOW`, rejects hard links and special files, and limits traversal work. Restoration validates the whole archive before writing into the empty private filesystem, then creates files exclusively through directory handles. The host never extracts paths supplied by the engine into the user's filesystem. It atomically commits one validated `.cube2` archive under a pinned save-directory handle, scoped to game, signed creator and save schema. Invalid snapshots preserve the previous archive. Storage reporting includes this archive. Old three-slot `GPS1` preference blobs migrate automatically without being overwritten; ordinary games retain their existing 1 MiB save limit.

`GPE1` requests normal engine shutdown after the host has collected its snapshot. The original in-game Quit handler sends `GPQ1` followed by its `GPS2` archive; the host validates it, waits for a successful process exit, saves preferences and play time, and closes the player window. Unexpected EOF remains an error.

The Rust launcher opens the selected data/packages directories before compiling and passes directory handles to Bubblewrap's `--ro-bind-fd`; it never resolves their pathname again after compilation. Tests replace a selected path with a symlink and verify the opened handle still points to the original directory. The engine sees only the two selected subdirectories. The rest of the installation, the user's home, library and desktop sockets are absent. The root filesystem is read-only and `/tmp` is limited to 16 MiB and the private `/save` mount to 64 MiB. The committed save archive remains limited to 32 MiB; snapshots require 1 MiB free space before flushing preferences. Runtime needs Bubblewrap with `--ro-bind-fd` support (tested with 0.9.0), Linux seccomp thread synchronization and `close_range` support. There is no unrestricted fallback.

The `complex` gameplay/audio/preferences test passes with a 2 GiB address-space cap; startup did not complete with 512 MiB or 1 GiB in the tested software renderer. This is a virtual-address limit, not a measured physical-memory requirement. The app's saved ceiling remains 512 MiB by default; users can now select up to 4096 MiB. Ordinary package hard limits remain capped at 512 MiB. The profile permits a signed hard cap up to 4 GiB; the recipe recommends 2 GiB and sets a 4 GiB hard cap. The effective process cap is always the lower of that cap and the saved user ceiling. A 500 ms tick deadline is recommended for first-use software shader compilation; the default remains 100 ms, and Settings permits 10–1000 ms.

The guard itself can be verified without game media:

```sh
g++ -Wall -Wextra -Werror -pthread tests/cube2_guard.cpp -o target/cube2-guard-test
target/cube2-guard-test
```

## Signed selector and desktop checks

The selector template is `ports/cube2`. It accepts only its exact reviewed launcher and a dependency-free Cargo description; it cannot add package C++, extra Rust modules, media or shaders. The host embeds and verifies the engine source kit. Native builds honor the saved optimization setting and target the local CPU when optimization is enabled. The cache identity includes the source/adapter/compiler fingerprints, host version and CPU identity. Playback verifies and snapshots the cached ELF instead of recompiling.

`tests/cube2_package.py` packs/signs a private fixture, installs it, builds it, plays cached frames, remembers the original data folder and checks scoped preferences/progress. It corrupts the cached engine and verifies playback is refused before execution, then restores it. `tests/cube2_gui.py` uses the actual system folder picker and library Play button, checks the separate Vulkan window contains only the game, verifies movement and the exact ammunition change from a short click, types commands through the original console, and checks both operating-system close and in-game Quit for saved preferences/time and a clean process exit. These are maintainer tests; no Python is used in the user build or player path.

```sh
python3 tests/cube2_package.py
python3 tests/cube2_gui.py
python3 tests/cube2_recipe_attacks.py
```

The local CLI can remember data without exposing it in the package:

```sh
gamepkg build com.gamepkg.cube2
gamepkg run com.gamepkg.cube2 --game-data /path/to/original/sauerbraten --frames 60
```

The engine starts at its original menu and preserves saved console/HUD and FPS preferences. Choose a mode/map through the original menus or console. Original maps, textures and audio remain in the selected folder; original executables are never launched. Edited maps reload from the private saved-file archive. Blank multiplayer settings clear launch commands; the signed networking flag still applies. Saved join/hosting choices are applied at the next launch through the host broker.

## Native keyboard, text and pointer input

The existing `GPK1` movement/action mask remains available for controllers and touch. Additional desktop keys use bounded `GPI1` events with explicit SDL USB scancode, press/release/repeat flags and modifier bits. `GPT1` carries at most 31 bytes of UTF-8 text; the host splits longer input at character boundaries. Each tick accepts at most 64 additional events. `GPM1` updates Shift/Ctrl/Alt/Meta state, and focus loss sends `GPR1` to release tracked keys and reset modifiers. Function keys F1–F24, console/chat text, editing/navigation keys and punctuation use the original engine input handlers. Clipboard text is forwarded only when the user pastes into the focused window. No native event structures or pointers cross the bridge. Middle and side buttons use fixed-size `GPC1` packets; `GPW1` carries signed wheel steps with a combined 16-notch bound per packet. Fractional scrolling accumulates on the host, excessive whole steps are discarded, and horizontal directions match native SDL. The adapter emits each notch separately so original editing bindings retain their step counts. Focus loss also releases tracked extra mouse buttons.

`tests/cube2_pointer_gui.py` sends physical X mouse-button and wheel events through the separate player and checks the original engine’s saved binding results. The real-engine session test also verifies multiple notches, both axes and release-on-reset; protocol unit tests reject invalid buttons, modifiers and signed bounds.

## Saved-file attack tests

`tests/cube2_saved_files.py` compiles the production C++ persistence adapter without SDL or game media and runs it inside isolated namespaces. It checks archive round trips, malformed counts/lengths/reserved fields, traversal and encoded paths, duplicate/unsorted entries and file/directory collisions, symlinked parents/files, hard links, FIFOs, and file/count/aggregate quotas. Rust tests independently validate archive framing, migration, creator/schema isolation, atomic failure preservation, directory replacement resistance and storage accounting.

```sh
python3 tests/cube2_saved_files.py
cargo test --all-features --lib cube2
```

## Host-owned multiplayer transport

The staged ENet `unix.c` keeps its initialization and time helpers but replaces its socket/address operations with the reviewed `adapters/network.h` transport. The engine receives a private Unix socketpair on descriptor 3; frames and saved-file responses use descriptor 4. Data directory descriptors are pinned above these slots, consumed by the namespace launcher, then closed before game code runs. The mandatory syscall guard still denies creating sockets, process creation and execution. No external socket descriptor reaches the engine.

`GPN1` requests and `GPR2` responses have explicit little-endian operation/status/length/reserved fields and an 8192-byte envelope cap. The current broker implements IPv4 UDP, with at most 16 virtual sockets, eight opens per second, 4096-byte datagrams, 2048 operations and 1 MiB of payload traffic per second. It closes channels that exceed the hard request-flood ceiling or send malformed framing. A mutex serializes ENet calls from the main and resolver threads. RPC transport failures do not enable any raw-socket fallback.

The signed `network = true` enables the broker and `false` disables it. Enabled
UDP has no hostname, destination, source or bind allowlist; LAN/private addresses
and the broadcast socket option need no separate permission. Saved join/hosting/
master fields supply launch commands rather than capability grants. DNS resolves
game-requested hostnames with bounded jobs and a 500 ms broker wait; lookup failure
is explicit and never leaves a late reply in the engine RPC stream. Master-list
TCP is still a bounded snapshot of the chosen master's fixed `list\n` request;
general engine TCP authentication/registration remains unfinished.

The ignored Rust test `two_original_engines_connect_through_the_host_broker` compiles the pinned engine once, runs two isolated original engines, starts a private loopback listener, connects the client, then stops UI polling for 35 seconds. It verifies both workers continue ticking with bounded frame/audio queues, the server receives the client's game identity, the remote connection survives, and shutdown commits preferences and the background play clock. No public server is contacted.

```sh
GAMEPKG_CUBE2_DATA=/path/to/original/sauerbraten \
  cargo test --offline --all-features --lib \
  cube2::session::tests::two_original_engines_connect_through_the_host_broker -- --ignored
python3 tests/cube2_network_bridge.py
```

Broker unit tests cover unlisted peers/sources/listeners, hostname lookup, broadcast options, stale handles, malformed framing, packet/handle/open/rate bounds and disabled-network denial. The C++ shim test verifies raw sockets remain denied, malformed/truncated/oversized replies cannot overrun receive buffers, oversized sends fail before reading their buffers, and concurrent requests retain their framing.

The updated source host also applies ENet's bounded receive/send buffer sizes,
address reuse, read/write timeouts and TTL to its owned UDP sockets. Settings
survive replacement of the ephemeral socket with a listening endpoint;
address reuse is applied before binding. Error and TTL queries read the host
socket instead of returning fixed placeholder values. IPC operations 11 and 12
carry set/query requests with exact lengths and validated virtual handles.
Queries leave the engine's output untouched if the host denies or malforms a
reply. The virtual master-list snapshot supports nonblocking mode and checks its
own connection state for error queries; it is not an arbitrary TCP connection.
Blocking mode and unrecognized/excessive options are denied. Broadcast accepts the original boolean enable/disable option.
These changes require rebuilding the source host and engine cache; the cache
fingerprint includes the adapter. LAN discovery, master authentication, public
registration and complete shutdown/listen/accept behavior remain open.

## Background desktop execution

The desktop hands the warmed, deadline-limited session, pinned save store and play-history clock to one host worker. It advances the original engine independently of window painting, so networking stays active while minimized. The CLI keeps its synchronous finite-frame behavior unless a host explicitly enables background execution.

The UI submits bounded controls/events and polls one replaceable latest frame; it cannot accumulate a frame backlog. Audio uses a host-only writer to the existing output stream and retains the existing 1600-sample submit/4800-sample queue limits. When no output stream is active, at most 100 ms of recent audio is retained. No audio device handle reaches the game.

Short input edges are latched across worker/UI scheduling, mouse deltas saturate at the existing bound, and additional events retain their 64-event quota. Focus loss clears pending input immediately. If UI updates stop for 250 ms, held controls are released; already queued one-shot events are delivered once and tracked keys/modifiers are reset without exceeding the event quota.

The worker checkpoints play history independently and stops its clock before saving/closing. Normal close and in-game Quit flush preferences/maps and reap the engine. Engine failure preserves the prior archive and stops the clock even if the UI does not resume for a long time. Worker drop requests shutdown and joins the owner rather than leaving a detached game.

The real desktop test starts a private window manager, verifies an actual minimized `IconicState`, observes a play-time checkpoint while minimized, restores the window and quits through the original game handler. The real session tests also kill only a test-owned engine and verify process reaping, previous-save preservation and exclusion of later UI idle time. A null-output-device test feeds audio through a writer thread while the device stream stays on its owning thread.

## Choose a server or host a game

On the game page, **Multiplayer** accepts either a join `hostname:port` or a hosting `local-IPv4:port`, with a separate local/private-address opt-in. The game port and the adjacent server-information port are authorized. Ports must be 1–65534; wildcard binds, IPv6, credentials, paths, queries, script syntax and control characters are rejected. Joining and hosting cannot be selected simultaneously. **Save multiplayer settings** persists without starting a connection; **Play** also saves the visible choices and applies them at launch. **Use offline play** removes the grant for the next launch. Existing sessions keep their pinned policy. Hosting uses `startlistenserver 0`, so it does not advertise on a public master server.

The signed selector and creator identity scope the bounded host-only `network-selections.json` record. Versions with the same declaration retain choices; changed creators or declarations do not inherit them. Malformed settings fail closed, invalid edits preserve the prior file, and the game cannot read or modify the host settings file. DNS is resolved and checked before the engine starts, once per selected hostname; game and server-info ports share the pinned address.

```sh
gamepkg configure-network com.gamepkg.cube2 --server play.example:28785
gamepkg configure-network com.gamepkg.cube2 --server 127.0.0.1:28785 --allow-local
gamepkg configure-network com.gamepkg.cube2 --listen 192.168.1.20:28785 --allow-local
gamepkg configure-network com.gamepkg.cube2 --clear
```

No signing key is needed for these end-user choices. The original creator signature remains intact. The real two-engine test now saves host/client selections and exercises automatic join/hosting startup commands, then verifies the connection and independent background clocks. CLI and desktop tests exercise denial before local opt-in, persistence, and clearing to offline.

## Host-selected master server

The Multiplayer form also accepts an optional **Server browser master** `hostname:port`. This explicitly permits discovering and joining the IPv4 servers listed by that master. No master is contacted by default. The CLI equivalent is:

```sh
gamepkg configure-network com.gamepkg.cube2 --master master.example:28787
```

Use the actual master address you trust. Local/private masters and discovered servers require the separate local/private-address opt-in. **Use offline play** / `configure-network --clear` clears discovery as well as direct join/hosting choices. Hosting remains unadvertised.

The host resolves and pins the selected master before the engine starts, then retrieves `list\n` asynchronously through a host-owned TCP socket. Connect timeout is two seconds; total retrieval is limited to five seconds, 64 KiB and 512 unique servers. At most 16 retrieval jobs can run process-wide. Only validated numeric IPv4 `addserver IP port` records are forwarded. Echo text, script fragments, DNS names, invalid ports and disallowed addresses grant nothing; malformed framing, control characters and quota overflows reject the response. Game ports must be 1–65534; their adjacent information ports are also granted.

The original in-game browser receives an immediate last-good snapshot through virtual stream handles. These handles accept only the chosen master endpoint and exact list request; they cannot open arbitrary TCP sessions, register a public server or send authentication/chat data. The host starts discovery at launch and refreshes its cache at most once every 30 seconds while network RPCs are active. A browser refresh during a pending fetch uses the previous snapshot (or reports no reply if no snapshot exists); refresh again after retrieval completes. A failed refresh retains the previous list until its ten-minute expiry. Discovered grants are session-local, replaced with the next valid list, and expire with the snapshot. This keeps slow masters outside the game tick deadline.

The ignored `original_server_browser_reads_only_the_host_master_snapshot` test runs the original browser against a private mock master under the ordinary 500 ms tick deadline, then checks all 512 approved records in its saved server list, including reads beyond the original 4 KiB/8 KiB buffers. Broker tests cover asynchronous empty/ready snapshots, exact requests/endpoints, unauthorized sends, TTL and caching. Parser tests cover injection, malformed/truncated lists, duplicate and excessive entries, and private-address opt-in. No public server is contacted by these tests.

The ignored `two_original_engines_join_through_the_discovered_server_list` test verifies the original browser, the discovered grant (without a static destination), a real two-engine connection, and 35 seconds of continued background networking/play-time accounting.

`python3 tests/cube2_readiness_gui.py` exercises an actual desktop Build with a 512 MiB ceiling, verifies the visible 2048 MiB requirement, changes and saves the ceiling through Settings, and launches the built game without a manual refresh. It requires the original data fixture at `target/cube2-test-data/` and keeps its library and configuration in a temporary directory.

## Native mouse capture

The desktop player locks and hides the system pointer while focused, including
when Escape opens the original engine menus. Cube 2 continues to draw and move
its own menu cursor through its original relative-input handler. Wayland uses
pointer lock; X11 and Windows use supported confinement and recentering with raw
relative motion. Focus loss releases and shows the system pointer; focus return
recaptures it. Ctrl+Alt explicitly releases the pointer until the next click in
the game window. Recapture drops the first relative delta to avoid camera jumps.
Game input is cleared while released or unfocused. Closing or a runtime failure
releases capture before any error dialog.

`python3 tests/cube2_mouse_gui.py` checks actual X11 pointer ownership and cursor
pixels through launch, Escape, explicit release, click recapture, focus changes,
and native close. Run `tests/cube2_package.py` first to prepare its private cached
engine and remembered original data; no game data is included with the test.

## Resolution and original crosshairs

The desktop host requests the game window's physical pixel size. **Limit game
render resolution** in Settings is off by default, so a 1920×1080 window renders
at 1920×1080 rather than being reduced to 1280×720. Enable the setting and choose
maximum width/height to reduce rendering work while preserving aspect ratio.
Existing settings files also default to no limit. Changes apply next launch.

The private offscreen SDL window only supplies the graphics context. The engine
renders into a dynamically allocated framebuffer with color and depth/stencil
attachments. Its default framebuffer bindings are redirected to that target,
so resizing does not depend on SDL/EGL window-surface resizing. Allocation is
checked against the driver's actual renderbuffer capability. The original
engine's coordinate range and session memory/time limits still apply; failure
to allocate reports an error instead of silently reducing the chosen resolution.
Resize requests are validated on both sides and coalesced during window drags.

Modern crosshair files remain preferred. If unavailable, the engine uses the
older original installation's `data/crosshair.png`, `data/teammate.png` and
`data/hit.png`. Player crosshair settings remain in effect. These files are read
from the selected original installation; none are copied into the package.

`python3 tests/cube2_resolution_gui.py` checks actual window resizing, widescreen
presentation, the engine's saved resolution and visible crosshair pixels. Set
`GAMEPKG_CUBE2_TEST_DATA` to an older original installation and add `--legacy`
to test the old crosshair filenames in place. Add `--limited` to verify the optional 1280×960 limit; without it, the test verifies native 1600×900 rendering. The test uses and restores only
its private maintainer fixture's selections and saves.


## Original startup and ordered input

Normal launch adds no map, HUD, menu or FPS overrides. Explicit host-selected
multiplayer choices remain the only launch commands. Gameplay tests choose their
fixture map explicitly rather than depending on production startup shortcuts.
Desktop keyboard controls use ordered SDL scancode press/release events,
including movement, Enter, Escape, Tab, Pause, distinct keypad keys and left/right
modifiers. Media/browser/application keys with SDL equivalents also pass through
the bounded 512-entry scancode table, including focus-reset release. Accepted
keyboard, paste and committed IME text stays ordered with physical events.
Controller/touch actions retain their held-button mask. Initial OS lock-key
state and remaining uncommon-key/platform behavior need further verification. Original engine FPS/menufps settings own worker pacing, and the
player requests presentation at the window backend's cadence.

A trusted `GPL1` marker precedes original map loading. It grants one fixed
60-second loading deadline for that tick; repeated markers cannot renew it.
Other ticks retain the configured render deadline. The host coalesces idle
modifier/resize snapshots and buffers up to 1024 discrete input events during
loading, draining at most 64 per engine tick without reordering them. This avoids
stopping a normal map load because idle desktop updates filled the queue.

`tests/cube2_startup_gui.py` verifies the original menu, chooses a map through
original console input, preserves custom console sizes and FPS settings, and
closes normally. The original-menu framebuffer is initialized before first draw
so uninitialized depth storage cannot mask menu background pixels.


## Stereo audio preservation

The mixer callback preserves SDL's original left/right samples instead of
averaging them. The 100 ms ring, byte protocol, background worker and host output
queue carry complete stereo frames; chunk and queue limits count frames rather
than accidentally halving or doubling latency. Volume and resampling apply to
both channels together. Stereo devices receive separate left/right samples;
mono devices receive their average and extra surround channels remain silent.
Legacy mono guest audio is unchanged.

`tests/cube2_audio_bridge.cpp` exercises the actual production callback and
encoder with known positive/negative channel pairs, wraparound and malformed
callback lengths, without SDL or game media. CI compiles and runs this test.
Rust tests cover stereo wire truncation/channel validation, exact sample values,
volume, resampling and output-channel mapping. The ignored
`audio::tests::original_cube2_stereo_reaches_the_output_callback` test loads the
signed cached selector, chooses a map, checks distinct original engine channels,
then verifies background audio reaches the real CPAL callback via an ALSA null
device. Set `GAMEPKG_CUBE2_TEST_ROOT`, the private `GAMEPKG_CONFIG`, and
`ALSA_CONFIG_PATH` to the maintainer fixture and null-device configuration to
run it; no speaker output or copied original assets are required.


Current source uses the shared signed `network = true`/`false` permission.
Enabled UDP has no destination, bind or DNS allowlist, and broadcast can be
enabled through the original socket option. Local/private addresses need no
extra grant. Saved join/host/master choices are launch preferences; blank
choices no longer force offline mode. Disabled networking still blocks every
broker operation. Socket, packet, time and transfer limits remain enforced.
The bounded master-list snapshot remains separate from general engine TCP,
authentication/registration and full LAN fidelity, which are unfinished.
