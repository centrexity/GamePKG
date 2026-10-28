# Native Windows gameplay validation

These checks concern the reviewed Takedawn profile. Its prepared game functions
compile to native Rust code; the pinned original platform runtime remains
C/C++. Players do not need Python, C2Rust or WSL. The owned supported ISO is read
in place. Maintainer tools and private test saves are separate from player
requirements and the user's library.

## Prepared v15 multiplayer input follow-up

The unreleased 0.1.4 recipe selects the v15 kit, SHA-256
`7f56741fa8f0a56d3e718af1f772bae9200514b6293c2d218e56cde700123c8b`.
It changes only the staged native controller policy and its CMake attachment;
all 487 prepared Rust units and 24,035 exports are unchanged from v14.
Older v14 recipes remain admitted for identity/save continuity.

An actual v14 split-screen run reproduces a missing PS2-control gate: both
players are in play under the original multiplayer module, but their physical
bottom buttons remain Xbox A instead of the accelerator. The new read-only gate
follows the title's own controller assignments and device ports, then checks
each player's phase, intro and result state. It does not assume that the first
physical controller owns player one. Shared pause/exit transitions stop driving
controls; one player's results do not change the other player's active controls.
Keyboard/touch input keeps the existing source exclusions and slot ownership.

The complete staged input source passes 213 assignment/state/handover checks
under Address, UndefinedBehavior and Leak sanitizers at both native pointer
widths, with a compile-time 32-bit requirement for the ELF32 CI mode. All 15
glyph-family checks still pass. A native reference build compiles the changed
input object and relinks hash-verified existing runtime/Rust objects; it retains
all 24,035 exports. It is not a fresh rebuild of every source file.
Actual split-screen checks pass PS2 accelerator mapping for both players,
held-button suppression when pause opens, and mapping restoration after Resume.
Both Xbox and Nintendo face layouts pass with player one pausing. A separate
completed run verifies player two owning Pause, suppressing both held buttons
and restoring both accelerators after Resume. Original Vulkan and unsupported-path
gates pass, with dummy audio and unchanged private references. A first helper
assertion incorrectly excluded controller reads on the exact pause-transition
frame; its failure and same-frame traces are preserved before the corrected run.

The same incremental reference executable also passes the pinned original
single-player Xbox Race and PS2-layout Crash assertions. Race retains countdown,
acceleration, held-button pause/Resume and Quit/Yes. Crash retains countdown,
accelerator response, impact/aftertouch/crashbreaker, results handover and
Continue to the front end. These two runs pass original Vulkan/unsupported-path
gates with dummy audio and read-only ISO access; they remain incremental
reference evidence rather than normal signed v15 Build/Play qualification.

Both app entries build and six real source-kit handoff/rejection tests pass.
The normal signed Linux v15 package now passes an unseeded optimized native Build:
all 487 units compile from an empty game-object cache in approximately 62 minutes,
and all 24,035 prepared exports remain. The initial private preflight omitted the
native tools folder and stopped before any unit compiled; configuring the
existing documented folder resolves that setup error.

Normal integrity, wrong-disc rejection without saved selection/play time,
selected-ISO Play, a second launch with remembered ISO and increasing play history
all pass. The ISO stays in place. Deliberately damaging the private executable
fails integrity; source repair and final integrity succeed while preserving all
487 compiled-unit hashes and modification times, the signed package, settings,
ROM selection, play history and player data.

The exact repaired executable (SHA-256
`621988980facdace69fc4dbf1e2b6de4043b442dd8d910854f09df38002b30df`)
then passes five actual native control routes: the pinned original Xbox Race and
PS2 Crash checks, plus split-screen Xbox-face/P1 Pause, Nintendo-face/P1 Pause and
Xbox-face/P2 Pause. The latter three verify both accelerators, held-button
suppression while paused and restored mapping after Resume. Viewed resumed
screens show both players moving. All runs pass strict executed-path and actual
Vulkan gates with dummy audio, read-only ISO access and unchanged private
reference saves. Virtual pads stand in for physical controllers. These are
scoped control routes, not completed multiplayer races or physical device tests.

The full Windows v15 signed Build/repair [workflow 40612591810](https://github.com/centrexity/GamePKG/actions/runs/40612591810) now passes on actual Windows at `b40bbbe`. Its cold optimized Build completes in about 4 hours 26 minutes on the CI machine. The library recognizes the native build automatically, deliberate SDL-companion damage fails integrity, and normal repair restores it in about 112 seconds while preserving all 487 Rust object hashes/timestamps and the signed package. The verification-only artifact passes its published SHA-256 and archive CRCs; no original media is used and no gameplay is claimed. The executable is 59,915,264 bytes, SHA-256 `fd95c9d028d6d5eb17ae810efa4e0530ee70fd1766a4f6608fddb8cf69caa12e`. This qualifies the v15 build/repair path, not the newer generic Windows source compiler work or a stable release. Physical
Windows original-game graphics/audio/controllers, mixed split-screen
result/driving prompt presentation, natural multiplayer completion, higher medals
and full campaigns remain unqualified. The immutable v14 hardware preview stays
unchanged and does not contain the v15 input fix.


The new private v15b hardware-test bundle pins the verified `081a5b1` Windows
app and ports `d8672b5`, with a separately signed 0.1.4/v15 recipe. All 18,595
app files match the verified native archive byte for byte. Its native game
builder/engine/ports inputs match the successful `b40bbbe` full-game build;
the app additionally retains explicit Steam settings and browser startup fixes.
It deliberately does not include the newer, unqualified generic Windows
source-compiler work. The bundle contains no ISO, original-media extraction,
private creator seed or prebuilt CPU-specific game.

`GamePKG-Windows-Preview-v15b-081a5b1.zip` is 268,594,298 bytes, SHA-256
`776039653964b13ae7ce0c4059ad21b5b500afade02551f67638f94591f82b68`.
Its launcher uses absolute paths beside the script for a separate preview
library and settings; default-off watchdog and disabled automatic updates remain.
A private, network-disabled Wine desktop verifies the actual packaged Windows
CLI signature/import and original CMD launcher, visible v15 library with Needs
ROM, explicit settings overriding invalid inherited settings and normal Alt+F4
close. Automated audio is dummy/null with no host audio device exposed. It does
not build or play the original game during this launcher check. The first v15
candidate and failed private launcher/forced-window-destruction checks remain
preserved; the v15b archive is a distinct immutable file, and old v14 ZIPs are
untouched. Physical Windows original-game GPU, sound and controller testing,
complete multiplayer/campaign qualification and stable publication remain open.

## Verified v14 baseline

The verified 0.1.3 recipe selects v14, with source-kit SHA-256
`7cfd18d26b8a6e51005f75d7cedc1c01925237574bc579df5027b0167413a986`.
It requires the 0.1.18 development app. All 24,003 prepared Rust game functions
are retained across 487 units and 24,035 native exports. The older 0.1.17 stable
app does not include this native engine.

The normal signed Linux package passes a cold native Build, wrong-disc rejection,
ISO-in-place Play, a second launch with its remembered ISO, play-history saving
and integrity checks. Intentional private-cache corruption is detected and
repaired; all 487 unit-object hashes and modification times, signed package,
settings, ROM selections and player data are preserved. The exact repaired
executable passes original Race and PS2-layout Crash controls.

Private tests of that same repaired executable also cover:

| Route | Observed result | Fresh-process check |
| --- | --- | --- |
| First World Tour race, Silver Lake | Second of six, two laps, 222.383 seconds; best lap 105.467 seconds. Original results, reward/save flow and a new Downtown Road Rage event. | Profile/metadata load with identical hashes; the new Road Rage1 event is selectable. This is not a gold-medal result. |
| Newly unlocked Downtown Road Rage1 | Natural Out of time loss with zero takedowns, zero new points and no medal. Original Continue, successful autosave and return to Select Location. | Profile/metadata load unchanged; Driver Details retains one completed event, 1,135 points and one takedown from the preceding race. |

The title's own debug AI drives these tests with reviewed menu/pad inputs. No
score, medal, unlock, event time limit or save result is injected. All four processes use
dummy audio and pass the actual-Vulkan and unsupported-path gates. These are
scoped campaign checks, not verification of all 173 events.

A separate current-v14 run disables the title's debug AI and uses normal steering,
accelerator and boost inputs. It earns the original Downtown Road Rage1 Bronze
medal with one event takedown, 400 points and two newly unlocked events. The
original Totaled result, Continue, autosave, points breakdown and map all run.
A fresh process shows the bronze icon beside Road Rage1, 2/173 completed events,
1,535 global points and two total takedowns. The earned profile and metadata
remain byte-for-byte unchanged through cold loading. Both processes exit zero
with actual-Vulkan and strict unsupported-path gates enabled and dummy audio.
The driver uses the original pause/resume menus between reviewed actions, then
holds normal acceleration continuously to the natural finish. It writes no game
memory, score, event timer, medal, unlock or result. This qualifies that medal and
persistence path; it does not establish an entirely uninterrupted real-time event,
higher medal tiers, all campaign events or physical controller behavior.

A separate unpaused headless check now drives the entire event with normal
steering, accelerator and boost inputs and debug AI disabled. It reaches the
original Totaled/Bronze result with one takedown, 400 points and two new events.
The actual controller-read log shows all five Continue presses reaching the
game. Original reward and successful autosave screens run, and saved bytes
change. A fresh process loads that exact earned save and displays 2/173 events,
1,535 global points, two takedowns and the Road Rage1 bronze icon. Its active
profile, metadata and earned reference remain byte-for-byte identical after
loading. Both owned processes exit zero under the actual-Vulkan and strict
unsupported-path gates, using dummy audio and the ISO in place. There are no
pause transitions after initial unpaused state, game-memory/result writes or
debug-AI activation. This qualifies an unpaused headless event/save path; physical
controllers, Windows hardware and exact wall-clock pacing remain separate.

An earlier unpaused check earns Bronze with two takedowns but stops before
Continue; it does not qualify that result's persistence. Private save-driver
errors are retained separately: a long held input postpones menu commands, and
parse-time input scheduling was incorrectly treated as controller delivery.
The latter attempt finishes with Bronze but never delivers Continue and leaves
the save unchanged; its zero process exit is not a passing save qualification.
The corrected driver bounds queued holds by the observed game clock, waits for
actual controller reads and rejects unchanged earned saves. A negative/positive
check rejects the undelivered trace and recognizes the earlier real delivery.
These are maintainer-driver corrections, not production deadline changes.

The complete v14 Windows archive also builds with actual standalone Windows
Rust under Wine. Its linked executable has SHA-256
`4467bfd5ae1b91565a99cd06792d6695bc2d61e22626f62f4e6b5dbd66ae6540`.
It passes original profile create/reload, unchanged saves, stereo capture,
Race/PS2-layout Crash controls, keyboard menu navigation, 1024×768 resize and
normal close. It finishes an original three-lap Race, fifth of six in 345.483
seconds, with a best lap of 108.517 seconds. Traps, actual-Vulkan and unsupported
path gates are enabled. Wine does not establish physical Windows game behavior.

Actual Windows/macOS host validation passes at app `f87f383` in
[run 40581542080](https://github.com/centrexity/GamePKG/actions/runs/40581542080).
Both hosts pass their test and strict lint jobs. Windows also passes managed
standalone SDK setup, isolated playback fixtures, optional WSL profile checks
and verified ZIP packaging. These host fixtures do not establish original-game
SDL/Vulkan/audio/controller behavior on physical Windows hardware.
The focused [confined Windows window gate](https://github.com/centrexity/GamePKG/actions/runs/40581541946)
also passes at that app revision. It uses production ownership with synthetic
media and no WSL, including the default-off watchdog launch.

The earlier private Windows hardware bundle uses app `f87f383`, ports `e230359`
and the signed v14/0.1.3 recipe with separate preview library/settings. All 18,593
app files match its verified native Windows ZIP, and all 178 bundled port PNGs
match the reviewed source. It includes the native Windows storage and window
color fixes and default-off watchdog. It contains no ISO, private signing key or
prebuilt CPU-specific game. Its complete ZIP SHA-256 is
`19f9a53d9f84c468c003f71f82ef5174c02f7d2cf171951224b49d1f3936c676`.
This is a private hardware-test bundle, not a stable release qualification.
The full v14 native
Windows signed Build/repair gate passes on actual Windows in
[run 40446319498](https://github.com/centrexity/GamePKG/actions/runs/40446319498):
all 487 units build with native optimization, the library recognizes the built
game automatically, integrity rejects an intentionally damaged SDL companion,
and repair restores it while preserving every unit hash/modification time and
the signed installed package. The build takes 7,517 seconds and repair 74 seconds
on that runner. Compiler children can discover only the verified SDK and Windows
System32 tools. This gate uses no ROM and does not qualify original-game playback;
physical Windows game checks remain required before stable promotion.

## Historical v13 Windows routes

The earlier v13 production runtime builder was exercised with the actual standalone Windows
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

The stricter executed-path audit found that the older v13 runtime reaches
`wbinvd` at `0x00353DB5`, `cli` at `0x00367C86` and `sti` at `0x00367C97` in
both profile-create and reload logs. The upstream translator currently reports
these instructions and treats them as no-ops. Historical route and save/audio
results above remain observed outcomes; they do not prove complete instruction
semantics. That v13 executable fails the stronger path gate.

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
the unchanged assertion passes on the taller desktop. With instruction traps
enabled, the Windows candidate also completes the original three-lap race:
fifth of six, 345.483 seconds, best lap 108.517 seconds, normal results flow and
strict path/Vulkan gates. Normal signed Linux Build/Play/repair and the updated
private preview and full v14 Windows signed Build/repair are now verified as
described above; physical Windows qualification remains required. The rejection is not waived because
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

## Earlier app and platform checks

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

The superseded hardware preview used this exact native app and the verified
signed v13 recipe, with a separate library and settings. Every extracted app
file matched the native ZIP. That preview pinned production app `b2b43fd`;
the subsequent `c766be1` app change adds the optional frame-stall diagnostic
watchdog described below. That older preview predates the setting; the current
v14 bundle described above includes it.

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

A current v14 Linux completion probe reaches the original multiplayer module
`003A939C`, with both player phases entering `2` (in play), under strict path
checks and actual Vulkan. It does not establish a race finish: the pinned
`src/game/autopilot.c` helper only permits the single-player module `003A9ACC`,
so its player-one AI never engages here. The owned probe was stopped once this
invalid driver assumption was identified; its reference profile remained
unchanged. Enabling that debug flag is not evidence of multiplayer driving.
A completion check still needs a reviewed ordinary-input driver; production
AI guards, scores and results must not be patched to manufacture qualification.

## Ordinary Windows source compiler diagnostic

This is separate from the passing signed v15 prepared-game Build/repair and the
immutable v15b hardware preview. Ordinary Windows source-game support remained disabled during these diagnostics;
the subsequent normal-loader integration is recorded below.

[Paired Windows controls](https://github.com/centrexity/GamePKG/actions/runs/40651190995)
at `7441a6c` show the pinned LLD version probe succeeds outside confinement with
the same cleared environment, but fails inside AppContainer with either regular
file or private-pipe output. Output writer read-attribute rights do not fix it.
[Focused path checks](https://github.com/centrexity/GamePKG/actions/runs/40651991680)
at `0c5c292` show `GetLongPathNameW` returns access denied for both ordinary and
extended paths, including the probe's own executable. LLVM's Windows startup
uses this lookup before processing linker inputs. An ordinary image path also
fails; no relaxation of confinement or compiler fallback has been added.
The [ancestor diagnostic](https://github.com/centrexity/GamePKG/actions/runs/40652577335)
at `ba013a0` returns the same access-denied result for every ancestor through
`C:\`. Those diagnostics alone did not qualify a compiler fix; no ancestor ACLs were widened.

The optional watchdog remains off by default. Its exact settings persistence and
validation regression passes locally; both above Windows runs pass the separate
real-window/default-off watchdog test before reaching the compiler failure.

### Qualified standalone compiler and shared SDK

[Actual Windows run 40656721386](https://github.com/centrexity/GamePKG/actions/runs/40656721386)
passes both PE32 and PE64 jobs at `b818f303020d9aed22fdde3c7fc7e5fdece41dc3`.
The selected linker is GNU Binutils 2.42 `ld.exe` already included in the pinned
Rust 1.89.0 toolchain. Rust produces its optimized fat-LTO static library; the
host starts the native linker separately. Both stages retain AppContainer/job
bounds, private streams, one absolute deadline and the existing directory grants.
There is no unconfined fallback, additional download or ancestor ACL expansion.

Each actual Windows width passes compiler startup, native PE identity, exact
binary-message exchange and rejection of an ungranted host-file include. The
signed source package passes native Build and generated SDK execution, including
asset reads, drawing requests, high 32 bits of u64 state, scoped save data retained
across fresh processes, and damaged-build rejection. The shared fixture also
passes locally under Linux's normal build/worker isolation and is invoked
explicitly in Linux CI. The separate Win32 window/restart/default-off watchdog
fixture passes in the same Windows run.

The verification-only archives match their published SHA-256 and CRCs: 32-bit
artifact 12802221666 is `24494f318c282fb7cd87332f17de3e817e32beece6d06b199b87cf66ba92e830`;
64-bit artifact 12801504559 is `600e60da6095de684c5a1d831de6248ba2a0896205579455aed6015e9d2a8abf`.
They contain only test logs. Earlier missing-thumbnail/license and incorrect
asset/verification fixture failures remain recorded. The final fixture follows
production's signature verification before asset access. No owned media or
original-game gameplay is exercised here.

This qualified the standalone compiler/shared SDK contract at `b818f30`; its
ordinary Windows source-game entry points were still disabled at that head.
Subsequent shader and normal-loader integration is recorded below. Physical
original-game checks remain open. The
immutable v15b hardware preview and earlier whole-game Build/repair are separate.

### Native Windows source-game and shader entry points

The [shader/compiler workflow](https://github.com/centrexity/GamePKG/actions/runs/40662676099)
passes at `5df761a`, and the [normal-loader integration](https://github.com/centrexity/GamePKG/actions/runs/40670512933)
passes at `0f9c0a0`. Ordinary Rust source packages now use native Windows x86/x64
Build/Play entry points in the development source. The normal loader discovers
the native artifact, verifies its signed package, restores scoped saved bytes,
records actual elapsed play time and rejects damaged native/shader artifacts.
Its explicit-settings test seam shares the production loading body and avoids
process-global test configuration changes. Reviewed engines retain their own
platform support; this does not enable every C-based engine on Windows.

Slang 2026.19 is installed from its pinned official Windows archive without any
vendor installer or user Python requirement. All 4,942 files are checked on reuse;
selected external Slang folders must match that complete reviewed payload. Native
shader compilation uses the shared bounded AppContainer/job runner, private work
files and explicit cleared environment. Linux retains its existing bubblewrap
compiler boundary. Signed SPIR-V/WGSL/GLSL native builds pass reflection and
translation validation, normal game loading and shader requests. Damaged shader
outputs fail playback, and an ungranted host include is denied while the same
file compiles in the deliberate unconfined test control. This is compilation and
runtime-request evidence, not a native GPU pixel/rendering qualification.

The [mixed-bitness workflow](https://github.com/centrexity/GamePKG/actions/runs/40672606863)
passes at `3442e33ec00553849d89973a8c1a1338f518b552`. It includes both PE32/PE64
source regressions, all three native shader backends through normal game loading
at both app/game widths, and the actual Win32 window/restart/default-off watchdog
fixture. The 32-bit app launches the verified 64-bit Slang tool on AMD64 Windows;
OS capability is checked independently of the app's pointer width. Its native
Rust game/compiler remain 32-bit. Genuinely 32-bit Windows can use shader-free
source builds, but still needs a suitable shader compiler before shader support
can be qualified there. These hosted 32-bit processes run on a 64-bit OS.

At that head, Windows DXIL setup, physical GPU/input/audio and full original-game
fidelity stayed open. The initial private audit verified Microsoft's DXC 1.9.2609
Windows ZIP and its 33 files, before production installer integration.
A headless Wine prototype compiles DXIL and Metal using Slang's explicit DXC
*directory* option; the earlier full-DLL-path load failures are retained. That
prototype does not qualify DXIL in AppContainer or production playback. The
existing immutable v15b hardware preview remains unchanged and does not include
these newer ordinary-source features. No whole-game 487-unit build is repeated.


### Confined Windows DXIL compilation

[Run 40676730080](https://github.com/centrexity/GamePKG/actions/runs/40676730080)
passes at `f0d1dc3278265b53ccb68cd374207e3235b5d954`. Both PE32 and PE64 app/game
jobs on AMD64 Windows retain the source and three desktop shader regressions,
then compile signed DXIL through Slang and the pinned Microsoft DXC 1.9.2609 SDK
inside the same AppContainer/owned job. They verify DXBC output and reflection,
cache publication, corrupt DXIL cache rejection on recompilation and the explicit
unsupported-renderer message. Ungranted host includes fail for both SPIR-V and
DXIL, while each same-file deliberate unconfined control succeeds.

The vendor-specific DXC ZIP rule normalizes Windows separators before strict
portable validation. It is confined to that pinned SDK; ordinary tool ZIP and
`.gamepkg` paths still reject backslashes. New fixtures reject traversal,
reserved device names, separator/case/prefix aliases and links. Full official
33-file installation, no-fetch reuse, selected damaged-DLL rejection and repair
pass locally. Both Windows log archives match published sizes/SHA-256 and pass
all CRCs with only the expected logs. This qualifies compilation and integrity,
not DXIL playback or physical graphics/audio/controllers.

The native game-window fixture now passes on actual Windows AMD64 in
[run 40713577705](https://github.com/centrexity/GamePKG/actions/runs/40713577705)
at `d42d7d206ee15e0d580509279622d7a30c2ab4a9`. It builds native-only PE games,
then checks GLSL/SPIR-V/WGSL outputs through test-only software OpenGL: held and
released keys, transparent-black fallback and recovery, resize, normal close,
cached restart and increasing saved play time. All 36 captures pass independent
whole-viewport and letterbox checks, including resized windows. The complete
79-file evidence archive matches its published size, SHA-256 and CRCs.

An earlier capture could include offscreen white edges despite passing center
checks. The fixture now places its own window fully onscreen, records DPI and
capture geometry, and rejects the frozen clipped capture while accepting the
full resized capture. It retains the existing one-level RGB tolerance for the
shader field and requires exact black fallback. The earlier alpha-oracle failure
and clipped captures remain recorded. Python/Pillow are maintainer tools and
all automated audio is muted. The later Windows32/64 Vulkan qualification below extends this software-window
evidence. Physical original-game graphics/audio/controllers remain unqualified.

### Platform admission regression

[Run 40678994989](https://github.com/centrexity/GamePKG/actions/runs/40678994989)
passes at `c3ea1983fc7a3eba3e732a34b7b4c8017186ec30` in both Windows app widths.
The broader portability run had stopped on an obsolete test that expected every
non-Linux native compile to fail: a successful Windows build failed `unwrap_err`,
and macOS expected the older Linux-only shader error text. That failure is retained.
The corrected test preserves unavailable compiler-exchange rejection, reserves
native/shader-unavailable assertions for platforms without those implementations,
and verifies ordinary Windows source-game admission. Unsupported reviewed engine
profiles now reject before tool preparation, staging or build locks/output.
Three exact boundary tests and the existing source/normal-loader/shader/DXIL and
Win32 window/watchdog regressions pass. Both log-only archives match published
sizes/SHA-256 and CRCs. The separate Windows AMD64 game-window qualification above supplies pixel
evidence; these boundary checks alone do not establish graphical behavior.


### Packaged 32-bit native rendering

[Run 40681411252](https://github.com/centrexity/GamePKG/actions/runs/40681411252)
at `cfa8038298cfbaeedb5b7db7fc7fd7ba39888607` passes Linux32, Windows32 and
Android32 jobs. Published artifact sizes, digests, archive CRCs and executable
architectures were verified. Android evidence covers its CLI and emulator tests,
not an Android GUI or native game worker.

The actual packaged Linux ELF32 CLI and GUI also pass local native game-window
checks on a 64-bit Linux host. GLSL, SPIR-V and WGSL outputs pass through software
OpenGL; SPIR-V and WGSL pass through explicitly selected, checksum-pinned private
32-bit Lavapipe Vulkan. Across the five cases, all 25 captured 960-by-720 fields
match every pixel exactly. Tests cover input, texture clearing/restoration,
normal window close with exit zero, cached restart, increasing play history,
and shader/cache/reflection rejection cases. Audio uses null/dummy outputs.
The tests use separate displays, libraries, settings, network namespaces and
private temporary directories. Initial parallel close failures with shared X11
socket paths remain recorded; all five cases pass with private temporary paths.

`8b5ae99` adds these five native GUI cases to Linux32 CI. The first run fails
before rendering because bubblewrap cannot traverse the SDK under private runner
temporary parents. `5382576` places that test SDK under its own traversable `/tmp`
folder without changing compiler isolation or system permissions.
[Run 40725961506](https://github.com/centrexity/GamePKG/actions/runs/40725961506)
then passes all three platform jobs, including the five Linux32 native graphics
cases and packaging. The graphics archive matches its published size, SHA-256
and CRCs with exactly 36 files. Independent inspection verifies all 25 complete
960x720 fields exactly and all five normal-close, cached-restart and increasing
history records. The original failure is retained. This does not qualify a
genuinely 32-bit OS shader toolchain, original-game 32-bit gameplay, physical
GPU/audio/controllers or Android native game delivery.

### Host-independent older-generator reference

The broad all-features [run 40715301601](https://github.com/centrexity/GamePKG/actions/runs/40715301601)
exposes a tiny FPATAN underflow boundary where AMD and Intel physical results
differ beyond C1. The legacy test incorrectly treated the host approximation
as a universal reference. `778b6ee` derives exact-tiny dyadic result bytes, C1,
UE and range wrapping independently with MPFR, retaining all other physical
state checks and the existing numerical-accuracy and monotonicity bounds.

All seven focused tests and all 230 tests across 70 suites pass locally, with
zero ignored tests, strict all-target Clippy and formatting. The complete
616,852,492-byte AMD state corpus is unchanged, SHA-256
`fa7632c0b879c45eb541a066d20f6f47d0be7f5272fcbb1124b90e3af4d2dabd`.
Both opposite-host-C1 and captured-Intel-underflow variants pass; corrupted
implementation C1, UE and numerical bits still fail at the exact boundary cases.
Hosted [AMD EPYC run 40728736496](https://github.com/centrexity/GamePKG/actions/runs/40728736496)
and [Intel Xeon 8370C run 40728749559](https://github.com/centrexity/GamePKG/actions/runs/40728749559)
pass the seven tests and strict Clippy. CPU inventories are checked from each
job, and both one-log archives match published sizes/digests/CRCs. The old
failure remains recorded. This experimental CPU project is separate from the
prepared-Rust game functions and original Takedawn C/C++ player runtime; neither
production numerical code nor game delivery changes. Full-project checks still
need their own terminal results.

### Actual Windows32/64 OpenGL and Vulkan windows

[Run 40728736217](https://github.com/centrexity/GamePKG/actions/runs/40728736217)
at `b2e1b0eaa3575be1093e205eb350eda974c541fc` passes both app widths on AMD64
Windows. Each tests native-only game builds for three OpenGL outputs
(GLSL/SPIR-V/WGSL) and two Vulkan outputs (SPIR-V/WGSL), using private pinned
software graphics. Both archives match published size, SHA-256 and CRCs and
contain exactly 137 expected files. All 120 captures pass independent
whole-viewport/letterbox checks, including resize. Input, exact black texture
fallback/recovery, normal close, cached restart and increasing play history pass.
Actual CLI, GUI, game and graphics-library architectures are checked.

The first Vulkan attempt fails before opening a window. The follow-up native
probe confirms that the CI processes are elevated; the loader normally ignores
an environment-selected driver at that privilege level. The private test loader
honors that explicit selection, without installing system drivers or changing
production loader behavior. Each width confirms its own loader, the exact
pinned Lavapipe DLL hash, Win32 surface support and exactly one CPU Vulkan
device. The test-only loader and probe are not player requirements or releases.

This closes the hosted native source-game window gap for both Windows widths,
including Vulkan. It does not establish genuinely 32-bit OS shader tools,
physical graphics/audio/controllers, original-game Windows gameplay, other
reviewed Windows engine profiles, Android GUI/native workers, full campaigns or
complete multiplayer qualification. Existing preview ZIPs remain immutable;
no full 487-unit cold build, merge or stable release is repeated here.

### Reviewed C compiler startup controls

The pinned Clang frontend still fails its restricted `--version` probe before
any C input is compiled. This remains a failed qualification, and reviewed
Windows C-engine admission stays disabled. The source-game Rust compiler uses
the separately qualified GNU linker path.

The maintainer fixture now pairs that failure with the exact frontend and cleared
environment outside the restricted process, then tests regular-file output with
ordinary and extended executable paths. It also tries a unique temporary
DOS-device name rooted at the already granted toolchain. This last experiment
changes the image's path namespace, grants no additional file access, uses no
drive letter, and removes only its own exact mapping. Creation ownership is
tracked so an unsuccessful creation cannot remove somebody else's device.
It is a test control, not a production compiler fallback or qualified fix.

The hypothesis comes from LLVM's Windows initialization:
[`InitLLVM.cpp`](https://github.com/llvm/llvm-project/blob/main/llvm/lib/Support/InitLLVM.cpp)
obtains its arguments through
[`Process.inc`](https://github.com/llvm/llvm-project/blob/main/llvm/lib/Support/Windows/Process.inc),
which expands the image name before driver options are handled.
[Microsoft documents](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getlongpathnamew)
that this lookup needs access to parent directories.
The actual paired controls must determine whether the reviewed binary has that
same cause; source inspection alone does not establish a runtime fix.

Both Windows GNU target widths pass strict all-target Clippy for these controls.
Actual Windows MSVC execution remains required. The final compiler startup,
target-sized COFF/ABI, denied host include and same-file positive-control
assertions remain mandatory and unchanged.
