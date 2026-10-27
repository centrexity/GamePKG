# Native Windows gameplay validation

These checks concern the reviewed Takedawn profile. Its prepared game functions
compile to native Rust code; the pinned original platform runtime remains
C/C++. Players do not need Python, C2Rust or WSL. The owned supported ISO is read
in place. Maintainer tools and private test saves are separate from player
requirements and the user's library.

## Verified routes

The production runtime builder was exercised with the actual standalone Windows
tools under Wine, including an extended SDK path containing spaces. The resulting
executable has SHA-256
`6f858f72c89957abf6b5c0b38afbddae6ca7ed0dfffbf63980db4de34eb8efeb`
and retains all 24,032 native exports. All game bodies were compiled from the
prepared Rust units.

Reviewed original menu/pad inputs and the original debug AI were used for these
World Tour checks. No medal, score, unlock or race result was injected.

| Route | Observed result | Persistence check |
| --- | --- | --- |
| First Silver Lake race | First of six, two laps, 3:35.55; best lap 1:46.78. Gold, three new events, three awards and 2,095 points. | Original autosave succeeded. A fresh process showed 1/55 USA and 1/173 overall gold medals, with the new Downtown Road Rage event available. |
| Newly unlocked Downtown Road Rage 1 | Natural Totaled result with three takedowns. Silver, two new events, one award and 1,800 more points. | Original autosave succeeded. Another fresh process showed the silver event medal and the new face-off events. |

After each cold load, the active profile and metadata matched their saved bytes
exactly. The pre-medal reference stayed unchanged. These three race/Road Rage
probes finished successfully and reached no unresolved upstream game-function
stub. Probe termination after menu inspection is separate from a normal game
shutdown; normal settings restart and shutdown have their own checks.

The newly unlocked Waterfront Face-Off also reaches its original loss result:
one lap, second of two, 2:17.01, 165 points and no takedowns. Original Continue,
autosave and points screens return to the map without unlocking its reward. A
fresh process retains 4,060 global points, six takedowns and two completed events;
the Face-Off reward remains locked. The profile and metadata load byte-for-byte
unchanged. This cold probe exits zero. The initial event probe's outer wrapper
terminated with status 143 during its intentional stop after the menu return;
that harness failure remains recorded separately. The cold probe isolates game
and driver process sessions and stops cleanly. Neither reached an unresolved stub.

The earlier original-driver Single Event Road Rage attempts remained in play
past the driver's 900-second result deadline. They remain recorded as incomplete
natural-finish checks. They are not counted as successes or silently replaced
by the World Tour results above.

## Reached untranslated paths

The stricter executed-path audit finds that the current v13 runtime reaches
`wbinvd` at `0x00353DB5`, `cli` at `0x00367C86` and `sti` at `0x00367C97` in
both profile-create and reload logs. The upstream translator currently reports
these instructions and treats them as no-ops. Historical route and save/audio
results above remain observed outcomes; they do not prove complete instruction
semantics. The current v13 executable fails the stronger path gate.

The shared maintainer gate now rejects every reached untranslated instruction,
rather than checking only the two MXCSR instruction names, and still rejects
reached unresolved function stubs. Window/restart checks use the same rule;
the native restart probe explicitly enables upstream stub logging. Its fixtures
cover arbitrary future instruction names, split log chunks, earlier failed
restart phases and missing logs. The v14 candidate implements the reached cache and interrupt operations through
native Rust helpers and the original kernel's host dispatch gate, retaining the
original following-instruction interrupt shadow. Both real-kernel Linux and
standalone Windows conformance checks pass, including rejected masking-guard
mutations. Its complete standalone Windows Rust build retains all 24,035 exports
across 487 units; native runtime linking also succeeds (executable SHA-256
`4467bfd5ae1b91565a99cd06792d6695bc2d61e22626f62f4e6b5dbd66ae6540`).
The Linux candidate passes profile create/reload, full three-lap Race, original
Race/Crash control checks, and a 15,000-flip Time Attack session with normal
Quit/Yes/Continue back to the front end. The Windows candidate also passes profile create/reload, unchanged save hashes,
stereo capture, original Race and PS2-layout Crash control checks, keyboard profile
navigation, a 1024×768 window resize and normal close under Wine. These use an
independent headless GPU desktop and retain actual-Vulkan and unsupported-path
assertions. The first resize attempt was constrained by a 720-pixel test desktop;
the unchanged assertion passes on the taller desktop. Full signed-package Build,
updated packaging and physical Windows qualification remain required. The rejection is not waived because
the game reaches a menu or a result screen.

## Player handling of unsupported paths

The native player enables the runtime's immediate untranslated-instruction trap
and unresolved-stub logging. Linux and Windows also audit the original output
stream: a reached unsupported instruction or unresolved function stops the owned
session and produces a launch error. Failure state survives verbose output,
fragmented reads and managed restarts. Final output is audited even when the
process exits zero or the host requests a stop. These checks are separate from
the optional, default-off frame-stall watchdog. They report missing execution;
they do not implement unqualified game paths. Older admitted source packages can
therefore require a package update before they play.

## Offline two-player coverage

A bounded Windows/Wine probe reaches the original multiplayer Race with two
virtual pads, separate car selection, split cameras and both HUDs. Player one
reaches 39 mph while player two stays at zero; a separate player-two throttle
input produces 38 mph while player one is stopped. Both then receive concurrent
throttle inputs and move through the track. Debug AI is off. The original main
already defaults to two pads; no production default change is needed.

The reviewed-input probe eventually exceeds its 900-second overall deadline
while still in the race. It does not qualify a natural multiplayer race finish.
A separate follow-up opens and resumes the player-two pause menu, then uses the
original Quit/Yes flow to return to multiplayer setup and exits zero. Neither
probe reaches an unresolved function.

A controlled sequence confirms both players' pause/resume, including transfer
back to player one. Original read-only state traces match in the native C
reference, Linux Rust build and Windows Rust build. The Windows run also returns
through player one's original Quit/Yes flow and exits zero. An additional Windows
run with control logging disabled captures a fresh `Paused - Player 1` screen
and exits zero. The earlier screenshot-only missing-pause finding is withdrawn;
it did not establish a delivered-input failure. Physical pads, a complete
multiplayer race and actual Windows hardware still need qualification.

A separate concurrent comparison fails Vulkan buffer initialization and falls
back to CPU rendering. It is retained as invalid graphics qualification. The
maintainer gameplay gate now requires the actual Vulkan executor's confirmation
and rejects CPU fallback; a device-created message alone is insufficient.
The save/audio gate applies the same check independently to its create and reload
processes, before publishing a passing result. This includes native Linux runs.
Regression fixtures cover incomplete initialization, mixed restart outcomes and
markers split across log chunks. Earlier qualified route logs also pass this
stronger check.

## Online scope

These routes qualify offline gameplay. The
[pinned upstream plan](https://github.com/xenophobentx/takedawn/blob/5a00fc10f9548a189e9043c5a55975863ad49175/README.md#whats-next)
lists System Link/LAN, XLink Kai and Insignia as future work. The signed profile
currently disables networking. This integration does not claim those online
modes. The bounded two-player observations above do not qualify physical
controllers or complete multiplayer play. The separate GamePKG networking APIs do not establish
that this original game's online services work.

## App and platform checks

Native Windows and macOS host validation pass for app revision `b2b43fd`,
including the history contention fix, in
[run 40254964579](https://github.com/centrexity/GamePKG/actions/runs/40254964579).
The exact Windows ZIP has SHA-256
`2c77d459c8fd8073cc1aa7676deb86287ddd5f85760f96cf536f40279e34e644`.
Its isolated Wine checks pass CLI/GUI versions and embedded audio, temporary
signed-package installation, default Vulkan rendering, resize/close and actual
OLE catalog import with persistence and no package download.

The release builder selects the static MSVC runtime. Both app binaries pass
normal and delayed PE import checks, and the exact ZIP passes the Wine checks
with external MSVC runtime DLLs disabled. The game executable and its SDL
companion pass the external compiler-runtime import check independently. This
avoids the unbundled runtime requirement found in an earlier app artifact.

Windows32, Linux32 and Android32 checks pass in
[run 40257219069](https://github.com/centrexity/GamePKG/actions/runs/40257219069).
The Windows32 gate also executes both actual app entries after relocation,
without a display or external decoder files. The x86-64 game itself still
requires a 64-bit system.

The unpublished hardware preview uses this exact native app and the verified
signed v13 recipe, with a separate library and settings. Every extracted app
file matches the native ZIP. This preview pins production app `b2b43fd`;
the subsequent `c766be1` app change adds the optional frame-stall diagnostic
watchdog described below. The handed-off preview predates that setting.

All automated audio output is dummy/muted or routed to a verified private
null-only sink. Rendered captures and non-silent
audio captures do not establish audible fidelity or physical controller access.
A fresh serialized Windows/Wine save/audio repeat completes the 45-second
capture and unchanged profile reload, but reports at least 55 SDL queue
underruns during the introductory sequence. A preceding unchanged run reports
none. The capture gate therefore proves complete mixed output and save behavior,
not consistently smooth playback; audio timing remains an open qualification.

Two later unchanged Windows/Wine runs record the private null sink through
WASAPI and compare its actual output with the game's capture. Each complete
45-second stream matches every PCM byte at one fixed recording offset, with
no queue-empty reports during either compared create process. The second
reload logs one queue-empty event; its output has no separate WAV reference
and is not qualified by the create comparison. These successful repeats do not
resolve earlier intermittent failures or prove hardware audio.
The maintainer tool `tools/check_takedawn_pcm_output.py` checks the complete
48000 Hz stereo s16le stream, using the measured preroll in `--offset-frames`.
Its fixtures reject inserted silence, dropped/reordered frames, changed
channels, truncation, invalid formats and incorrect offsets.


An independent audio-path control queues a known 45-second stream completely
before starting playback, with no game code, renderer or per-frame audio
producer involved. The Linux SDL 2.30.0/PulseAudio path reproduces all 2,160,000
stereo frames byte-for-byte (PCM SHA-256
`8042adebd51f89b51fa3aa65b4e2a21d93c089adfe0d6615658e6af253c6f0eb`).
Both Windows SDL 2.32.10/Wine/WASAPI control recordings contain discontinuities.
In the second, the initial reference anchor is present and the first exact
comparison fails at reference frame 489,568; later sampled anchors are displaced
by 1,393 frames. The first also differs at startup and later within the stream.
Only a private null sink and its monitor are loaded; the active Windows stream
has unity volume, 48 kHz stereo and the server's copy resampler. All control
processes finish successfully and consume their prequeued stream in about
45 seconds. The captured output, rather than successful process exit or queue
consumption, is what exposes the mismatch.

These controls establish that this Windows-through-Wine recording path can fail
without a game-side audio producer. They do not isolate Wine, SDL, host scheduling
or the recorder as the cause: the Linux and Windows SDL versions and transport
formats differ. They also do not resolve the game's separately observed
queue-empty reports or qualify real Windows hardware. A failed Wine recording
alone is insufficient evidence for changing the game's buffers or audio code.

Wine gameplay also does not establish original-game GPU/audio/controller access
inside an actual Windows AppContainer.

The full signed Windows install, native Build and integrity repair gate passed in
[run 40229229052](https://github.com/centrexity/GamePKG/actions/runs/40229229052).
It compiled all 486 prepared Rust units with native optimizations, produced a
ready-to-play library entry, rejected a damaged SDL companion, then repaired it
while preserving every compiled object hash and modification time. No owned
media or gameplay was used in that build gate.

The native frame-stall watchdog is optional and explicitly off by default for
new and existing settings. In the reviewed runtime it watches actual flips and
writes diagnostic reports without terminating the game. Settings accepts 5–600
seconds when enabled; startup allows at least 60 seconds. Saving, cancelling and
disabling the option pass the real desktop settings checks. Actual Windows
host checks pass in [run 40433416799](https://github.com/centrexity/GamePKG/actions/runs/40433416799),
including saved-setting restart and default-off launch. This host fixture does
not qualify the original game's physical Windows graphics or controls. Broader
campaign coverage and actual Windows hardware qualification remain open. No stable release is qualified
by these routes alone.

## Private GPU display for maintainer Wine checks

On Linux, the window and keyboard probes can run on a separate headless
Weston/Xwayland desktop, without activating windows on the user's desktop:

```sh
python3 tools/with_private_gpu_display.py --vulkan-probe -- \
  python3 tools/check_takedawn_windows_window.py \
  /private/build/burnout3-gamepkg-rust.exe /path/to/owned.iso /private/reference-saves
```

The helper needs Weston, Xwayland, Metacity, D-Bus, xauth, xprop and xdotool;
`--vulkan-probe` also needs vulkaninfo. These are maintainer tools, not Windows
player dependencies. It creates its own short Wayland socket path, authenticated
X11 display, window manager and compositor data directories. Rootful Xwayland
allows real XTest key events and focus checks without a physical Wayland input
seat. Child test commands retain their existing memory scopes and audio controls;
the game probes explicitly use dummy audio. The helper cleans up only its own
process groups and records display logs. A successful Vulkan probe or display
startup does not replace the game's actual-executor, input, save, audio or
unsupported-path assertions, and does not qualify physical Windows hardware.

See the [Windows build notes](../engines/takedawn/rust-port/WINDOWS_BUILD.md)
and [native process ownership notes](../engines/takedawn/windows-runner/README.md)
for the SDK, sandbox, committed-memory budget and restart mechanisms.
