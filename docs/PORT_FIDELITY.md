# Port fidelity audit

Initial audit: 2026-10-17; overview reconciled with verified work on 2026-10-19. Compare behavior against each credited, pinned upstream
revision. A playable port or passing smoke test does not prove feature parity.
The implementation tracker is [port-parity.toml](port-parity.toml); unfinished
work stays open until behavior is verified.

## Confirmed missing or changed behavior

| Port | Remaining gaps |
| --- | --- |
| Takedawn | The original native implementation links all 24,003 generated game functions converted to Rust. Race controls pass for Xbox/PS2 layouts; Crash results, profile persistence/reload and non-silent stereo capture have private tests. Signed-package native Build/Play, remembered ISO selection, history, storage, integrity and the separate original window are verified. The standard Build resumed after 456 verified units, compiled the remaining 28 and freshly built the native runtime; its output passes the original race-control gate. Desktop keyboard input creates/reloads a profile and drives the first World Tour race, including pause/resume and normal window closing. Full campaign/progression, steering equivalence, audio fidelity, physical controller and broader platform coverage, production isolation and official publication remain open. Two genuine missing native bodies are recovered and pass 128 C/Rust behavior cases; 13 original unresolved upstream stubs remain. Tested routes do not reach the remaining stubs. |
| Cube 2 | Wayland lock-state access, Windows lock-state runtime validation, remaining uncommon-key/platform behavior and full controller/touch equivalence; further LAN/platform and master authentication/registration fault/service coverage; custom asset/mod workflows; large recording import/export and further movie capture fidelity; hardware-accelerated engine rendering; native platforms beyond Linux. Vulkan currently presents software-rendered engine frames. |
| DOOM | Original WAD music, stereo effects, six save slots, configuration, desktop controls and quit/ENDOOM are runtime verified. Complete playback of all 45 selected-WAD music entries, supported map/save/load music transitions and ordered/merge PWAD cases have private tests. Music backend selection, multiplayer, complete mod/campaign compatibility and broader control/platform coverage remain unfinished. |
| Zelda3 | Original ROM gameplay, stereo and SRAM are verified in native and Wasm. Widescreen/240-line rendering, extra sprites, dim flashes and earlier button-press intro skipping have signed defaults, saved GUI/CLI controls and runtime tests. All original enhancement switches now have signed/saved controls and exhaustive flag-mapping tests; selected combined gameplay and low-health behavior are verified. External MSU music, replay/save-state tools, language selection and complete enhancement/campaign/platform behavior remain open. |
| Arkanoo | Gravity, speed-triggered portal behavior and ball-to-ball responses are restored with upstream reference and actual-engine tests. Original world proportions and movement/contact order now match upstream reference checks. Per-ball launch timing, life recovery and victory/endless transitions also match original-code fixtures; prior item/score/level caps are removed. Brick health and original blast rules are also verified. All twelve original procedural patterns are restored. Focused mouse movement and original Space/Escape controls are restored. Editor painting/erasing, brick selection, names, save/load, clear confirmation, test return and the custom endless pool are restored. Original ASCII pattern import/export now uses player-selected files. Gameplay cursor confinement/hiding, F11 fullscreen and Q-to-quit outside play are verified in native and browser runtime checks. Normal-size/fullscreen/maximized preferences now persist through the native host. The six main menu actions, project-homepage button and fullscreen/effects settings controls are restored. Music/resolution/VSync controls and remaining menu presentation still need work. Original shard particles are restored; the metallic ball, glass paddle and normal-brick textures are restored; speed lettering and six campaign image backgrounds are restored and directly verified; editor image selection is restored; grid composition, editor/menu lettering, whole-window scaling and other art/effects still need work; original penguin/heart-loss behavior is now verified. Current runs do not persist; high score, unlocked level, gravity and effects preferences do. |
| 2048 | Random spawning, merge scoring/best score, win/continue/game-over UI, restart, 128-step undo and persistent boards/history are implemented and tested. Byte-sized tile exponents remove the old 32768 cap. Cooperative expectimax assistance/autoplay matches independent upstream reference fixtures; a deterministic 932-turn game reaches 2048. Final signed-package delivery and broader platform verification remain open. |

Evidence: [README coverage table](../README.md#port-feature-coverage),
[DOOM adapter](../engines/doom/adapters/engine.c),
[Zelda adapter](../engines/zelda3/adapters/engine.c),
[Arkanoo credits](../ports/arkanoo/CREDITS.md), and
[2048 implementation](../ports/2048/source/src/lib.rs).

Cube 2's physical keyboard path now preserves Pause, keypad distinctions and
left/right modifiers, plus media/browser/application keys with SDL equivalents.
The engine accepts and releases keys across SDL's full bounded scancode table.
Actual desktop tests verify independent ordinary and media-key bindings,
the original console and saved preferences. Accepted text stays ordered with
keys, including paste and committed IME input; preedit text is not captured.
This does not establish full controller, touch or platform coverage.

## What GamePKG needs to add

### Adapter review: behavior that must not be counted as complete

The adapter review below distinguishes intentional isolation from missing original
behavior. A successful return from a compatibility function is not evidence that
the original operation happened.

| Source | Observed behavior | Required follow-up |
| --- | --- | --- |
| `engines/cube2/adapters/network.h` | Broadcast reaches the owned UDP socket; real nonblocking TCP now supports bind/connect/listen/accept/poll/shutdown. Reverse lookup returns a numeric address. Bounded buffer sizes, timeouts, address reuse and TTL now reach the actual host socket; error/TTL queries read host results. Socket shutdown now reaches the OS; bounded browser snapshots use explicit virtual EOF/half-close semantics. | Audit each original ENet caller; finish the host operations it needs and test real discovery, authentication, server registration, disconnect and error handling. Keep permission denial explicit. Do not infer complete networking from ordinary packet exchange. |
| `engines/zelda3/adapters/engine.c` | Signed/saved video options configure the original PPU before play: bounded side area, height, sprite limits, dim flashes and early intro skipping. Default language remains selected, external MSU playback is unavailable, and the frame scheduler omits the desktop replay log. | Add selected optional resources, language configuration and bounded replay/save-state persistence, and finish individual enhancement/campaign behavior qualification with actual engine tests. Existing ROM/presentation checks do not verify those missing features. |
| `ports/arkanoo/source/src/menu.rs` and `render.rs` | All five original recorded effects are restored, but the menu covers only part of the original presentation/settings and effects still advance through game ticks. | Restore the remaining transitions, resolution confirmation/revert, VSync and remaining licensed art/audio; verify each against the pinned original. Window-size persistence alone does not implement a resolution menu. |
| `ports/2048/CREDITS.md` | Tile exponents stop at 63 and scores use checked fixed-width storage; the reference implementation has a smaller tile representation. | Retain explicit bounds in documentation and boundary tests. Verified solver/gameplay behavior does not establish unlimited arithmetic, package delivery or every-platform support. |

Single-threaded audio lock no-ops and host-supplied SRAM are intentional
adaptations, not automatically missing features. They still require evidence
that audio ordering and save/load behavior match the original. Conversely,
silently ignored configuration or placeholder success must stay on the open
checklist until a verified replacement exists.

Prioritize shared resource selection/persistence and complete input first: they
unblock mods, optional music, recordings and original controls across several
ports. Next complete the networking adapter and graphics/presentation controls.
Track each port's remaining assets and menus separately. Full campaigns,
cross-version saves, Windows/Android gameplay and rebuilt signed catalogs remain
delivery gates even after individual features pass.

- Complete ordered keyboard, pointer and controller events, including original
  bindings and rebinding, through host-controlled input.
- Scoped resource import and enumeration for mods, extra WADs, custom maps,
  language assets and optional music. Preserve original user files in place.
- Scoped persistence for all save slots, configuration, replays and recordings,
  with versioned save compatibility and explicit import/export.
- Channel-aware audio throughout each adapter, plus music backends and controls.
- Scoped networking for discovery, authentication and multiplayer workflows.
- Per-game enhancement settings where the original engine exposes them.
- Hardware graphics and native platform support where currently absent.

These require bounded host APIs or virtual resources. Keep filesystem, network
and process isolation while restoring functionality. A stub is only acceptable
when a verified host replacement provides the same observable behavior.

## Remaining original presentation is not cosmetic completion

Arkanoo's pinned `rendering.rs` also has dedicated implementations for ball
speed lettering,
bonus icons, animated backgrounds, portal rendering, HUD/FPS, pause/settings,
game-over, level-transition/victory and editor presentation. Background/menu/HUD presentation and bitmap lettering still contain replacements.
All five recorded effects are restored; original music and asynchronous effect
timing are not yet connected to the port. Restored object art and portal stages have the measured
grouping and sampling limits below; they do not verify those remaining paths.
Keep each original drawing path open until its behavior has reference evidence;
matching a few screenshots is insufficient for animated or stateful effects.

The same rule applies to engine adapters: unavailable features must report failure
or remain visibly unsupported. Cube 2 shutdown/listen/accept now reach owned TCP
sockets; UDP broadcast also reaches the host socket. Private IPv4 LAN discovery now has actual-engine verification below; platform
and further fault coverage remain open. Zelda's ignored MSU enable remains a confirmed
gap. Shared host work still needed is:

- Selected resource collections/directories and bounded streaming for mods,
  optional music, recordings and language assets, retaining user files in place.
- Complete network lifecycle/discovery/authentication behind signed permissions.
- Original resolution/VSync controls, hardware graphics and complete input paths.
- Versioned replay/save-state/resource configuration and import/export.

## Completion evidence required

Editor behavior now has independent fixtures from the complete unchanged original
editor module. The custom endless pool matches 106 original-code layouts with
normal, special and empty patterns. Native/Wasm runtime checks cover archives,
legacy migration, denied saving and damaged-data preservation. Rendering,
original fonts/art remain separate gaps. Original ASCII pattern import/export
uses signed, purpose-scoped file dialogs. Actual native/core/component tests
cover cancellation, invalid text, permission denial, selected-file preservation,
exports to a chosen destination and archives larger than 64 KiB. Linux system
Open/Save dialogs and both browser upload/download profiles pass. The optional
browser Save-picker path also has cancellation/stale-result unit checks.
Directory/mod-resource browsing, streaming assets and large recording exports
remain open for the other engines.

The ordered source-input changes have passed the full Rust suite, permission and
malformed-buffer checks, native gameplay, and core/component browser gameplay.
Arkanoo tests exercise mouse scaling, Space/Escape controls and Restart; SDK
fixtures also cover Unicode, layout-aware keys and denied input permissions.
Both 32-bit and 64-bit Windows builds pass compilation checks; this is not
Windows gameplay verification. These checks do not close the remaining feature
gaps listed above.

Cube 2's rebuilt desktop fixture also passes original startup, console, saved
preferences and physical-key regressions with the explicit OpenGL test backend.
The headless display lacks Vulkan presentation support (DRI3), so this round
does not establish Vulkan playback. The application's Vulkan default is unchanged.

For every restored feature, test its normal path, denied-permission behavior,
malformed input and persistence where applicable. Verify original bindings,
menus, audio channels/music, save/load across every slot, resource selection,
and multiplayer using actual engines. Compare replacements with the pinned
upstream behavior rather than merely checking that they run.

Run full level/campaign coverage separately from startup and short gameplay
tests. Test cross-version saves and each claimed native platform. Rebuild and
verify the packages and catalog users import after source changes. Record
unverified behavior explicitly; do not silently skip it or describe it as done.

Latest local validation: the full Rust suite passes 375 tests. Its 50
fixture-dependent or platform-specific ignored tests are not counted as passed.
The signed isolated Arkanoo harness separately executes 26 fixture tests on Wasm
and again with native builds, plus actual native-window and core/component
browser checks. Both Windows word sizes pass compilation checks; Windows gameplay,
full campaigns and updated signed-package delivery remain separate requirements.

Host-owned per-game window records now preserve normal size, fullscreen and
maximized state across launch and close. Linux desktop regressions verify normal
resize, F11, reopening fullscreen and maximized/fullscreen combinations, restoring
the original normal bounds, quitting through game controls and the window manager,
library-window isolation, 100%/150% interface scaling without progressive size drift,
and malformed-record fallback without overwriting data.
Records are bounded and scoped by game/creator/library; symlink/device rejection,
invalid dimensions, minimized-state handling and storage accounting have regressions.
This restores window persistence, not the original resolution/VSync menu, default
presentation or remaining original menu/music/artwork behavior. Windows
32/64-bit compilation checks pass; Windows window-lifecycle behavior remains unverified.

The six main-menu actions now have mouse targets projected from the pinned
original menu: New Game/Resume, Restart, Gravity Mode, Level Editor, Settings and
Quit. Eighty-one independent cases compile the complete unchanged original menu
module and verify integer hit boundaries for these buttons and the supported
fullscreen/back controls. Actual native/core/component tests verify activation,
pause preservation, settings, editor entry and quit. Linux native and both browser
profiles also click Settings/fullscreen/back/New Game through their actual windows.
Browser fullscreen now preserves the canvas aspect ratio and maps absolute/locked
pointer motion to the displayed image, excluding borders and aspect-ratio bars.
Remaining menu presentation and music/resolution/VSync controls,
fonts/background artwork and music remain open; the homepage action is verified below.

Effects settings now preserve original range/default, mute, integer dragging and
clamping. The `ARK3` codec stores effects flags without reducing the 1 MiB pattern
capacity and migrates valid `ARK1`/`ARK2` archives. Tests preserve malformed/future
records and show warnings for unavailable saving. Actual native/core/component
PCM verifies gain, mute/zero and relaunch; native/wasm32 output at the maximum
frame value verifies the fixed 32-bit phase overflow. Linux native controls and
both browser profiles pass real pointer drags, mute, persisted flags and rendered
settings restoration after Restart. Original recorded effects/music and broader
settings/platform fidelity remain open.

Cube 2 socket-option regressions now inspect actual OS receive/send buffers,
address reuse, timeouts and TTL before and after binding a signed listening
endpoint. Options are reapplied before bind, so address reuse affects binding.
Malformed requests, invalid/stale handles, excessive values, blocking mode and
unauthorized broadcast remain rejected. The production C shim is checked for
option/query framing, denied operations, malformed replies and unchanged query
outputs on failure. Discovery snapshots explicitly support nonblocking mode and
connection-error queries without exposing an arbitrary TCP connection. These
checks do not establish full network feature parity.

After the socket-option update, the separately executed two-original-engine
regression rebuilds the engine and verifies direct private multiplayer, continued
background execution for 35 seconds, connection survival and clean saved shutdown.
The full Rust suite passes 367 tests with 47 fixture/platform tests ignored; the
explicit engine test is additional evidence, not a claim that all ignored tests ran.

The discovered-server variant also rebuilds and runs both original engines against
a private local master fixture. It verifies the original browser writes the
returned server to `servers.cfg`, joins it, preserves the background connection
and shuts down cleanly. This exercises the snapshot stream's nonblocking/error
queries as well as the UDP option protocol; no public server is contacted.

Arkanoo startup, splash skip/timing, restart timer lifetime and editor return now
match independent pinned-original state observations. Splash clicks are consumed
once; editor return retains the paused run and New Game/Resume state. Replacement
splash artwork, original music/audio, remaining display/settings controls and
other visual fidelity are still open. Saved preferences remain a GamePKG feature;
active-run persistence is not implemented.

The updated signed Arkanoo harness passes all 24 runtime tests on Wasm and again
with native builds where supported, plus the real Linux native window and both
core/component browser profiles. Their setup now explicitly skips the splash
before exercising menu, effects, gameplay, editor and system-file controls.
The failed-history regression skips the splash and requests the file operation
in the same first tick, preserving its rollback boundary. Full Rust validation
passes 368 tests with 48 ignored; lint, browser input/window checks and public
site-link validation also pass. These are not Windows/Android gameplay tests.

Arkanoo shard particles now restore original lifetimes, rotation/spin, gravity,
fading, diamond-point shape and launch/brick/record/portal burst rules without
the former 64-particle cap. An independent oracle compiles the unchanged original
Particle implementation and draw function; 114 observations cover each ordered
point and age/alpha value, including negative/edge coordinates. Actual native and
wasm32 image pixels match these observations. A 200-brick burst retains
2,000–3,000 shards within the unchanged host-call, draw-command and image budgets. The layer
uses occupied 128×128 tiles; full-viewport corner coverage prevents silent
loss through the per-texture dimension limit.
Disjoint random-word fields and rejection sampling preserve independent uniform
ranges without per-field host RPCs. Native desktop and both browser profiles pass
ordinary gameplay/editor checks with the restored effects. Ball trails/fireball glows and penguin/heart-loss behavior are restored below;
remaining original art/audio stay open.

A hosted dense-frame timeout persisted after isolating the tests. Thousands
of individual entropy round trips dominated the native burst cost. The trusted
SDK now batches 64 host-generated words in a fixed 512-byte cache while charging
each consumed value against the original shared 8,192-call allowance. Particle
generation, random sampling, geometry, pixels and all time/memory limits remain
unchanged. A two-core local comparison reduces 5,000 random requests from about
13.8 ms to 0.33 ms and the complete dense frames from 22–35 ms to 7–12 ms.
The signed 72-case native and Wasm suites pass. Protocol checks retain full
little-endian values, exact shared call boundaries across cached/drawing calls,
per-tick reset, and rejection of malformed/truncated blocks; a failed cached
random tick preserves both the committed save bytes and modification time.
Hosted confirmation of the optimized native frame remains pending.

Arkanoo ball-history trails now follow the original pre-physics 800/1400 px/s
thresholds and one-entry trimming, including the original tier-drop behavior.
Regular/super comet length, radius, gradient, fading/pulsing and spinning ring
match unchanged original drawing fragments. Full-file hashes guard the oracle;
49 original update observations and 16 drawing cases supply 15,909 ordered points
and colors, including negative/edge positions, wide history and u64::MAX frames.
Actual signed runtime tests compare the resulting clipped RGBA pixels through
the bounded tiled renderer. The ball's metal artwork, original speed-text font,
other remaining art/audio stay open.

The animation clock now retains the original u64 width, including continued play
past u32::MAX and pause behavior. Native/wasm32 clock probes and effect cases
extend through u64::MAX. Synthesized audio reduces the u64 phase before adding
the bounded sample index; actual PCM checks cover both u32::MAX and u64::MAX,
and a u128 reference checks all four tones without arithmetic overflow.

Final trail/clock validation passes 375 ordinary Rust tests with 50 fixture/platform
checks ignored. The signed isolated harness separately runs all 26 runtime tests
on Wasm and again with native builds where available, followed by actual Linux
native-window and both core/component browser gameplay/editor/file controls.
Lint, formatting and the public site's local-link checks pass. These results do
not establish Windows/Android gameplay, full campaigns, original ball/font art
or the other open port requirements.


Penguin/heart-loss restoration now matches 337 observations compiled from the
unchanged original loss/update implementation, including the original final-life
GameOver freeze. Sixteen static poses compare exact ordered RGBA rectangles;
three fallback hearts compare original points. Native/wasm32 runtime tests verify
all traces, original heart PNG bytes and HUD destination coordinates. The host
now supports explicitly linear-filtered asset draws alongside nearest-neighbor
draws, with unchanged path, texture, command and per-frame budgets. Its one-image
native/Wasm cache remains instance-scoped and every draw is charged fully.
Permission/path rejection, malformed calls, cache reuse, filtering changes and
failed-tick rollback have regressions. Original fonts, metal ball, background,
remaining menus and recorded audio are still open.

Current validation supersedes the historical totals above: 380 ordinary Rust
tests pass; 52 fixture/platform tests are ignored by that command and are not
counted as passed. The isolated signed harness separately passes all 28 Arkanoo
runtime tests on Wasm and again with native builds where available, followed by
actual Linux native and core/component browser gameplay/editor/file checks.
This is source verification, not updated signed-catalog delivery or a claim of
Windows/Android gameplay, full campaign coverage or complete port fidelity.


The final source audit also checked engine-build patches and virtual-file
allowlists. Cube 2 saves selected config/map/image/demo extensions within fixed
quotas; movie outputs and arbitrary mod trees must not be assumed persistent.
Zelda stores SRAM, not desktop replay/save-state files. These limits require
explicit workflows and behavior tests, not broader ambient filesystem access.
Both Windows word-size compilation checks, all browser unit scripts, lint,
formatting and seven-page public-site link validation pass for this source.
No public package/catalog release was made by these checks.


Networking policy update: current source uses a signed on/off flag. Enabled
networking no longer restricts destinations, sources, DNS names or listening
addresses to endpoint lists, and local/private addresses need no extra opt-in.
Legacy transport names map to enabled/disabled and legacy endpoint fields are
read-only compatibility data. This removes policy restrictions, not missing
adapter functionality: broader LAN/platform coverage, authentication/registration faults and public
service interoperability still need engine tests; the TCP lifecycle and
private successful exchange are restored below.


Network-switch validation: the full Rust command passes 383 tests with 52
fixture/platform checks ignored. After adding old-identity migration, all seven
saved-server-choice tests pass separately, including preservation across the
schema change and rejection of cross-creator reuse. Signed native packages pass
TCP/UDP echo with no endpoint/local grants and disabled-network denial; signed
core/component packages pass unlisted local TCP echo and disabled HTTPS/TCP/UDP/
listener denial. The production C++ ENet shim's malformed/framing checks pass.
Lint, both Windows compilation targets, browser unit checks and site links pass.
These tests do not establish the unfinished full engine networking features.


The TCP broker now restores original nonblocking connect, bind/listen/accept,
readiness, OS error/option queries, partial streams, peer EOF and shutdown.
UDP now starts unbound and binds its existing descriptor rather than silently
replacing it; repeat-bind, connected-peer and shutdown behavior reaches the OS.
The original list browser explicitly requests its bounded sanitized snapshot;
authentication/registration uses real streams. Snapshot target changes discard
late old results. Shared handle/open/message/transfer limits remain unchanged.

A private original-engine regression passes registration plus the master's
original UDP information ping, original cryptographic challenge/response and
authenticated privilege. Original `saveauthkeys` and offline relaunch restore
`auth.cfg` through an exact C++/Rust save-name allowlist entry. An independent
fixture compiles unchanged hash-verified original cryptography with a public
test credential; no real account key is packaged or published. Further LAN/platform, authentication/service fault cases and release delivery stay
on the open checklist.


Latest TCP restoration validation: the full Rust suite passes 390 tests with 53
fixture/platform tests ignored. The eleven focused broker regressions pass again
after the failed-reconnect side-effect fix. Four ignored original-engine checks
run separately and pass: registration/authentication/key reload, sanitized browser,
direct two-engine multiplayer and discovered-server multiplayer. Both connection
variants retain background play for 35 seconds and save cleanly. Production C++
network framing and hostile saved-file tests, source hashes, lint, formatting,
browser unit checks and the seven-page site-link check pass. CLI and GUI binaries
are rebuilt locally; this is source validation without a new package release.


Private-LAN validation now runs two original engines in separate network
namespaces joined only by a private Ethernet bridge. The helper refuses the
calling host's network namespace, never attaches a physical interface and tears
down its owned processes/namespaces. The networking-disabled and LAN-search-off
runs leave the browser empty and produce no captured discovery datagrams. The
enabled run sends a real limited broadcast to port 28784, receives the same
original ping token from the custom server's information port, saves the
correct discovered address/port, clicks its original browser row and joins.
Both sides confirm the remote identity and close successfully. Packet metadata,
phase screenshots and server lists stay under ignored `target/cube2-lan-proof/`.

The fixture waits for the original game welcome/player identity, not merely the
ENet transport connection: changing a name before welcome is discarded by the
unchanged original `addmsg` guard. After the first startup frame, both sides now use a 500 ms ordinary tick
deadline, including the first remote-model/shader use. The original-engine LAN
regression passes without a separate server warmup period. Reviewed cold-resource
notifications use a fixed loading allowance; this does not prove that every
software-rendered frame meets 500 ms. Broader renderer performance remains open.

Authentication coverage now rejects a valid response generated with the wrong
private test key, ignores a success for an unrelated request ID, verifies no
privilege was granted, and retries with the correct key under a new request ID.
The corrected key authenticates, replaces the bad key in `auth.cfg` and survives
offline relaunch. Master disconnection/backoff, registration refusal/retry,
multiple users/domains, further service interoperability and platforms remain
separate open requirements.

The selected local media fixture also reports a missing
`data/menus/browse_textures2.cfg`, referenced by its own `data/menus.cfg`.
Network validation does not establish texture-browser/editor completeness with
that incomplete fixture. Complete matching media and asset-version validation
remain part of the content/editor work; these tests do not alter selected media.


Latest validation for these checks: 390 regular Rust tests pass, with 54
fixture/platform tests ignored by that command. The original authentication
failure/retry/key-reload test and the private Ethernet LAN test run separately
and pass. The LAN scenario has repeated successful runs after correcting its
welcome-state synchronization. Proof directories are unique per run so concurrent
attempts cannot overwrite packet evidence; `verified.txt` identifies a successful
run. Lint, formatting and all seven public-page link checks pass. No production
network policy or runtime permissions were changed by these test additions.

Resource-loading follow-up: the staged, pinned Cube 2 engine now announces cold
model loads, first geometry/material use, shader compilation/first binding,
texture/cubemap uploads and surface reallocations. Model and shader cleanup reset
the first-use state. Notifications coalesce once per input frame; the existing
fixed 60-second allowance cannot be renewed by additional markers. Cached
resources retain the ordinary tick deadline. A production-header regression
covers cold/hot frames, shadow-before-material use, cleanup and shutdown guards;
the original-engine sanitized server-browser regression also passes after a fresh
build. The isolated original-engine LAN regression also passes with the server warmup
removed and a 500 ms ordinary deadline on both sides after startup. Host-reader
tests verify delayed cold-frame success, ordinary-frame timeout, fixed loading
timeout, duplicate-marker rejection and ordinary-deadline restoration. Worst-case
first-use timing and graphics-driver coverage remain unverified.

Loading follow-up validation: the complete default all-features suite passes
392 tests with 54 dedicated media/platform checks ignored. The original-engine
browser and cold private-LAN checks were run explicitly and passed separately.
Strict lint, formatting and the seven-page public-site link check pass. These
results establish the tested Linux software-renderer paths, not full port parity
or hardware/platform coverage. No binary/package release is created.


Desktop lock-state follow-up: the trusted native input hook now queries Caps Lock
and Num Lock from its existing X11 window connection, or the UI thread's Windows
keyboard queue. It emits changed state only to the registered focused game
viewport. X11 queries use XKB's locked modifiers and the actual keysym modifier
mapping rather than assuming Num Lock is always Mod2. No XWayland connection is
used as a substitute for native Wayland state. The X11 virtual-display regression
verifies all four initial combinations, unchanged-state deduplication, silent
unfocused sampling and focus-return resynchronization. A narrowly scoped trusted
UI FFI allowance follows the existing clipboard integration pattern; guest source
restrictions are unchanged.

Cube 2's protocol and reviewed engine now accept precisely the Caps/Num bits in
addition to their earlier modifier mask. Host keyboard, mouse and wheel paths
retain those bits, including after held keys are cleared. Unknown bits remain
rejected. Windows uses GetKeyState's toggle bit; its runtime behavior and native
Wayland state access still require verification/implementation. X11 state queries
are current-state snapshots; exact historical modifier ordering for rapidly
queued external lock changes remains unverified.

API references: [XKB state and keysym modifier mapping](https://www.x.org/archive/X11R7.7/doc/libX11/XKB/xkblib.html)
and [Windows toggle state](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getkeystate).

Lock-state validation: 394 default all-features tests pass, with 55 dedicated
media/platform tests ignored. The original-engine lock-packet/game-binding test
passes separately with an original-limit player name and clean scoped saving.
The signed package rebuild/cache/integrity test passes after updating its loading
fingerprint and obsolete LAN-opt-in expectation. A freshly rebuilt Vulkan GUI
passes original menus, console preferences, physical Pause/keypad/right-Ctrl/media
bindings, focus changes and close with Caps/Num enabled before launch on its
private X11 display. Both i686 and x86_64 Windows all-features build checks pass;
these are compilation evidence only. Strict lint, formatting and public-site link
checks pass. Source-only publication retains app version 0.1.17.


Ordered pointer follow-up: Cube 2 now consumes all eight original physical mouse buttons on X11
and both wheel axes from the same native event sequence as physical keys and
accepted text. Mouse/wheel events carry the held modifier sides and current lock
snapshot at their position in that sequence. Primary/secondary clicks no longer
collapse into a once-per-paint held mask. The click used to regain mouse capture
is suppressed. Fractional wheel movement retains logical-pixel scaling and clears
on focus loss. The host resets this state when starting a new game.

The portable keyboard mapper also no longer feeds Cube 2 alongside its physical
key path. This removes unexpected controller actions from original keys such as
Z and avoids fabricating left-Shift while right-Shift is held. Controller/touch
actions still use their existing portable mapping. Reviewed mouse state combines
physical and controller sources; either source can release without releasing the
other, and focus reset releases their shared effective state once. The production
C++ state regression covers both overlap orders, complete clicks and reset.

A signed private package rebuild/cache/integrity regression passes. With the
freshly rebuilt Vulkan GUI and original engine, all eight original mouse bindings,
four wheel directions, physical Z without an extra fire action and a rapid
right-Ctrl/middle-click/key-release/wheel burst produce the exact saved result.
Original script state observes right=1/left=0 during the click and 0/0 at the
following wheel event. Original menus, console preferences, physical Pause,
keypad, right-Ctrl, media-key focus reset and close still pass with Caps/Num
initially enabled. No media is published. The X11 test remaps its tenth physical button to logical 10/11/12 to exercise
original MOUSE10/11/12 as well as the five common buttons. Native Wayland
additional-button normalization, relative-motion placement within the
event sequence, broader held-input recovery and OS-event-loss behavior, physical-controller hardware verification, native Wayland state and wider
hardware/platform coverage remain open.


Modifier timing follow-up: native host packets retain event-time modifier
snapshots, and the bridge now queues private SDL state markers in the same order.
The staged original event loop consumes those markers before dispatching each
following input event. SDL_GetModState consumers therefore receive the recorded
state during dispatch rather than the final state of the entire input batch.
The producer also retains the final snapshot outside dispatch. Focus reset queues
its zero snapshot before generated release events. Foreign user
events are not consumed; invalid internal markers and failed/filtered marker
queue writes stop the engine instead of silently changing input semantics.

A deterministic original-console regression failed before the fix in both
directions: plain Left followed by Ctrl produced Xabcdef, while Ctrl+Left followed
by Ctrl release produced abcdeYf. The unchanged console now produces abcdeXf and
Yabcdef respectively, proving both the later-press and later-release cases. The
production-header regression independently checks keyboard/mouse/wheel ordering,
Caps/Num snapshots, standalone snapshots, foreign events and six rejected
marker/queue cases. Both engine builders stage the same hook and helper; cache
identity and the signed-package cache regression include the helper's contents.

Ordered-pointer validation: the final all-features suite passes 399 tests with
55 dedicated media/platform checks ignored. Production loading/mouse C++ state
checks run through the normal suite. The signed package and eight-button original
GUI regressions run separately and pass; strict lint and formatting pass. Both
app entries are rebuilt locally. This is source progress, without a new release
and without a claim of complete input, rendering or cross-platform parity.

Modifier validation on the final source: 400 default all-features tests pass with
56 dedicated media/platform checks ignored. The original-console before/after
regression passes explicitly, as do the signed-package cache/integrity test,
eight-button/wheel/ordering GUI check and original startup/lock/focus/close GUI
check. Both Windows all-features build checks, strict lint, formatting, all 125
pinned Cube 2 source hashes and seven public-page link checks pass. Both app
entries are rebuilt locally. This closes the reproduced SDL modifier lookahead
issue; wider input/port parity remains open and no release is created.


Keyboard-source overlap follow-up: reviewed input state now tracks physical and
portable-action keys separately for every accepted SDL scancode, emitting changes
only when their combined state changes. Either source can release first without
cancelling the other. Physical repeat key-downs remain explicit; malformed repeat
key-ups are rejected by both the host encoder and reviewed engine. Focus reset
releases both sources once, and later stale releases do not create another edge.
The old held-mask comparison no longer competes with physical key state.

Controller Left Shift also contributes to effective modifier snapshots for
keyboard, mouse and wheel input. The engine retains the physical modifier base
separately, preserving right Shift, Caps/Num and other physical modifiers when
controller Shift changes. The SDL scancode/modifier constants are asserted at
compile time. A production-header test covers every accepted scancode, both
source/release orders, repeat, reset, invalid indices and combined Shift masks.
The unchanged original engine's W and Left Shift bindings verify both overlap
orders and one focus-reset release through saved callback counters. These tests
exercise the controller/touch action boundary; full physical-device, gesture,
broader lease-recovery, relative-motion and platform coverage remain open.

Keyboard overlap validation: 402 default all-features tests pass with 57 dedicated
media/platform checks ignored. The original W/Shift overlap and original-console
modifier-timing regressions pass explicitly. Signed package rebuild/cache,
playback/save/integrity, all-eight-button/wheel/ordering GUI and initial-lock
startup/focus/close GUI checks pass. Both Windows all-features checks, strict lint,
formatting, 125 pinned source hashes and seven public-page link checks pass.
CLI and GUI are rebuilt locally. This is source-only progress at version 0.1.17;
full controller/touch hardware and overall port parity are not claimed complete.


Native touch identity follow-up: mouse emulation in the window adapter now
belongs to a complete device/finger pair. Previously it compared only the
finger number, so another native device reusing that number could move or end
the emulated pointer. Only a fresh Start can acquire ownership; duplicate starts
and unmatched Move/End/Cancel events cannot create or take over a gesture. The
adapter still forwards all raw touch events independently for multi-touch game
controls. Production-helper integration tests cover zero, ordinary and maximum
IDs, matching fingers on distinct devices, distinct fingers on one device,
duplicate starts, orphan events, terminal events and reacquisition. The full
737-test release suite, strict lint, 32-bit Windows library compilation and
real native Cube 2 mouse/focus/window regression pass. This proves the routing
change and host regression coverage, not multi-touchscreen hardware behavior.
The cancellation follow-up below completes emulated-button cleanup and focus
interruption; pointer disappearance alone intentionally does not release a
pressed button, so a separate cancellation event is required.

Native touch cancellation follow-up: the pinned egui 0.31.1 host fork adds
`PointerButtonCancelled`, which releases the specified button without a click.
The adapter emits it for an owned cancelled touch and cancels its touch owner
when the window loses focus. Raw touch cancellation, pointer disappearance and
the emulated button now end together. Unrelated fingers and late events cannot
release another gesture. Other held mouse buttons remain held; ordinary mouse
release behavior is unchanged. No synthetic movement or focus change is used.

The real-widget tests warm the layout before pressing, verify same-frame and
later cancellation never activate a button, reject duplicate cancellation and
require a fresh tap to activate the widget afterward. A red case demonstrates
that `PointerGone` alone retains the original pressed state. All three tests
pass on host and actual ELF32 with the production patched egui. A private X11
window test sends native touch/focus events through the actual adapter and egui
context, covering ownership, interruption, orphan events and fresh-tap recovery;
it executes exactly one required test. These are constructed native events,
not a physical touchscreen qualification. Windows i686 library compilation,
strict lint, CLI/GUI builds and all 740 standard tests pass; the real native
Cube 2 mouse/window regression also passes after its private fixture rebuild.
CI includes the cancellation, native-window and two-width gates. The fork keeps
the original version, both upstream licenses and source revision, with only
two modified upstream Rust files. Physical devices and wider platform coverage
remain open.

Touch mouse-control follow-up: the first-person desktop players accept a drag
that begins in the upper two-thirds of the game window. One device/finger pair
owns relative motion until it ends or is cancelled; other fingers can keep using
the bottom movement/action regions. A camera finger crossing into those regions
does not acquire button actions. Positions are converted from logical window
coordinates using display scale, with fractional movement retained. Lifting a
finger, cancellation, invalid input, focus loss and game reset release ownership;
an unmatched move cannot reacquire it. Button contacts also require a fresh
Start after focus loss. Other games retain their existing touch regions without
camera motion. Mouse sensitivity and the original games' mouse semantics apply.

Three tests cover ordered/scaled/fractional drags, device identity, multiple
fingers, End and Cancel, bounds, focus/reset/invalid input, and combined camera
and action contacts through actual egui window polling. The original window
test exposed a nested context-read deadlock, corrected by reading display scale
before entering the input closure. The full 735-test standard suite passes.
`tools/check_touch_look.py` builds the actual gesture implementation and its
ownership/motion test against pinned real egui; host and actual ELF32 execution
pass. A fresh private Cube 2 native build also passes its real Vulkan-window
mouse capture, menus, release/recapture, focus and close regression. Physical
touchscreens, OS touch-to-mouse emulation and full controller/touch/platform
equivalence remain unverified.

Controller camera timing follow-up: right-stick motion now integrates elapsed
window time rather than treating a stick rate as a displacement on every
update. The original dead zone and 60-Hz turning speed remain, while fractional
movement is retained for slow turns and fast displays. Neutral input and direction
changes discard stale fractional travel. Focus loss, game reset, invalid or
backwards clocks and gaps over 100 ms discard pending travel rather than replay
a camera jump. Aggregated rates remain bounded to 64 reference units per axis;
one accepted interval can produce at most 384 mouse units. Native mouse deltas
and game sensitivity preferences remain unchanged.

Production-helper tests verify equal travel within one mouse unit at 10, 20,
30, 60, 75, 120, 144 and 240 updates per second, including slow input, both axes,
varying intervals, neutral/reversed input and focus/time discontinuities.
`tools/check_controller_look.py` compiles the actual helper and its actual tests;
`--32-bit --linker gcc-11` executes them as ELF32 on the local multilib setup.
Both widths pass. CI runs both widths with its installed compiler. This fixes
the host calculation shared by the first-person ports; physical controllers,
device-specific calibration and complete controller/touch parity remain open.

Input-lease recovery follow-up: the 250 ms stale-input release is unchanged. The
background worker now retains bounded, UI-observed physical key/button state
through lease expiry, but clears it on explicit focus/input reset. Only a fresh UI
submission can resume it. Fresh transitions are processed first; still-held,
untouched controls are then reasserted once, with modifier keys before other
keys and mouse buttons. Completed taps and controls released in the fresh batch
are not restored. The first repeat for a previously held key can resume an
effective press; subsequent repeats keep their normal semantics. Recovery is a
new effective press after the safety release, not a reconstruction of every
historical input condition across that discontinuity.

Old queued events retain their order and stay released at stale-only frame
boundaries. A queued recovery boundary separates that history from fresh input;
fresh controller edges and look deltas wait for it, preserving their latches
without repeated reset/repress cycles. Existing 64-event/frame and 1024-event
queue bounds remain; oversized batches, malformed recovery-state packets and
recovery overflow fail before replacing the last good input state. The cache
contains at most the accepted scancodes/buttons and stays in memory only.

Deterministic queue tests cover release/resume, modifier/button snapshots,
completed taps, repeat, focus reset, multi-frame backlogs, deferred controller/look
input, quota failure and rejected-state atomicity. An original-engine test feeds
the actual queue output into original W/mouse/function-key bindings, verifying
exact saved callback counts including the backlog/controller boundary. A second
test runs the real background worker with no UI submissions until wall-clock
expiry, confirms rendering continues, resumes held controls, releases them and
saves the expected callback count without replaying a completed tap. Physical
hotplug/lost OS-event detection, broader recovery bindings and remaining input,
controller/touch/platform parity still require coverage.

Lease-recovery validation: 407 default all-features tests pass with 59 dedicated
media/platform checks ignored. Both original-engine recovery checks pass
explicitly, including real worker expiry and scoped save persistence. Signed
cached playback/integrity, uncontended eight-button/wheel/ordering GUI and
initial-lock startup/focus/close GUI checks pass. Both Windows all-features
checks, strict lint, formatting and seven public-page links pass. Both app entries
are rebuilt locally; the unchanged reviewed engine is reused by its verified
cache. This is source progress without a new release or a claim of full port,
physical-device or every stalled-UI/input-history equivalence.


Arkanoo metallic-ball follow-up: the old plain white circle is replaced by the
original procedural metallic texture. The unchanged pinned Rust drawing function
is compiled to emit its point/color calls; real SDL software rasterization and
target clipping produce the independent 12×12 RGBA fixture. The generated PNG
matches all 576 decoded bytes. Rendering uses the existing filtered asset API at
original integer-truncated rectangle positions and half world scale, including
fractional and negative-coordinate probes. No external font, music or selected
player media is added. The speed label, other artwork/settings/audio and overall
port parity remain unfinished.

Metallic-ball validation: the independent PNG/SDL reference test and full default
suite pass (408 tests, 60 dedicated checks ignored). The signed harness separately
passes all 29 runtime tests in Wasm and native builds, including exact ball pixels,
linear draw kind, fractional/negative integer rectangles, heart texture separation,
portal ring/orbit, physics, menus, input, persistence and bounded asset behavior.
Native GUI gameplay/editor/file-picker workflows and both core/component browser
players pass. The reference generator checks both pinned source-file hashes before
compiling its unchanged drawing fragment. Arkanoo's source recipe is 0.1.21;
no signed package or app release is created. Other art, speed lettering, audio,
settings and broader platform/pixel-compositing coverage remain open.

Arkanoo glass-paddle follow-up: the flat body and incorrect ghost-bonus tint are
replaced with the original rounded metallic normal/extended textures. The
unchanged pinned procedure is rasterized by real SDL, retaining corner clipping,
line endpoints and alpha blending. Both source-derived PNGs match every decoded
reference byte. The unchanged original overlay function supplies independent
core, thruster and aura geometry/colors at normal, extended, full/decayed spin
and threshold states. Fast paddle contacts now trigger the original discharge;
its intensity decays by 0.9 per active frame with the original cutoff. Ghost-ball
bonuses leave the paddle appearance unchanged. Integer world geometry is retained
before half-scale projection. This restores paddle art and discharge only; bonus
icons, other artwork, fonts, recorded audio, menus/settings and full platform
coverage remain open.

Glass-paddle validation: 409 all-features tests pass with 61 dedicated checks
ignored by default. The signed harness explicitly passes all 30 checks in Wasm
and native builds, including both complete textures, original overlay commands,
spin thresholds, one-frame decay, a real fast-paddle collision and ghost-bonus
independence. Native GUI and both core/component browser gameplay/editor/file
workflows pass with movement checks against the restored cyan core. Reference
regeneration is byte-identical; strict lint and formatting pass. The source
recipe advances to 0.1.22 without a signed-package or app release.

Arkanoo normal-brick follow-up: all six original color textures now use the
unchanged pinned `draw_block_texture` procedure and `BLOCK_COLORS`. Real SDL
rasterization supplies independent 60×20 RGBA fixtures, including line clipping,
inner glow, gradient and beveled borders. Every decoded PNG byte matches, and
signed runtime probes cover all colors, linear filtering and ordinary,
fractional/negative integer rectangle positions. Gameplay and the editor's placed
bricks share the same textures; the original flat-color palette is retained.
The 106-layout regression checks the textured boards without dropping occupancy,
coordinate or uniqueness assertions. Editor color/archive checks compare complete
texture pixels. The gameplay test requires a real partially destroyed board with
a rendered ball, so a menu cannot count as broken bricks. Later sections cover
graphite stripes/bevels and ice refraction/glints/shine. Other art/audio/settings and full-frame fidelity remain unfinished; the
explosive fire/glass layers are covered below.

Normal-brick validation: all 410 all-features tests pass with 62 dedicated checks
ignored by default. The signed harness explicitly passes all 31 runtime checks
in Wasm and native builds, including exact pixels and all 106 layout cases.
Desktop plus core/component browser gameplay/editor/file workflows pass. Painting,
reload and imported-color checks compare nine samples from the original textures
with a three-level 8-bit renderer/capture rounding tolerance; runtime source pixels
are exact. Strict lint, formatting and byte-identical reference regeneration pass.
The source recipe advances to 0.1.23; no package or app release is created.

Graphite-brick follow-up: the original opaque body, rivets, diagonal stripes and
bevels are restored from the unchanged pinned branch. Two separate transparent
outside layers retain the original non-clipped x=-20..70/y=20 pixel coverage,
straight alpha and order; nearest sampling retains the primitive pixel grid.
Every decoded layer byte is checked, including the transparent interior masks.
Six real-SDL backdrop comparisons keep opaque pixels exact and record one/two
8-bit-level software line/copy rounding differences outside. This comparison does
not establish native/browser framebuffer equivalence; full frame alpha/color-space
semantics remain explicitly open, alongside other presentation work.

The full-board test exposed repeated decoding in the old one-entry asset cache.
Native and core/component Wasm now share a session-scoped LRU cache capped at
32 paths (each at most 1,024 bytes) and the existing 1 MiB decoded-pixel budget.
Frames still charge every draw against unchanged image/command/time limits.
Eviction preserves live immutable frames; replacement, byte/entry bounds and
cross-draw/frame identity have dedicated checks. The signed stress fixture renders
all 200 metal bricks on cold and warm frames without extending the game deadline.

Graphite/cache validation: 414 all-features tests pass, with 63 dedicated checks
ignored by default. All 32 signed checks explicitly pass in Wasm and native builds.
Both local app entries are rebuilt; native GUI and core/component browser
regressions pass. Windows 32/64 all-features compilation checks, strict lint,
formatting, reference regeneration and seven public-page link checks pass.
The maintainer harness compiles its test executable first and runs it directly;
compiler/lock waits no longer consume the runtime test allowance. Arkanoo's source
recipe is 0.1.24; no signed package or app release is created.

Game-color compositing follow-up: an actual SDL/default-OpenGL comparison exposed
a native color error of up to 66 8-bit levels. For example, translucent white with
alpha 30 over black appeared as RGB 96 instead of 30. The game renderer had used
egui's UI helper, which premultiplies color in linear light. Rectangles/circles
and ordinary sprites now use an encoded-RGB premultiplication path separate from
UI styling. Raw immutable source bytes and opaque colors remain unchanged.

`tests/reference/sdl_blending.c` renders six foreground colors over four opaque
backgrounds using actual SDL rectangles, nearest textures and ordered overlaps;
`tests/blending_gui.py` checks 72 RGB samples in each tested configuration. Core
and component Wasm and native-worker playback across Vulkan/OpenGL (six desktop
combinations) now differ from the live default SDL renderer by at most one level.
Both core/component browser profiles match exactly. Software SDL is recorded
separately, preserving the earlier software line/copy rounding observation.
This corrects solid source-over compositing; filtered interpolation, antialiased
edges, frame-reset/background semantics and complete original frames remain open.

Game-color validation: 417 all-features tests pass with 63 dedicated checks
ignored by default. Both local app entries are rebuilt. All 32 signed port tests
pass in native and Wasm, followed by actual native GUI and core/component browser
workflow regressions. Windows 32/64 all-features compilation checks, strict lint,
formatting and site-link checks pass. CI includes the desktop-only SDL comparison,
which also passes locally; the local full comparison includes both browsers.
No game dependency, permission, resource limit, package or app release changes.


### Opaque filtered sprite sampling

Native linear sprites now use host-owned encoded-RGB textures instead of the UI
renderer’s sRGB texture interpolation. The live SDL reference in
`tests/reference/sdl_filtering.c` flushes nearest controls before changing their
sampling mode. `tests/filtering_gui.py` compares 46 RGB samples across static,
moved and swapped-source frames in six desktop configurations and both browser
profiles (1,104 comparisons). It includes negative-position clipping and a
512-pixel alternating texture minified to 128 pixels. OpenGL matches exactly;
Vulkan and browsers differ by at most one 8-bit channel level. The forced Mesa
OpenGL 2.1 configuration also passes the static comparison; Mesa refused to
disable its VAO extension, so the no-VAO path is not verified by that run.

All 417 default all-features tests pass (63 tests remain separately invoked).
The signed-port harness passes all 32 checks in both native and Wasm, plus the
native GUI and core/component browser workflows. Windows 32/64 compilation,
strict lint, formatting and public-page links pass. The desktop-only comparison
added to CI also passes locally. No binary/package release is created.

The sampler retains existing image/command/frame limits and immutable source
pixels; callback resources remain alive until queued draws finish. The existing
source-over comparison now interleaves filtered sprites with shapes and nearest
sprites. Transparent SDL straight-alpha interpolation, rare sRGB-only surface
blending, edge rasterization and complete original-game frames remain open.

### Plain-shader fallback texture

Vulkan used an opaque white fallback while OpenGL and the documented API used
transparent black. The previous test only rejected the last blue/green texture,
which allowed white to pass. Before correction, the strengthened WGSL test
reproduced a white `(255, 255, 255)` sample instead of black.

Vulkan now supplies a transparent black 1×1 image. The private shader test makes
sampled RGB and alpha observable through opaque output, checks nine interior
locations after the plain draw, then verifies that a textured draw recovers.
All twelve configurations pass: GLSL/SPIR-V/WGSL across core Wasm, component
Wasm and native execution, plus wrapped API checks in OpenGL core, Vulkan
component and Vulkan native. Receipt tampering, reflection/entry/stage/resource
rejection and malformed translated caches remain checked in those runs. Seven
shader unit tests, strict lint, formatting and Python syntax checks also pass.
CI now includes native texture/fallback tests for all three backends. This does
not establish arbitrary shader alpha-compositing or complete game-frame parity.


### Ice glints, refraction, cracks and moving shine

Recipe 0.1.25 restores all drawing steps of the pinned ice branch: f32 pulse,
inclusive top/left glints, three position-seeded refraction segments, the five
health-dependent crack segments and ten clipped moving diagonals. The original
integer geometry is projected at half scale. A dependency-free i32 range sampler
matches pinned Rand 0.8.5 without changing the established u64 level sampler.
The editor now advances and uses its own animation clock, rather than a frozen
gameplay clock; leaving the editor pauses that clock.

The unchanged original branch is compiled by `tests/reference/arkanoo_ice.py`.
It verifies rendering/entities source hashes, uses pinned Rand dependencies,
and records 96 cases (twelve frames, four positions, two health states). The
reference includes shine wrap, f32 precision-boundary and u64-maximum frames.
Real SDL supplies independent alpha masks: the 61×21 overlay preserves inclusive
outside endpoints and the 60×20 shine stays clipped. Signed runtime checks compare
every mask alpha and exact pulse color; the separate editor-clock probe checks
that the editor draws its own fifth frame while gameplay has advanced once.
A complete damaged 200-ice board still fits unchanged 1,024-command/8 MiB image
and tick limits, using three commands per brick. Guest crates remain forbidden.

Grouping white lines into ordered overlay masks introduces software SDL rounding
differences of at most three 8-bit levels in the 576 original-versus-grouped
backdrop comparisons. This is a measured SDL reference bound, not a claim of
native/browser framebuffer equivalence. Complete original-game frames and
the other tracked presentation/platform requirements remain unfinished.


Validation: all 63 port logic tests and all 33 signed checks in each of native
and Wasm pass. The real native editor and both core/component browser editors
paint and animate ice, erase/repaint the original normal brick, and finish the
existing gameplay, save/reload, import/cancel/export, cursor and fullscreen
workflows. Strict lint, formatting, Python syntax and byte-for-byte reference
regeneration pass. These checks use temporary libraries/displays; no player
library, private media or release packages are published.


### Explosive magma, moving core and glass layers

Recipe 0.1.26 restores the original f64 magma pulse, moving bright core,
top-half glossy fill, top/left glints, two stationary reflections and eight
clipped moving shine diagonals. It retains the separate original f32 shine phase
and inclusive 61×21 outside coverage. The shared white-mask helper keeps ice's
existing behavior, and editor painting uses its independent active clock.

The moving core illustrates why source tracing must preserve library semantics:
`BLOCK_HEIGHT - 20` appears to produce a zero-height rectangle, but the original
Rust SDL 0.37.0 constructor clamps it to one pixel. The reference therefore uses
the actual crate, not a simplified rectangle stub. Both SDL archives are verified
against the pinned original Cargo.lock checksums before compilation. This is a
maintainer-only build; no guest crate, host capability or player dependency is
added. Other oracles using constructor stubs still need dimension-edge auditing
before broader full-frame equivalence can be claimed.

The unchanged explosive branch supplies 64 cases across sixteen frame values and
four positions, including shine wrap, f32/f64 precision boundaries and u64 maximum.
Signed native/Wasm checks compare exact magma colors, real core coordinates and
one-pixel height, both complete alpha masks, draw order and nearest sampling.
Real SDL comparisons over six backdrops verify that removing the core changes
pixels in every case (384 comparisons). Grouping white overlays records at most
three 8-bit levels of software SDL rounding difference; that does not establish
native/browser framebuffer equivalence. Complete original-game frames remain open.

Full 200-fire and mixed ice/fire/metal boards are checked cold and warm at three
animation phases, within unchanged 1,024-command, 8 MiB image and tick limits.
Each explosive brick uses four commands, preserving its core as a separate opaque
rectangle before the glass layers. The expanded visual test fixture dispatches
through direct helpers to retain the existing source complexity policy.


Validation: all 63 port logic tests and all 34 signed checks in each of native
and Wasm pass. Native and both browser editors paint and animate fire/glass,
then restore the normal brick for existing save/reload and import/export tests.
The window checks also require visible core pixels that cannot be produced by
mixing only white reflections into the magma color; the native check samples
all physical pixels so the subpixel-height core cannot fall between logical
sample rows. Strict lint, formatting, Python syntax and exact reference
regeneration pass. No binary/package release is produced.


### Fresh frames and background parity

The browser player kept old pixels when a custom frame had no clear call. A
translucent clear repeatedly accumulated against the previous frame, and a
transparent clear preserved prior geometry. The native player already rebuilt
each frame against its black panel, with opaque RGB #10171d when no clear color
was supplied. The browser now follows that same rule: opaque backgrounds replace
the canvas directly, while alpha backgrounds composite over fresh opaque black.
Completely empty custom frames show the default background rather than retained
pixels or the legacy grid fallback. Every visible primitive belongs to its tick.

`tests/frame_reset_gui.py` builds and runs a signed input-controlled source probe.
Before correction, the real core browser retained red RGB 200/48/32 where the
next frame required dark RGB 16/23/29. The corrected host passes all eight
configurations: native and core Wasm across Vulkan/OpenGL, component Wasm across
both renderers, and both browser profiles. Each configuration checks eight phases
and delayed stability, including alpha-128 and fully transparent clear colors,
absent backgrounds, zero commands, disappearing geometry and phase revisits.
Browser pixels remain opaque. Samples are taken inside flat regions; this does
not establish edge rasterization, arbitrary shader blending or complete original
frames. The API still supplies complete frames, not an automatic retained target.

The browser code is embedded into exports at build time, so old exports need
regeneration to receive this correction. Player builds, source policy, signatures,
permissions and all command/image/time/memory limits are unchanged. The CI step
runs the actual desktop/browser regression; the local script discovers Chrome or
Chromium and the matching installed driver. Python/Selenium/display tools remain
maintainer-only dependencies.


Validation: the red/green browser reproduction and all eight rendering
configurations pass after correction, including delayed samples. The final
CI-compatible browser/driver discovery path passes both browser profiles.
Existing complete browser-export checks pass in core and component modes,
covering real tile gameplay, scoped resources/saves/ROMs, sprites, audio,
shaders, watchdog and corrupt-export rejection. All 34 signed port checks pass
in native and Wasm; its actual native and both browser gameplay/editor/file,
fullscreen and visible-effect workflows pass. Node smoothing/keyboard checks,
strict lint, formatting, Python syntax and seven public-page link checks pass.
No package or binary release is created.


### Original glass bonus capsules

Recipe 0.1.27 restores the original four glass capsules and their symbols in
place of colored rectangles and letter labels. The unchanged original function
supplies radial alpha/tint, top-left reflection, all 720 interleaved ring points,
shadow and repeated white symbol passes. In particular, the long-paddle symbol
retains the union of its two offset white bars; the ghost and rocket symbols
retain their actual rectangular drawing geometry. Their existing falling physics
and 40×40 original collision boxes are unchanged. Inclusive point endpoints use
41×41 textures at 20.5×20.5 logical size after original integer truncation.

The reference uses the unchanged rendering procedure and Bonus::rect, pinned
rendering/entities hashes and checksum-verified real Rust SDL constructors.
Four tints share highlight/ring layers; four symbols retain original shadow and
white overwrite order. Direct source RGBA is used for non-overlapping layers;
interleaved ring points are captured in a transparent OpenGL target and converted
back to straight RGBA. Ten immutable source-derived PNGs remain within the existing
32-entry/1 MiB decoder cache. Every draw still charges its full frame image budget.

All sixteen kind/position fixtures compare every decoded RGBA byte, nearest
sampling, layer order and exact projected geometry in signed native/Wasm runtimes.
An invalid-kind probe emits no commands. Cold/warm full 200-item scenes retain
800 bonus draws, per-asset Arc identity and unchanged 1,024-command/8 MiB/tick limits.
Ninety-six original-versus-layered backdrop comparisons have maximum channel delta
one under default SDL OpenGL. The independent software SDL path records delta five;
its texture-copy rounding differs from direct point drawing and is not hidden.

The live scene test invokes the production game renderer with all four items in
six desktop configurations and both browser profiles. It checks transparent
corners, tint, symbol, rim and highlight against original SDL pixels, then moves
the fixtures and verifies old positions clear. Exact nearest-neighbor boundaries
allow adjacent source texels because interpolated f32 UVs can land on either side
of a boundary. This is an explicit sampling limitation, not proof of exact scaled
framebuffers. Stable sample differences are at most two levels in the tested poses.
Complete original frames, original world-viewport clipping/aspect, fonts, background
and remaining presentation/platform requirements remain open.


Validation: all 63 port logic tests and all 35 signed checks in native and Wasm
pass. Existing native and both browser gameplay/editor/file workflows pass.
The eight-configuration bonus scene passes all 24 poses (480 RGB sample checks),
with explicit nearest-boundary handling and a maximum observed channel delta two.
The final CI-compatible browser discovery path passes both browser profiles.
Strict lint, formatting, Python syntax, tracker/CI parsing and byte-for-byte
reference regeneration pass. CI runs the actual bonus scene in desktop/browser
players. PNGs and notices are source-only; no signed package/binary release is made.

### Original rocket projectiles

The previous flat yellow projectile is replaced with the original orange body,
red nose built from eight ordered SDL lines, dark-orange fins and both flame
colors. A SHA-guarded maintainer oracle extracts the complete unchanged draw loop
from pinned rendering.rs and executes its primitives through real SDL software
rasterization. These thirteen opaque primitives fit a 10x20 source image, drawn
nearest at 5x10 after the original integer truncation. Transparent corners retain
the underlying game frame. The source-derived PNG has the original WTFPL notice;
players need no asset-generation tools or added dependencies.

Five origins include positive fractional, negative fractional and bottom-right
edge cases. Signed runtime tests compare complete RGBA data and exact positions.
A separate production Game.draw stress scene checks 200 projectiles across cold
and warm frames, shared immutable image identity and unchanged frame limits.
The real-player bonus scene harness also supports `--rocket`, checking corners,
nose, body, both fins and both flame colors, movement and cleared old positions.
Original viewport clipping remains unimplemented; normalized sprite pixels and
commands do not establish complete-window equivalence.

Viewport investigation: pinned main.rs independently scales x/y on window resize
using window dimensions divided by 1280/720. It does not uniformly letterbox all
resizes. Pinned Menu::new also puts Back at original y=760, below the 720-pixel
window; cropping the current projected canvas wholesale would hide that working
control. Full viewport restoration must reconcile these original behaviors,
HUD/editor layout and pointer transforms with explicit evidence rather than
assuming a fixed aspect-ratio crop is faithful.

Rocket validation: all five source-art tests and 63 port logic checks pass. All
37 signed runtime checks pass in both Wasm and native execution. Existing native
and both browser gameplay/editor/file workflows also pass. The eight-player
rocket matrix passes 24 poses and 672 RGB comparisons: desktop maximum channel
difference one; both browser profiles exact for these samples. The existing
bonus matrix also passes after the shared harness extension (maximum difference
two). Original sprite sampling at exact nearest boundaries explicitly permits
adjacent texels; full geometry-edge rasterization and original viewport clipping
remain open. Reference regeneration is byte-identical. Source-only recipe 0.1.28
does not change the app version or publish binary/package releases.

### Four original portal stages

The old 48-circle approximation is replaced with the original ten rings of 32
filled discs, shrinking rings with faster rotation, expanding radial flash with
white core, and final purple fade. The production Game.draw passes its actual
frame and completion timer into these helpers. The existing 270-frame state
transition, pause and resume checks retain their production paths.

The maintainer oracle compiles the complete unchanged pinned draw_portal, records
every ordered integer coordinate and RGBA value, then rasterizes those commands
through real SDL software rendering. Eighteen compressed fixtures cover the
0/30/31/149/150/151/180/181/269/270/271 boundaries, f32 phase precision beyond
2^24, and u64::MAX. Their traces match production exactly. Alpha-zero points are
preserved in the geometry trace; invisible layers produce no draw command.

GamePKG groups these ordered points through its existing effect-image compositor.
Original software SDL versus the complete resulting layer over the dark backdrop
has maximum channel delta four in these fixtures, including dense closing-ring
overlaps. The tests enforce that measured bound. This is a quantified rounding
limitation, not an exact original framebuffer claim. Layers fit one image below
512x512; no command, memory, image-byte, source-policy or deadline limit changes.
Players need no new dependencies or generation tools.

All 64 logic checks and all 38 signed native/Wasm checks pass. The latter compare
all layer pixels with real SDL and also run cold/warm complete Game.draw scenes.
The live scene checks six poses (swirl, closing, flash, fade, empty, swirl again)
in six desktop configurations and both browser profiles. Desktop sample channel
difference is at most three; browsers at most two. Exact nearest boundaries
permit adjacent texels explicitly. Browser samples use explicit pixel rounding,
matching desktop sample coordinates. Existing gameplay/editor/file workflows
also run separately. Original viewport clipping, whole-frame matching, remaining
background/menu/HUD/audio and platform requirements stay open. Recipe 0.1.29 is a
source-only change; the host version and published binary/package releases stay
unchanged.

### Campaign animated backgrounds and component input constants

Campaign levels 7–9 now render the original starfield, pulsing circles and digital
rain instead of the replacement grid. A SHA-guarded oracle compiles the unchanged
campaign branches and sends their ordered pixels to real SDL software rendering.
Its 21 cases include both animation frames, f32 precision loss beyond 2^24,
32-bit frame overflow and u64::MAX. Star x addition deliberately wraps, matching
the original release build; this avoids architecture-dependent overflow behavior.
Ordered geometry/colors match production exactly. Full framebuffer composition
retains a two-level software-SDL rounding bound in tests.

Sparse backgrounds use non-overlapping 64-pixel tiles through the existing image
API. Existing effects retain their 512-pixel tiling. The signed runtime test
reconstructs every original background pixel and checks tile overlap, dimensions
and clipped placement. It also keeps 200 fire bricks and 3,000 particles at all
four window corners visible across cold/warm scenes. All layers remain inside
the unchanged 1,024-command, 8 MiB image and time/memory limits; the background
must not consume the budget and silently omit later art.

The eight-configuration player scene checks both frames of all three animations
and returns to the starfield to detect retained pixels. Desktop samples differ
by at most two levels; both browser profiles by at most one. Samples are inside
world/tile geometry edges and exact nearest-boundary choices remain explicit.
Original edge rasterization, aspect and complete-window comparisons remain open.

The component source SDK was missing the raw non-directional button constants
already provided by core/native. All button/held aliases are now available there.
An actual signed component checks all twenty raw constants against host input
bits before its typed asset/ROM/save/drawing/audio and denial/rollback checks.
The live background fixture uses held B (Z) in every desktop/browser profile.
Existing library input mappings and permissions are unchanged. The CLI and GUI
were rebuilt locally; older hosts need rebuilding to expose these component
constants. This source-only change publishes no binary release.

Validation: 65 logic tests and 39 signed checks in both native and Wasm pass;
native and both browser gameplay/editor/file workflows pass. The complete
background player matrix, signed component API/constant checks, component/input
unit tests, strict lint, formatting and byte-identical reference regeneration
pass. Source recipe 0.1.30 retains host version 0.1.17. Original image backgrounds
1–6, editor artwork, twenty endless procedural themes, fonts/HUD/menu/audio,
viewport and remaining platform work are still unfinished.

### Procedural snow, smoke and tornado

Endless levels now select their original theme from the original level seed and
i32 random draw. The Snow, Smoke and Tornado paths are adapted with original
positions, sizes, phase calculations, alpha and draw order. Seventeen other
selected themes still use the previous grid fallback and remain explicitly open.
This change does not substitute those three themes for the original twenty.

Color sampling needed a separate Rand-compatible u8 sampler. Rand 0.8.5 uses a
near-exact modulus rejection zone for small integer types, unlike its wider i32
approximation. Using the i32 sampler for colors would change later random values
and background positions. Four seeded streams compare 1,024 typed u8 samples
with real Rand; no game dependencies or source-policy exceptions are added.

The oracle compiles the complete unchanged draw_animated_background against exact
Rand 0.8.5, rand_chacha 0.3.1 and rand_core 0.6.4. It finds actual level seeds for
each theme instead of forcing the selection. Thirty-three cases cover three
levels per theme, ordinary animation, f32 phase loss, 32-bit frame overflow and
u64::MAX. Production ordered-point counts and complete trace hashes match;
compressed real-SDL framebuffers independently verify all composed pixels.
Transparent layer grouping retains a measured four-color-level bound against
software SDL. It is not exact framebuffer equivalence.

Dense smoke uses 128-pixel tiles; sparse backgrounds retain 64-pixel tiles. Tiles
never overlap. Full scenes preserve all 200 fire-brick overlays and particles at
all four window corners with 3,000 active particles, cold and warm, under the
unchanged command/image-byte/time/memory limits. Source functions remain direct,
portable and dependency-free. The previous campaign background helper is renamed
to draw_background to cover both branches.

Validation: all 66 logic checks and all 40 signed checks pass native and Wasm.
The eight-player procedural matrix checks both animation frames for each restored
theme and a revisit: maximum sampled channel difference three on desktop and two
in browsers. Samples stay inside tile/world edges and nearest boundaries allow
adjacent texels explicitly. The existing campaign matrix is also checked after
extending its shared harness. Native/both-browser gameplay/editor/file workflows,
strict lint, formatting and byte-identical oracle regeneration also pass.
Whole-frame/edge comparisons, seventeen other themes, original image/editor
artwork, fonts/HUD/menus/audio and remaining platforms are unfinished. Recipe
0.1.31 changes source only; host version and binary/package releases are unchanged.

### Procedural fire and deep sea

Fire now follows the original seeded rising flames, four color/alpha bands,
sine wiggle and phase-dependent sizes. Deep Sea follows the original five light
shafts, ordered fading bands and 200 drifting plankton points. Its sine/cosine
values are cast to integers before multiplying by 50/20; that original behavior
is deliberately retained rather than smoothed into a different animation.

The complete original-function oracle now accepts an explicit theme list while
retaining byte-identical default snow/smoke/tornado output. Twenty-two new cases
cover three actual selected levels per theme, ordinary frames, f32 precision
loss, 32-bit overflow and u64::MAX. Every ordered point/color is verified by
count and trace hash; real SDL supplies full independent framebuffer data.
Unit and signed-runtime verifiers are shared with the previous themes and keep
the existing measured four-level grouping bound. Source code remains direct,
dependency-free and inside the same source/resource policy.

All 67 logic tests and all 41 signed checks pass native and Wasm. Cold/warm
complete scenes retain all 200 fire-brick overlays and all four corner particle
tiles with 3,000 active particles. The eight-player visual matrix checks both
animation frames/theme and a revisit, with desktop sample difference at most
three and browser at most two. Interior samples and adjacent nearest-boundary
choices remain explicit; these checks do not establish world-edge or complete
original-window equivalence. Existing native/both-browser gameplay/editor/file
workflows, lint, formatting and reference regeneration are checked separately.
Fifteen procedural themes, image/editor backgrounds, viewport, fonts/HUD/menus,
audio and remaining platforms are unfinished. Recipe 0.1.32 is source-only;
there is no binary/package release or host version change.

### Procedural supernova and space battle

Supernova restores the original alpha-blended core, five ordered shockwaves and
seeded colored debris. Space Wars restores the original random interval/offset
choices, firing test, red/green selection and 100x4 source laser rectangles. The
clock first converts to f32 and then to u64; the subsequent offset addition now
wraps explicitly like the original release binary. Changing that order would
change extreme-frame firing behavior.

A scalar half-probability method matches real Rand 0.8.5's next-u64 threshold.
Four seeded streams check 1,024 bool values; the existing u8/i32/f32/u64 samplers
remain unchanged. Twenty-two original-function/real-Rand/SDL cases cover three
actual selected levels per theme and frame precision extremes, including a real
empty firing interval. Complete ordered-point hashes/counts and all composed
framebuffer pixels pass the existing four-level software-SDL grouping bound.
Previous procedural reference files still regenerate byte-for-byte.

All 68 logic tests and all 42 signed checks pass native and Wasm. Full scenes
retain 200 fire-brick overlays and all corner particle tiles with 3,000 active
particles. The eight-player matrix checks both animation frames, the empty
interval and a revisit: maximum sampled difference two on desktop, one in
browsers; empty-interval samples are exact. Samples for the original four-pixel
laser bars use a one-pixel vertical margin, since a five-pixel inset has no
interior. Other tile/world margins and explicit nearest-boundary choices remain.
The empty interval also samples earlier laser locations to verify clearing.

Existing gameplay/editor/file workflows, strict lint, formatting and reference
regeneration also pass. Thirteen procedural themes, image/editor
artwork, viewport, fonts/HUD/menus/audio and remaining platforms remain open.
Recipe 0.1.33 changes source only; host version and binary/package releases are
unchanged.

### Star Voyage and the SDL default line baseline

Star Voyage restores the original mixed typed root RNG stream. Theme selection
consumes i32 sampling before u8 background colors, the u64 star count, f32 speed
multiplier and u8 tint. Per-star seeds then preserve original position, distance,
size and alpha. Using the wrong sampler or restarting the root stream after
selection would change the whole scene. Eleven unchanged-original/real-Rand/SDL
cases cover three actual selected levels and frame precision extremes.

The shared player harness now derives its guest selector and native/browser key
mapping from one table. Eight configurations pass two frames at each of three
level seeds plus a revisit: maximum sampled difference three on desktop and two
in browsers. The previous supernova/laser/empty-interval matrix passes again.
Interior and nearest-boundary sampling limits remain explicit.

The remaining angled-line themes require their own evidence. Inspection of the
vendored SDL software backend alone initially suggested antialiased line behavior;
actual SDL 2.30 tests showed the default common point rasterizer is selected.
A new 32-case baseline records default OpenGL and software images separately.
Thirty cases have identical coverage and at most two RGB levels of blend-rounding
difference; two zero-length cases paint one point on OpenGL and none in software.
The baseline includes slope ties, reversal, degenerate and fully rejected lines,
edge-only paths and viewport crossings. SDL version and images/hashes are retained.

An altered scalar Rust helper now reproduces default OpenGL coverage. Integer
endpoint clipping must happen before rasterization: clipping completed points
instead shifted edge pixels in the independent test. The helper includes the
full SDL zlib notice and needs no SDL dependency in player/game code. Its signed
native/Wasm probe checks every final reference coverage set, raw layer color,
bounds, empty output and one-image layout. It is not yet integrated into the
remaining line-based themes, and does not establish every SDL version, line mode,
scaled frame or platform. Those requirements remain open.

Validation includes 70 logic tests, the final 44 signed checks in both runtimes,
unchanged full-scene drawing limits, actual player matrices, original gameplay/
editor/file workflows, strict lint and reference regeneration. A runtime-only
maintainer option permits rechecking both signed execution profiles without
repeating GUI workflows; CI retains the full default workflow. Twelve procedural
themes, image/editor artwork, viewport, fonts/HUD/menus/audio and remaining
platforms are unfinished. Recipe 0.1.34 is source-only, with unchanged host version
and no binary/package release.

### First integrated line theme: Kaleidoscope

Kaleidoscope now renders the original five layers of twelve diamonds through the
attributed default-SDL point-line helper. Alternating rotation, layer colors,
240 line commands and inclusive endpoint/corner blending are preserved. Endpoints
cast after the original floating-point size offsets; subtracting an integer size
from an already truncated negative position would change clipped geometry.

The new maintainer recorder compiles the complete unchanged original procedure
with pinned real Rand. It records raw line commands and passes them to actual
SDL 2.30 OpenGL/software RenderDrawLine calls. It never calls the guest rasterizer.
Eleven frame/level cases compare every original command field, order and complete
command hash. Independent additive OpenGL passes also count pixel contributions
for each of the five layers; all 55 full per-layer count maps match production
exactly, including collapsed f32 phases and heavily repeated extreme-frame lines.

The first transparent-layer grouping had up to four levels of rounding difference
at u32::MAX/u64::MAX. Backgrounds have a known opaque starting color, so the final
kaleidoscope path precomposes each point over that color using the recorded default
OpenGL convention: source and destination products round separately before adding.
Painted pixels become opaque; untouched pixels remain transparent. Sparse tile
counts, image charges, source policy, time and memory limits stay unchanged.
All eleven complete logical framebuffers now match original default OpenGL
byte-for-byte in unit and signed native/Wasm checks. Original software SDL differs
by up to four levels from that default renderer; this backend distinction remains
explicit rather than defining both outputs as identical. Other background paths
retain their previous compositor and verification bounds.

All 71 logic tests and all 45 signed checks pass. Dense scenes retain all 200
fire-brick overlays and four corner particle tiles with 3,000 active particles,
cold and warm. Eight actual player configurations pass both animation frames at
all three selected level seeds and a revisit. With original default OpenGL as the
canonical reference, browser samples match exactly and desktop samples differ by
at most one level. The test enforces that tighter display-rounding bound. Interior
sampling and adjacent-texel choices at exact nearest boundaries remain explicit;
this does not establish complete scaled-window or other-platform equivalence.

Existing native/both-browser gameplay/editor/file workflows, strict lint,
formatting and byte-identical reference regeneration also pass. The recorder needs
pinned original source, Rand 0.8.5, SDL development files and private-display test
dependencies; players need none of those tools. Eleven procedural themes, original
image/editor artwork, viewport, fonts/HUD/menus/audio and remaining platforms are
unfinished. Recipe 0.1.35 is source-only; no host version or binary/package release
changes.

### Rain and Warp streaks

Rain and Warp now preserve their original seeded line commands. Rain retains its
400 candidate drops, original speed/length ranges, y phase and slant. Upstream
comments disable its lightning code; the active original behavior is preserved.
Warp retains its exponential distance, speed-dependent tint, variable alpha and
trailing endpoints. No sampling type, float operation order or line clipping is
replaced with an approximation.

The generalized line recorder compiles the complete unchanged original function
with real pinned Rand. Twenty-two cases cover three actual level seeds per theme
and frame extremes. All original command fields/order/hashes match. Actual default
SDL 2.30 OpenGL additive passes independently supply 116 ordered-segment-group
pixel-count maps, which also match production exactly. Groups contain at most
48 consecutive original commands; zero-alpha commands contribute no visible count.
The earlier kaleidoscope recorder remains byte-identically reproducible by default.

Both complete canonical logical framebuffers match original default OpenGL
byte-for-byte in unit, signed native and signed Wasm checks. Rain also matches
the previous two-level renderer difference; original software SDL differs from
OpenGL by up to 15 RGB levels on faint overlapping warp streaks. Tests retain both
original images and verify their measured difference explicitly. This is not a
larger tolerance on the port: the default OpenGL comparison still requires exact
bytes, and previous software-only background bounds remain unchanged.

Rain uses 128-pixel sparse tiles so dense coverage cannot consume the command
budget before later art. All full-scene fire-brick overlays and corner particle
tiles remain present with 3,000 active particles, cold and warm. Other sparse
backgrounds retain their existing tile sizes; no command, image-byte, time,
memory, source or portability policy limit changes.

All 72 logic tests and 46 signed checks pass in native and Wasm. Eight actual
player configurations pass both animation frames/theme and a revisit, with exact
browser samples and desktop rounding at most one level. Interior samples and
explicit nearest-boundary choices remain; complete scaled-window/platform parity
is not established by those samples. Existing native/both-browser gameplay/editor/
file workflows, strict lint, formatting and reference regeneration also pass. Nine procedural themes, image/editor artwork, viewport, fonts/HUD/
menus/audio and other platforms remain open. Recipe 0.1.36 is source-only; host
version and published binary/package releases are unchanged.


### Cyber Grid and Aurora Waves

These two original line backgrounds now preserve the complete shared typed RNG
stream after theme selection. Cyber Grid retains its randomized background,
line tint, horizon, spacing, perspective endpoints and moving horizontal lines.
Aurora retains its randomized wave count, orientation, per-wave colors/alpha,
speed/amplitude, 5-pixel strip spacing and original float/cast operation order.
The reference selector explicitly includes both aurora orientations; its first
three natural seeds otherwise only exercised horizontal strips.

The unchanged complete original function and real pinned Rand supply 22 cases
across three selected levels per theme and extreme frame phases. All raw ordered
line fields and hashes match. Real SDL 2.30 default OpenGL supplies 185 independent
48-command-group contribution maps and all complete logical framebuffers; every
map and canonical framebuffer byte matches production. Original software SDL
has identical coverage in these cases but differs from default OpenGL by up to
six RGB levels; those separate original images and measured differences remain
verified without relaxing the exact canonical comparison.

Both themes reuse the attributed clipped line rasterizer and known-background
compositor. Existing density-based sparse tiles preserve all 200 fire-brick
layers and all four full-window particle tiles with 3,000 particles in cold/warm
signed native/Wasm scenes. Command, byte, source, portability, time and memory
limits are unchanged. A local opacity binding was renamed to avoid conflicting
with an existing function under the conservative source scanner; its policy was
not changed.

All 73 logic tests and 47 signed checks per execution profile pass. Eight actual
player configurations verify animation, theme changes, both aurora orientations
and revisit: browser samples are exact, desktop samples differ by at most one
color level. These interior samples with explicit nearest-boundary choices do
not prove complete scaled-window or other-platform fidelity. Seven procedural
themes, image/editor artwork, viewport, fonts/HUD/menus/audio and the other port
requirements remain open. Recipe 0.1.37 is source-only; the host remains 0.1.17
and no binary/package release is created.


### Matrix Rain and Nebula Clouds

Matrix Rain now retains the original shared u64 stream count/f32 multiplier,
per-stream seeded position/speed/length/head offset and ordered 8×12 character
rectangles. Its head addition explicitly wraps as in the original release;
large saturated frame phases produce an empty background in the original and
remain empty in this port. Nebula retains its typed background/cloud-count
stream, per-cloud position/size/color, original trigonometric drift/casts and
nested rectangles in their original order.

A new rectangle recorder compiles the complete unchanged original function with
pinned real Rand. It sends the raw ordered rectangles directly to actual SDL
2.30 RenderFillRect calls, independently of the port's pixel expansion or blend
implementation. Twenty-two seed/frame cases cover three selected levels/theme,
extreme phases, overlap and viewport clipping. Every command field/hash, all
133 independent 48-command-group OpenGL contribution maps and all complete
canonical default OpenGL logical framebuffer bytes match. Original software SDL
has identical coverage in these cases but differs by up to 23 RGB levels after
repeated faint cloud blends; both original images and the measured backend
difference are verified separately. The canonical comparison remains exact.

An initial expanded per-pixel cloud list exceeded the existing 16 MiB package
memory limit. The final renderer composites the ordered original rectangles
directly into a bounded logical image, retaining every blend without that list.
Non-overlapping 128-pixel tiles preserve all 200 fire-brick overlays and all four
corner particle layers with 3,000 particles, cold and warm. Source/portability,
command, image-byte, time and memory limits are unchanged.

All 74 logic tests and 48 signed runtime checks per profile pass. Eight actual
player configurations verify two animated frames/theme, another seed, an empty
phase and revisit. Browser samples match exactly; desktop samples differ by at
most one color level. Interior sampling with explicit nearest-boundary choices
still does not establish complete scaled-window or other-platform fidelity.
Five procedural themes, image/editor art, viewport, fonts/HUD/menus/audio and
the other open port requirements remain unfinished. Recipe 0.1.38 is source-only;
the host version and published binary/package releases are unchanged.


### Hex Pulse and Bubbles

Hex Pulse now retains the original staggered rectangular grid (the upstream
implementation uses outlines, despite its theme name), shared typed random
background/size/pulse-speed/tint draws, per-cell phase and float/cast order.
Bubbles retains its 150 seeded positions/speeds/sizes/phases, sway, original
rectangular outlines and separately ordered shine points. Neither effect is
replaced with a different geometric interpretation.

The new mixed-primitive recorder compiles the complete unchanged original
function with pinned real Rand and sends the original point/line/outline/fill
commands to real SDL calls. For these two themes, 22 selected-level/frame cases
verify every ordered outline/point command and 306 independently measured
48-command-group contribution maps. The level selector deliberately includes
Hex Pulse's smallest original grid size (20, or 18-pixel outlines), yielding
2,508 original commands before viewport clipping. Nineteen separate real SDL
outline baselines cover corners, edge clipping, fully absent shapes, complete
viewport borders and one-pixel degenerate dimensions.

The scalar outline helper follows SDL's default four endpoint-exclusive strips,
including degenerate behavior, rather than drawing each corner twice. This is
an explicitly altered adaptation of RenderDrawLinesWithRectsF under the existing
SDL zlib notice. All outline contribution maps and every complete canonical
default OpenGL framebuffer byte match production. Original software frames have
the same visible coverage in the game cases and differ by at most two RGB levels;
those measured differences remain checked separately.

Direct ordered-strip composition avoids retaining an expanded outline pixel
list; 128-pixel non-overlapping tiles preserve the full grid, bubble shine and
all later art. The shared transparency helper also cancels its original formula
exactly when the destination alpha is zero, avoiding redundant divisions for
first pixel contributions. Timed runtime checks use the optimized production
host rather than charging debug-host processing overhead as game execution.
Deep Sea also reserves its original maximum of five 120×720 shafts plus 200
plankton points in one allocation. This avoids intermediate growing buffers; the
complete 106-level private native sequence passes with the same 16 MiB cap.
The default 100 ms deadline is unchanged. Dense native/Wasm scenes retain all 200
fire-brick overlays and four corner particle tiles with 3,000 active particles,
cold and warm. All source, portability, time, memory, command and image-byte
limits remain unchanged, including the 16 MiB package memory cap.

All 75 logic tests and 49 signed runtime checks per profile pass. Eight actual
players cover animation for both themes, the densest grid and revisit, with
exact browser samples and desktop rounding at most one color level. Reference
regeneration, strict lint and formatting pass. Interior sampling and explicit
nearest-boundary choices do not prove complete scaled-window/platform parity.
Three procedural themes (Starry Night, Digital Circuit and Maze), image/editor
art, viewport, fonts/HUD/menus/audio and other port requirements remain open.
Recipe 0.1.39 is source-only; the host version and binary/package releases are
unchanged.


### Starry Night, Digital Circuit and Maze

The final three previously missing procedural paths now preserve their original
mixed primitives and ordering. Starry Night retains 200 per-star seeded size and
twinkle draws, satellite motion and blinking, and interval-seeded shooting-star
probability, progress cutoff and trailing endpoints. Circuit retains every node,
two independent seeded connection decisions and the original moving packet
phases/order. Maze retains all 627 seeded diagonal commands and its final moving
6×6 solver rectangle.

The scalar Rand adaptation now also preserves Bernoulli's exact f64-derived
10%, 30% and 70% u64 thresholds, rather than approximating probabilities with a
small integer roll. Four independent real-Rand seeds per probability verify
3,072 successive decisions; complete scene commands also verify interleaving
with i32/f32 draws. The reference selector includes a naturally selected Starry
Night seed with an active first shooting interval. Cases around 29/30, 59/60,
199/200, 259/260 and 399/400 preserve blink and shooting cutoff/interval behavior,
including the original f32 timing and saturating casts at extreme phases.

The complete unchanged original function and actual SDL 2.30 provide 43 cases,
349 independent grouped contribution maps and complete canonical default OpenGL
frames. Every raw command field/order/hash, contribution map and canonical
framebuffer byte matches in logic and signed native/Wasm checks. Original
software SDL has identical visible coverage in these cases and differs by at
most two RGB levels; these separate original images remain explicitly checked.
The recorder's earlier Hex Pulse/Bubbles output remains byte-identical.

All 77 logic tests and 50 signed checks per run pass within unchanged source,
portability, time, memory, command and image-byte limits. Dense scenes retain
all 200 fire-brick overlays and all four corner layers with 3,000 particles,
cold and warm. Eight actual players cover two frames for each theme plus a
revisit, with exact browser samples and desktop rounding at most one level.
These interior samples with explicit nearest-boundary choices do not prove
complete scaled-window/edge or other-platform fidelity.

All twenty procedural draw paths are now adapted and have per-path reference
and runtime evidence. This is not a claim that every older background matches
canonical OpenGL frames exactly: eight earlier point-based themes and campaign
7–9 retain documented software-SDL grouping bounds. Canonical composition for
those paths remains an explicit tracker gate. Image/editor artwork, viewport,
fonts/HUD/menus/audio, other platforms and the broader port requirements remain
open. Recipe 0.1.40 is source-only; no host version or binary/package release
changes.


### Exact canonical frames for the eight earlier procedural backgrounds

Star Voyage, Snow, Smoke, Tornado, Fire, Supernova, Space Wars and Deep Sea now
compose against their original known background using the canonical default
OpenGL blend convention. This replaces their former grouped transparent image
approximation. Other effects still using transparent layers are separate work;
no renderer-wide tolerance or policy relaxation is introduced.

The extended recorder compiles the complete unchanged original function with
pinned real Rand and sends its point/fill commands directly to actual SDL 2.30.
Two captures provide 88 selected-level/frame cases and 452 independent grouped
primitive-contribution maps. Guest point generation matches every clipped
ordered original primitive pixel/color, and every contribution map matches the
independent SDL pass. Existing full unclipped point hashes/counts and u8/boolean
RNG vectors remain active; canonical coverage adds to that evidence.

Every canonical logical framebuffer byte matches in unit and signed native/Wasm
checks. Old signed background tests also resolve their same seed/frame to these
new original captures and now require exact OpenGL pixels; they no longer accept
a four-level grouping bound. Separate actual software SDL frames differ from the
original default OpenGL renderer by up to six RGB levels. Faint smoke can also
change the RGB-visible mask solely through those rounding differences. These are
measured original-renderer differences, not a larger allowed error in the port.

Smoke now emits its 100 original ordered rectangles directly to the compositor,
avoiding the large expanded pixel list during production rendering. The legacy
point generator derives from those same rectangle commands for independent hash
checks. This preserves the original timing, clipping, color, dimensions and
blend order while retaining the 16 MiB cap and default 100 ms deadline. All other
source, portability, command, image-byte and time limits remain unchanged.

All 79 logic tests and 52 signed checks per run pass, including full 200-fire-brick
and 3,000-particle corner layers in cold/warm scenes. Two eight-player matrices
cover both animated frames for all eight themes, empty Space Wars clearing and
revisit. Browser samples are exact and desktop rounding is at most one level;
older procedural player checks now use the same strict canonical bound. Interior
sampling and explicit nearest-boundary choices still do not establish complete
scaled-window, viewport/edge or other-platform fidelity.

All twenty procedural themes now have exact canonical logical-frame evidence.
Campaign backgrounds 7–9 still have their documented software-SDL grouped-image
bounds and remain the next canonical-composition gate. Image/editor artwork,
viewport, fonts/HUD/menus/audio and the other port requirements remain open.
Recipe 0.1.41 is source-only; host version and binary/package releases are
unchanged.


### Exact canonical campaign 7–9 composition

The animated starfield, pulsing circles and digital rain now use the same
known-background compositor as the procedural themes. Twenty-one captures run
the unchanged original function with real SDL 2.30 point/fill calls at every
existing reference phase, including f32 precision loss, i32 saturation/release
wrap and u64::MAX. All 392 independently measured grouped primitive contribution
maps, clipped ordered pixels and complete canonical default OpenGL framebuffer
bytes match production exactly.

Existing full unclipped point traces/hashes and release-overflow tests remain
active. The old signed campaign probe retains its original case selector and
stress bit, while delegating to the shared exact-frame verifier. Both old and
new probes enforce the original 64-pixel campaign tile bound and exact canonical
pixels; the previous two-level grouped-image approximation is removed. Actual
original software SDL frames differ from original OpenGL by at most two RGB
levels, measured separately rather than treated as port error tolerance.

All 80 logic tests and 53 signed checks per run pass under unchanged source,
portability, time, memory, command and image-byte limits. Cold/warm complete
scenes retain all 200 fire-brick overlays and all four corner layers with 3,000
particles. Eight players pass both animation frames for each campaign background
and revisit: browser samples exact, desktop rounding at most one level. Reference
regeneration, the unchanged default recorder output, strict lint and formatting
also pass. Interior sampling with explicit nearest-boundary choices does not
prove complete scaled-window, viewport/edge or other-platform fidelity.

All twenty procedural themes and all three animated campaign backgrounds now
have exact canonical logical-frame evidence. Original image backgrounds 1–6,
editor artwork, whole-window proportions/clipping, fonts/HUD/menus/audio and the
other port requirements remain open. The broader composition tracker keeps its
explicit whole-window clipping requirement open. Recipe 0.1.42 is source-only;
host version and binary/package releases are unchanged.

## Scoped drawing clip verification

The shared SDK now supports replacement/reset clips in core Wasm, component Wasm, native and browser rendering. `tests/clipping_game.py` passes six desktop configurations (three execution profiles with Vulkan/OpenGL) and both browser profiles across six frame transitions. The desktop matrix also resizes each client window to 960×720, 853×641 and 700×800, requiring the captured dimensions before comparing logical samples. All 924 sampled RGB pixels match the analytic opaque-color expectations exactly. Sprite color samples stay inside sprite boundaries to avoid ambiguous pixel-center coverage at fractional scales. Tests cover rectangles, circles, nearest/linear images, shaders and textured shaders, empty clips, replacement/reset, clear and next-frame isolation. Signed runtime probes additionally verify malformed bounds, command limits, cumulative image charges and failed-tick rollback. `tests/web_clipping.mjs` checks browser command validation and balanced canvas state. This verifies the shared primitive; port adoption and original whole-window viewport/aspect behavior remain open.

## Ball-speed lettering restoration

Gameplay now renders the previously missing speed labels with original Roboto
Medium glyphs, speed conversion, integer positioning and 0.6 texture scaling.
The maintainer oracle guards the pinned font and original rendering fragment;
eighteen complete SDL_ttf labels match the dependency-free source compositor
byte-for-byte. Signed probes verify complete label RGBA and destination geometry
for six speeds at four fractional/negative positions. The new raw filtered-image
API retains existing command, memory and image-transfer bounds across native,
core/component Wasm and browser players. All 81 logic checks and both rounds of
54 signed checks pass, plus native and both browser gameplay/editor/file flows.
Direct actual-player comparison now checks complete 521×17-pixel text strips
through four frame transitions in all six desktop and both browser profiles:
283,424 RGB pixels with original SDL OpenGL reference, desktop difference at
most one and browser difference at most two. Moving, changing speeds and revisiting
the original frame verify clearing. Browser Canvas2D minification initially
differed by up to 45 levels; explicit one-level WebGL bilinear sampling fixes it.
Clipping/reset/resizing regressions pass after the change. This closes the
Linux/browser speed-label gate, not other fonts, HUD or whole-window fidelity.

## Campaign image background restoration

The six licensed bundled images now replace gameplay's placeholder backgrounds.
The oracle checks original asset hashes, rendering/main sources and the original
linear texture hint. Six bounded tiles per image reconstruct all original
1280×720 logical RGBA bytes exactly; the overlay reproduces every byte of the
original alpha-64 gameplay capture. The 8 MiB frame-transfer and 1 MiB texture
cache limits remain unchanged. Dense-scene testing found that full bonus layers
plus the background exceeded that transfer cap. Cropping only zero-RGBA margins
preserves every original bonus byte, offset and all four original layers while
restoring all 800 dense-scene bonus draws. Small repeated sprites retain within-
frame cache identity; backgrounds may legitimately evict earlier LRU entries.
Both rounds of 55 signed checks and native/core/component browser gameplay,
editor and selected-file workflows pass. All six desktop and both browser configurations pass image changes and revisit:
4,928 samples cover tile seams and world edges, with at most one RGB level
difference from original canonical pixels. Independent original primitive
captures over the restored image also verify cropped bonuses (480 comparisons,
existing three-level bound) and rockets (672 comparisons, at most one level),
including stale-pixel clearing. Both 55-check signed runs pass the stricter
whole-frame bonus-transfer sum. Editor/menu artwork and whole-window scaling
remain open; these samples do not prove arbitrary-size presentation.

## Original editor backdrops and bounded decoding

The editor replaces its flat-color placeholders with the same six original
image tiles used by gameplay, followed by the original black alpha-153 overlay.
The independent actual SDL oracle verifies all complete 1280×720 RGBA hashes.
Both 56-check signed runs pass, including full 200-metal-brick boards, complete
outside layers and footer under unchanged transfer/command/memory/time limits.
Real Previous/Next controls, forward/backward wraparound and original brightness
pass in all six desktop and both browser configurations: 9,152 sampled RGB
pixels, maximum difference one. A source-input check preserves selection when
testing a board and returning; this is session state, as in the original.

A concurrent native particle test exposed repeated PNG decoding cost after the
backdrop restoration. The tiles now encode identical opaque RGB pixels with
stored, unfiltered PNG scanlines. The production decoder benchmark preserves
all RGBA bytes and measures roughly 2.23 ms versus 0.14 ms per 512×512 tile.
All 56-check runs and native/browser editor/file workflows pass afterward,
including a repeat of signed checks concurrently with the eight-player editor
matrix,
without increasing the native deadline or any quota. The larger encoded files
have a 40 MiB storage recommendation, verified against a clean native install.
Original grid blend mode/inclusive endpoints, editor/menu lettering and whole-
window scaling remain open; restored backdrops do not establish those features.

## Canonical editor grid correction

The original disables blending before its RGB (80,80,100) grid and includes
both SDL line endpoints. The port's translucent rectangles shortened each line.
It now draws 32 opaque one-texel images with the original 1201×1 and 1×201
source-pixel footprints. The independent actual SDL OpenGL reference verifies
every complete grid/backdrop RGBA byte for all six images. Both rounds of 57
signed runtime checks and native/core/component browser editor/file workflows
pass under unchanged memory, command, image and execution limits.

Actual grid checks pass in six desktop and both browser configurations, through
13 real editor selection states and three exact/fractional/tall desktop sizes.
All 7,320 sampled RGB pixels are within one level of original canonical values,
with exact-nearest-boundary choices documented. Browser Canvas2D fractional
coverage initially differed by up to 43 levels. Nearest one-texel images now use
pixel centers and the background sampler's lower-neighbor tie rule; adjacent
half-pixel strips do not overlap. This needs no extra WebGL capability. Existing
clipping, reset, image/shader and resize regressions also pass afterward.

This verifies canonical grid geometry, color and supported client samples.
Original palette/lettering composition, browser CSS scaling and full original
viewport behavior remain open. It does not establish complete editor parity.


## Canonical editor palette restoration

Recipe 0.1.47 restores the original nine swatch colors (including orange
explosive and dark metal choices), selected white three-outline border,
ordinary gray and hovered gray borders, and black Roboto Medium 26 digits.
The original placement and draw order are retained: backdrop, grid and placed
blocks precede the palette. Mouse hover uses original integer hitboxes and
selected borders take precedence. It remains ephemeral rather than saved.

A hash-guarded SDL_image/SDL_ttf/OpenGL oracle records the complete 404x34
original region for 28 selection/hover combinations. Signed production draws
reconstruct that complete region within one channel level in native and core
Wasm. All 58 signed checks pass in each execution mode. Pure input tests cover
all nine choices, gaps, upper/lower edges, click selection, non-motion hover
retention and non-finite input; digit alpha matches the existing original
font glyphs with black ink. Font regeneration remains byte-identical.

`tests/editor_palette_gui.py` checks every pixel in the original palette
region at a 1280x960 client size (original world pixels plus letterboxing),
including three selection/hover states and a revisit. All six Linux desktop
configurations pass: 329,664 RGB pixel comparisons, maximum difference one.
This intentionally verifies canonical pixel size. A first half-size comparison
against directly scaled original SDL rendering failed; thin primitive borders
and backdrop resampling do not generally commute with rendering to original
pixels first. Half-size/fractional window and browser whole-frame presentation
remain open, as do the Colors label and other original editor/menu lettering.
No graphics, execution, memory or drawing budget has been increased.


## Original editor static lettering and toolbar

Recipe 0.1.48 restores the original Roboto Medium 26 title, Colors label,
seven toolbar captions, six background-counter variants, drawing instruction
line and browser heading/instruction text. All 18 full RGBA rasters match
independent SDL_ttf captures byte for byte, and generation is reproducible.
The source/font/editor hashes guard that oracle. RLE data uses byte strings
rather than large Rust numeric expressions, retaining the source complexity
limit and dependency-free guest policy.

The toolbar restores original translucent background colors and alpha,
opaque borders, centered captions and hover hitboxes. Mouse motion updates
hover using original integer coordinates; non-motion events retain it.
Static labels use original positions and order. The 688-pixel instruction
line uses bounded 256-column pieces with one source pixel of overlap and
non-overlapping destination clips. Every piece fits the existing image
limits; each clip is reset before subsequent drawing.

All 85 logic tests and both 59-check signed suites pass, including all seven
hover choices, six background captions, complete glyph bytes, placement,
filtered overlap/clip reset, and full 200-metal editor boards within unchanged
limits. At original pixel size, all six Linux desktop configurations compare
the complete static-label and toolbar regions over three states and revisit:
1,760,088 RGB comparisons, maximum original SDL difference two. A separate
filtered-font test compares the entire old/new label region across three
labels, movement and revisit in six desktop and two browser players: 408,960
RGB comparisons, maximum difference one, including both long-line seams and
stale-pixel clearing.

Dynamic pattern names and messages still use replacement lettering, and the
browser panel/row appearance is unfinished. Full editor-frame and window/CSS
scaling parity remain open. These static-text checks do not close those gates.

The final complete run also passes native gameplay/editor, real file-picker
import/cancel and Save-dialog export, plus core and component browser
play/editor/import/cancel/explicit-download workflows. Strict all-feature,
all-target lint and formatting pass. Only source recipes/tests are published;
this does not create an application or package binary release.


## Original dynamic editor text and browser clip boundaries

Recipe 0.1.49 replaces the uppercase, question-mark and 78-character editor
name display with the original Roboto Medium 26 text path. Names keep their
case and Unicode contents, the full Name/finish/edit suffixes and original
colors/positions. Saved pattern rows retain original white lettering and
integer vertical centering. Messages use original green lettering and whole-
label centering. Long strings retain their full measured width while only
visible viewport slices are allocated/drawn; they are not shortened in storage.

The dependency-free guest embeds captured alpha for all 1,321 font glyphs,
explicit-endian metrics and the pinned font's substitutions and GPOS kerning.
Font-aware canonical decomposition/recomposition preserves the original missing-
glyph fallback rather than claiming additional character coverage. The oracle
captures 3,268 original SDL_ttf cases: 3,259 complete surfaces and nine empty
results. Cases include every non-NUL mapped scalar, random mixed names,
combining boundaries, kerning, long names, space/filler behavior and a further
1,205 compound joiner/variation-selector sequences. That compound audit caught
64 initial mismatches; cluster-sensitive normalization, contextual dotless
forms and distinct joiner/combining-joiner barriers now match them all.
All recorded full RGBA bytes match production, including negative bearings,
height expansion, overlapping alpha and partial raster slices. Actual original
HarfBuzz calls classify every one of the 4,174 default-ignorable candidates:
4,165 are hidden for this font/runtime; the other nine retain original behavior.
Special unmapped Unicode spaces retain captured original advances. Unicode data
license is bundled beside the existing font license. Python/C/FreeType/HarfBuzz
are maintainer reference tools; players need none of them for these glyphs.

A signed probe verifies complete original source bytes, destinations, viewport
clips/reset and colors for ordinary, editing, multilingual and 200-character
names, browser rows and messages in native and core Wasm. Both 60-check suites
pass, including existing 70,000-byte saved-name and dense editor regressions.
The 86-test logic suite passes, including full font-reference comparisons.

The eight-player Unicode text matrix covers whole old/new regions, centered
oversized messages, names past the previous cutoff, kerning, combining accents,
negative bearings, top/side clipping and revisit: 1,576,960 RGB comparisons.
Maximum difference is two on desktop and one in browsers. The first browser run
exposed a 44-level error: Canvas2D antialiased fractional clip edges, halving
some glyph pixels. Browser clips now round both endpoints to hard pixels, as
both existing desktop scissor backends do. A direct fractional/empty/replace/
reset/fresh-frame regression passes, as do all 924 existing shared clip/image/
shader/resize samples across six desktop and two browser configurations (exact
RGB in this run). Drawing/transfer/deadline/memory/source budgets stay unchanged.

This closes the recorded dynamic editor lettering gap, not complete editor
presentation. Original browser panel/rows/overlay, remaining menu/HUD art and
whole-window/CSS scaling still require their own reference and runtime gates.

Final verification after the compound fixes passes both 60-check signed suites,
native gameplay/editor/file-picker import/cancel/Save-dialog export and core/
component browser play/editor/import/cancel/explicit downloads. Strict lint and
formatting pass. Source recipes and tests are published without a new binary
release or host-version change.


## Original saved-pattern browser composition

Recipe 0.1.50 restores the browser's original black alpha-200 world overlay,
opaque (40,45,52) panel, graphite border, all unselected row fills and blue
selected row. Original Roboto headings, row names and footer retain their
source placement/order. Empty lists have the original empty panel instead of
invented placeholder lettering. Existing paging beyond ten patterns is retained;
canonical comparisons cover the original first ten visible rows.

A font/render/editor hash-guarded actual SDL OpenGL oracle records twelve
complete 1280x720 RGBA frames: empty, one entry and all ten selected-row states.
Signed native/core-Wasm command composition matches the entire frames within
two levels. Original odd-width centering uses window-left plus half the remaining
width, which differs by one pixel from subtracting half the text width. The
reference follows the original integer arithmetic. A full 200-metal board plus
ten mixed/long-name rows retains every isolated browser command under unchanged
command, image, time and memory limits. Both 61-check signed suites pass.
The denied-save regression now checks an empty actual row list and no persisted
save, replacing its old dependency on removed placeholder text.

Six desktop configurations compare complete original-size world frames over
empty/first/last selection and revisit: 22,118,400 RGB comparisons, maximum
original difference one. Both web profiles compare complete 640x360 world frames
with a separately captured 640x480 native player: 1,843,200 comparisons within
two levels. The latter proves backend consistency at that size, not direct
scaled-original SDL equivalence. Total: 23,961,600 complete-frame RGB comparisons.
The twelve-frame original fixture regenerates byte-identically.

Whole-frame comparisons exposed a desktop/browser mismatch of 68 levels at
half-pixel panel borders. Native one-texel nearest images previously relied on
backend-sensitive quad-edge ties, and thin strips could disappear. Their native
destination bounds now use the browser's deterministic pixel-center rule after
viewport translation and physical DPI scaling. This does not affect filtered
sprites or image/command charging. Tests cover negative and adjacent half strips,
translated coordinates and integer/fractional DPI. The complete frame matrix
passes afterward. Shared clip/image/shader/resize checks remain exact (924 RGB
samples), and the eight-player editor grid matrix plus three desktop sizes stays
within one level across all 7,320 samples. Strict lint and formatting pass.

Full editor/menu composition, arbitrary original viewport/CSS scaling and native
platforms beyond the tested Linux configurations remain separate unfinished work.

Final native gameplay/editor/system import/cancel/Save-dialog export and core/
component browser play/editor/import/cancel/explicit-download workflows pass
with the panel and native coverage changes. Source-only publication leaves
application/package binary releases and host version unchanged.

### Original implemented menu control layer (recipe 0.1.51)

The implemented main-menu actions and SFX/fullscreen/back controls now use the
original Roboto lettering, captions, integer centering, hover/normal fills,
borders and title positions. The menu uses the original background-one image
and alpha-150 dimming. Six independently reproduced, overlapping filtered tiles
preserve its canonical SDL pixels while staying within the existing image-size
and per-frame budgets. These tiles derive from the upstream WTFPL background;
no font compiler or image tools are needed by users. The accessible Back button
in the port's extra vertical space remains an extension. Removed the invented
gravity indicator; tests instead verify the original caption and actual saved
mode. Accepted fullscreen toggles update the caption, and restart preserves that
state; external window-manager changes are not yet read back into the menu.

A guarded original-source SDL oracle captures 38 complete canonical frames for
the implemented control layer, covering New Game/Resume, every main hover,
settings volume boundaries, mute and fullscreen captions. Missing original
controls are deliberately excluded from this partial-layer oracle. The fixture
and all six artwork tiles reproduce byte-identically. Both signed suites pass
62 checks with full-frame reference differences at most two levels. Six desktop
renderer/runtime configurations match complete canonical world frames within
two levels; both browser profiles match separately captured smaller native
frames within the same unchanged tolerance. This smaller-size comparison proves
backend consistency, not arbitrary scaled-original equivalence.

Browser filtered images now blend with the current framebuffer before sample
quantization, matching the desktop operation. Compositing a quantized transparent
intermediate had accumulated a three-level menu text error. The complete menu
matrix passes after the correction, and shared clipping/image/shader/resize
checks retain exact output across all eight players (924 RGB comparisons).

At that checkpoint, music controls/playback, resolution and VSync controls, the
safe project-link and version display, splash/HUD presentation, arbitrary original viewport
scaling, fullscreen state readback and untested native platforms remain open.

Measured native-plus-Wasm library storage is 43.0 MiB including the signed
package, recovery copy and current builds. The advisory storage estimate is
therefore 48 MiB; execution, memory, image, command and source caps are unchanged.

Final native gameplay/editor/system import/cancel/Save-dialog export and both
core/component browser gameplay/editor/import/cancel/explicit-download workflows
pass. The restart workflow now checks independently captured original backdrop
pixels instead of the removed flat placeholder color. Strict all-feature/all-
target lint and formatting pass. Publication is source only; the host remains
0.1.17 and no package/application binary releases are created.

### Original startup artwork and fade (recipe 0.1.52)

The placeholder startup lettering is replaced by upstream `antigravity.webp`
(the original game's development-tool acknowledgment image), rendered over
white with the original 60-frame fade-in, hold through frame 210 and fade-out
through frame 270. The captured opaque artwork uses six overlapping filtered
PNG tiles within the existing image limits. A white source-over fade preserves
the original texture-alpha composition without adding a guest graphics API.
Skip, expiry and restart continue to use the original timer lifetime; the
startup frame does not advance gameplay or capture the cursor.

A guarded SDL/OpenGL oracle independently captures 17 full canonical frames and
all 271 original float-to-byte alpha values. Both signed execution modes verify
those values and the full frames, including timer boundaries. The fixture and
six artwork tiles reproduce byte-identically. All six desktop runtime/renderer
configurations match the captured canonical RGB frames exactly; both browser
profiles agree with a separately captured half-size native player within the
unchanged two-level tolerance. Half-size checks establish backend consistency,
not arbitrary original viewport equivalence. The original artwork's upstream
credits remain part of the game, with provenance in the port credits.

Original HUD and end-state screens, recorded sounds/music, missing settings and
project/version controls, external fullscreen readback and other platform/scale
requirements remain open. No execution, memory or drawing limits are raised.

Final native gameplay/editor/system file-picker import/cancel/Save-dialog export
and both browser-profile gameplay/editor/import/cancel/explicit-download flows
pass with the restored startup screen. Both signed suites pass 63 checks; strict
lint and formatting pass. Measured installed storage with native/Wasm builds
and the recovery copy is 48.3 MiB, so the advisory estimate is 56 MiB. No runtime
budget changes or binary releases are included.

### Original end-state overlay composition (recipe 0.1.53)

Level transition, game-over and campaign-victory overlays now use their original
full-world black dimming, Roboto text, colors, integer centering and vertical
positions. Level completion shows the actual level and score plus the click
prompt. Game-over chooses red GAME OVER or green VICTORY! from all blocks'
activity, independently of campaign victory. Campaign victory restores its gold
congratulations, green subtitle, final score and both original instructions.
The replacement opaque panel and invented end-screen tips are removed. Portable
explicit decimal formatting preserves zero and the complete u32 score/level
range without a guest dependency or formatting macro.

An independent SHA-guarded SDL/TTF oracle captures 28 complete overlay frames on
a fixed two-color contrast backdrop, including zero, one, ten-digit scores and
u32::MAX scores/levels. The fixture regenerates byte-identically. Signed runtime
checks compare each complete canonical RGBA frame within the existing two-level
tolerance. Full game-renderer probes require the complete isolated overlay
command suffix for every end-state variant; populated probes use 200 metal
bricks and the level-transition probe uses the animated ninth background. All
suffixes must survive under the unchanged 1,024-command and 8 MiB image limits.

Six desktop renderer/runtime configurations match complete canonical overlay
frames within one level; both web profiles match separately captured half-size
native frames within two levels. These are end-screen-layer comparisons over a
fixed backdrop, not proof of whole-game HUD or arbitrary scaled-original parity.
The original R/Q/Enter/click state transitions remain covered by existing logic
and signed tests. Original HUD/FPS presentation, recorded audio/music, remaining
settings/project/version controls, external fullscreen readback, arbitrary
viewport scaling and untested platforms remain open.

The broader parallel suite exposed execution-deadline failures in existing
3,000-particle background stress scenes. Shard rotation now computes sine and
cosine once per particle rather than repeatedly per diamond point; fully
transparent tiles are checked before allocating/copying a temporary pixel
buffer. Visible tile dimensions, bytes, ordering and command charging remain
unchanged. All 87 logic checks, including original shard geometry, pass. Both
64-check signed suites then pass at their original default parallelism and
unchanged memory, command, image and execution limits. Strict lint and formatting
pass. No timing tolerance or runtime cap was increased.

Final native gameplay/editor/system import/cancel/Save-dialog export and both
browser-profile gameplay/editor/import/cancel/explicit-download workflows pass
with the end-screen and particle changes. Publication is source only; application
version 0.1.17 and binary/package releases remain unchanged.


### Original HUD, real frame timing and bounded full scenes (recipe 0.1.54)

The placeholder header/tips are replaced by the original world-scoped score,
heart PNGs and stolen-heart location, centered level/TEST caption, maximum-speed
record pulse, gravity indicator and bottom-right FPS counter. Decimal formatting
retains the complete u32 range. New speed records remember the original frame;
level changes retain that record age. FPS counts actual frame timestamps in the
original sliding one-second window, including across run resets.

The host supplies relative monotonic microseconds sampled once per tick, stable
within that tick and scoped independently to each game/restart. Core/component
Wasm, native IPC and browser bindings agree. This exposes no calendar clock,
ambient OS access or extra source dependencies. Repeated clock reads still obey
the execution deadline.

Original SDL heart textures interpolate straight RGB/alpha before applying
alpha. The added `draw_asset_image_linear_straight` binding preserves that
operation; the existing premultiplied sampler retains its behavior. Both modes
share all path, decode, immutable-cache, command and full decoded-byte draw
limits. Browser exports retain bounded nonopaque PNG RGBA, including hidden RGB
at zero alpha, and charge that metadata against the existing export limit.
Filtered coverage snaps physical pixel edges to measured SDL conventions while
preserving the original UV mapping. Nine independently captured SDL edge cases
and translated fractional-DPI tests cover coverage and empty rectangles.

A SHA-guarded original SDL/TTF oracle records 29 complete contrast-backdrop HUD
frames: zero/max-u32 numbers, TEST, gravity, zero/six/stolen hearts, fresh/expired
speed records and wide/exceptional numeric inputs. Isolated canonical RGBA
comparisons retain the existing two-level tolerance. Six desktop combinations
match original canonical HUD RGB within one level; two browser profiles match
separately captured smaller native frames within two. A same-asset cache marker
alternates straight/premultiplied filtering and revisits the initial state.
These fixed-layer checks do not prove arbitrary original viewport scaling.

Dense full scenes exposed missing heart PNGs under the unchanged 8 MiB image
budget. Fire-brick base/blob/glass/shine pixels now compose once per frame at
original 61x21 resolution. Translucent inclusive endpoints remain intact. Bonus
icons compose once from the unchanged original SDL primitives into 41x41 source
textures. Full-image references over six backgrounds preserve the existing
three-level tolerance; all earlier commands, reference pixels, hashes and
thresholds are retained. Sparse particle tiles use 128-pixel boundaries without
removing any visible particles. Full-renderer checks require the complete HUD
suffix through 200 fire bricks plus 3,000 corner particles and 200 bonus drops.
No command, memory, image or execution limit is increased.

Recorded audio/music, remaining settings/project/version controls, external
fullscreen readback, arbitrary original window/CSS scaling and untested
platforms remain open. Publication is source only; application version 0.1.17
and binary/package releases remain unchanged.

Final verification passes all 90 logic checks and both 66-check signed suites
at their unchanged default parallelism. Native gameplay/editor, real system
file-picker import/cancellation/Save export, and both browser-profile gameplay,
editor, selected-file import/cancellation/explicit-download workflows pass.
All 924 shared six-desktop/two-browser clipping, image, shader and resize samples
remain exact; original end-screen comparisons also pass all eight profiles.
HUD, filtered-edge, fire and bonus reference regeneration is byte-identical. Strict lint,
formatting, generated clock SDK checks and all 940 hostile browser paths pass.
An earlier run hit the existing Wasm deadline while separate GPU captures ran;
the final full run passes unchanged limits and test parallelism.


### Original cannon and launch caption (recipe 0.1.55)

The original ammo-gated cannon barrel and highlight now draw immediately after
the glass paddle and before balls, bonuses and rockets. Original integer paddle
coordinates, integer half-width centering, RGB colors and source-world rectangle
sizes are retained. The original yellow `press space to launch` Roboto caption
appears on even 30-frame intervals; zero ammo draws neither cannon nor caption.
Drawing restores the world clip after the self-clipped text before later objects.

The independent reference executes the unchanged cannon branch extracted from
the SHA-guarded upstream rendering source, using pinned original SDL 0.37 and
actual SDL_ttf/Roboto rendering. Sixty states cover zero/max-u32 ammo, normal/
long/odd-width paddles, left/right clipping, blink boundaries and max-u64 frame
counts. Each state is captured at original 1280x720 and scaled 640x360. Canonical
signed-runtime comparisons retain the two-level tolerance; dense renderer probes
require every cannon/caption command and all three original heart PNGs with 200
fire bricks and 3,000 corner particles under unchanged limits.

Actual smaller-size checks exposed a shared rectangle problem: egui rounded
solid rectangle edges, while Canvas antialiased them. Both differed from original
SDL at sample-center ties. Game rectangles now use the same measured SDL edge
coverage as filtered quads, with physical viewport/DPI handling on desktop.
Only coverage is snapped; color, alpha, ordering, clipping, command and memory
charges are retained. App UI shapes are unchanged. Nine independent actual SDL
solid-rectangle cases cover scaled, negative, thin and empty rectangles, with
additional translated fractional-DPI assertions. This is a rectangle fix and
cannon-layer gate, not proof of arbitrary whole-game viewport fidelity.

Final checks pass both 67-test signed suites at default parallelism and unchanged
limits, plus native and both browser gameplay/editor and real file import/cancel/
export workflows with the rebuilt host. Six desktop runtime/renderer combinations
compare directly with original frames at both sizes: canonical Vulkan max one,
OpenGL exact; half-size Vulkan max two and OpenGL max one. Both browser profiles
compare directly with original half-size frames within one level. These are four
selected states plus the initial-state revisit on each graphical path; all sixty
states are checked by signed canonical runtime comparisons.

All 924 shared clipping/image/shader/resize samples remain exact. Blending
comparisons, original HUD, implemented menu controls, pattern-browser and end-state
frames pass all existing desktop/browser profiles within unchanged thresholds.
The 120 original cannon captures and nine solid-edge cases regenerate
byte-identically; translated fractional-DPI tests, strict lint and formatting pass.
Publication remains source only, application version 0.1.17 unchanged.


### Actual fullscreen observation and caption accuracy (recipe 0.1.56)

The port no longer treats an accepted fullscreen request as a completed OS
change. Before drawing, it reads the host's latest own-window observation and
updates the original Fullscreen/Windowed caption only from confirmed state.
Unknown observation retains the last confirmed value. Startup restoration,
external X11 WM changes and browser fullscreen exits now reach the same source
path, including after new-game/restart transitions.

The bounded `window_fullscreen()` SDK returns optional boolean state under signed
input permission in native/core/component and browser runtimes. The underlying
fixed-width wire value distinguishes unavailable, windowed and fullscreen.
Observations are stable during the tick, scoped per game/session, independent of
outgoing command commit/rollback and available without fresh gesture. Denied or
closed games return unavailable. Request gesture checks and every execution,
memory, command and network/file restriction remain unchanged.

Actual X11 testing found that winit's fullscreen getter and setter can use stale
cached state after an external WM change. The trusted UI now queries only its
owned window's `_NET_WM_STATE` property on the existing display connection, bounded
to 64 native-width atoms. Missing state means windowed; wrong types/formats,
oversized properties and errors mean unavailable. Fullscreen commands also repeat
the same idempotent own-window EWMH request, covering winit's cached no-op path.
No handles, arbitrary window IDs or new crates become available to guest source.
Other desktop backends keep their platform window report; Wayland's winit
implementation reads its last compositor configure. Windows/Wayland runtime
verification remains separate from the verified X11/browser paths.

Real production source fixtures verify the actual menu boolean alongside the SDK
value while external WM changes and F11 change the window, then close/reopen in
saved fullscreen. Native Rust, core Wasm and component Wasm all pass. Both browser
profiles follow real canvas fullscreen, external API exit, native Escape on a
private browser desktop and Restart; denied variants stay unavailable even in
real fullscreen. Raw core/component and generated SDK checks cover unknown vs
false, repeated reads, independent games, permissions and unchanged deadlines.
Private X11 tests also cover empty/64/65 atoms, incorrect types/formats, deleted
properties and a separate hidden window. The original signed caption check now
requires original Windowed glyphs after mere acceptance, original Fullscreen
glyphs after observation, external exit updates and last-confirmed preservation.

The full parallel run also exposed execution deadlines in dense rendering scenes.
Particle composition now allocates only occupied 128-pixel tiles, preserving the
original full-image grid anchor, row-major command order, every RGBA byte and
ordered overlap blending. The four-corner/3,000-particle case stores 212,992 bytes
instead of constructing and scanning a 3,686,400-byte full-screen temporary first;
all visible image dimensions and transfer charges remain identical. Background
RGBA initialization repeats the same encoded bytes with bounded block copies
instead of appending one pixel at a time. Hidden RGB and alpha remain exact.

All 91 logic checks pass, including byte-exact reconstruction of sparse tiles for
empty, mixed/clipped/overlapping and 3,000-particle fields. Both 67-test signed
suites pass at their original default parallelism and unchanged caps/deadlines,
followed by native and both browser gameplay/editor and import/cancel/export
workflows. Real saved-window tests pass resize/fullscreen/maximized combinations,
F11, reopen, normal-size preservation, game/OS close, library isolation and damaged
record fallback. Their obsolete cyan placeholder-title readiness check now uses
the original independently captured ARKANOO glyph mask and real splash-skip input;
it retains the existing timeout and rendering-rounding bounds.

Window-specific raw/WIT/SDK/Node checks, actual native and headed-browser exits,
bounded X11 tests, strict lint and formatting pass. Windows 32/64-bit GNU GUI
compile checks pass; this is not Windows runtime verification. Source publication
keeps application version 0.1.17 and binary/package releases unchanged. Other
native platforms, recorded audio/music, remaining settings/project/version
controls, arbitrary whole-game scaling and broader port work remain open.

The native controller also defers an accepted pending toggle while window mode
is unavailable, retaining its parity until a confirmed observation arrives.
It no longer interprets unavailable as windowed. Six service/GUI controller tests
include unknown-to-known transitions, both toggle directions and preservation
of a pending request. Final native/core/component SDK and real-window tests,
headed browser API/native-Escape exits and saved-window regressions pass with
this controller; strict lint, formatting and both Windows compile checks pass.
Test configurations disable both game and app startup update checks.

Portable source audio now has `audio_submit_stereo`, accepting interleaved
48 kHz signed 16-bit left/right values. Mono and stereo share the existing
1,600-value per-tick budget; stereo does not increase it. One nonempty format
is allowed per tick, malformed or denied calls leave staging unchanged, and
failed ticks clear both samples and channel metadata. Output remains device-only.

`tests/stereo_audio.rs` verifies core/component memory errors, odd lengths,
shared budget, format conflicts, permission denial, per-game isolation, rollback
and execution deadlines. `tests/stereo_audio_game.py` verifies generated bindings
in signed native/core/component games and actual Chromium core/component exports:
WebAudio buffers contain the exact independent left/right samples at 48 kHz.
`tests/stereo_audio_web.cjs` checks browser memory/budget/permission rejection,
channel deinterleaving and duration measured in frames. Existing mono runtime
tests and strict GUI lint pass. This is transport support, not restoration of
Arkanoo's original compressed recordings or its music controls; those remain open.

The original audio-trigger audit found that normal/final ice-brick destruction
requests Bounce, a first ice crack requests BreakingGlass, rockets request
BreakingGlass and explosive-brick blasts request Explosion. Losing the final
ball requests Oh. The port now preserves those categories and stops inventing
sounds for launch, bonus collection, level completion and portal activation.
The original main loop deliberately retains the last callback in each physics
update; collision requests keep that behavior.

The pinned unchanged original brick/rocket fragments now record their last
callback alongside all existing physics results: ten cases and thirteen frames,
including ghost order, repeated ice hits, metal and blast boundaries. Regenerating
these fixtures reproduces them byte-for-byte; all earlier physics fields remain
identical. Signed fixture comparisons exercise the production port methods.
Original recordings, their provenance, sustained playback/mixing and music remain
unfinished. The synthesized effects are still explicitly temporary.


Final recipe 0.1.57 checks: 93 logic tests and 68 signed tests per native/Wasm
execution profile pass, including original callback categories and real-brick
volume/mute/persistence tests. Earlier launch-beep tests now wait for a real
visible brick break. No execution cap, test parallelism or pixel tolerance changed.
Deadline failures during earlier full runs prompted two exact rendering changes:
transparent glyph RLE runs are skipped and row coordinates are shared; opaque
particle contributions use the simplified original blend equation. All original
SDL text fixtures remain exact. A separate old/new raster comparison passes
122,870 cases with clipping and hidden RGB; alpha/color equation checks and
strict lint/formatting pass. Window/GUI/platform workflows were not repeated in
this round. Recordings, music, simultaneous playback and complete event/update
audio ordering remain open.


Recipe 0.1.58 introduces four independent stereo effect channels, retaining clip
positions across tick outputs and preserving active clips through a Game reset.
The first free channel is reused; a fifth active request is rejected. Mixer gain,
sequential clipping and channel order match actual SDL_MixAudioFormat output.
Mute prevents new voices and advances existing voices silently. Input callbacks
are captured independently before the final physics callback; keyboard and
controller rocket launches can sound alongside a same-tick brick collision.

Production uses the temporary synthesized clips. Longer stereo fixture clips
exercise multi-tick tails; original recordings, their duration/provenance,
compressed playback, music and full asynchronous callback timing remain open.
All 96 logic checks, four source-attack checks and both 69-check signed suites
pass. Expanded six-case mixer/input checks pass with explicit native and Wasm
selection. Actual core/component Chromium WebAudio buffers contain the expected
channel samples for tails, free-slot reuse, silent advancement and both input
paths overlapping a collision. Limits, test parallelism and visual tolerances
are unchanged. Strict lint/formatting pass; this round does not establish new
native device, graphical window or platform coverage.

Source-policy rejection messages now identify the offending packaged Rust file.
The existing adversarial signed and packing checks verify this diagnostic while
retaining every rejection. No source-policy restriction was relaxed.


Recipe 0.1.59 restores the original life-loss and explosion recordings, credited
to 11325622 and StereogenicStudio under the Pixabay Content License. Their source
pages, pinned upstream MP3 hashes, prepared WAV/PCM hashes and conversion reference
are recorded in tests/fixtures/arkanoo-recorded-effects.json and the port credits.
Maintainer-only SDL_mixer decoding uses the original 44.1 kHz mixer format, then
SDL converts to 48 kHz stereo PCM. Players require no conversion or MP3 tools.
The two cached clips total 1,656,844 decoded bytes and voices share immutable PCM.
No runtime cap was increased. Three effects remain temporary tones; original
music, compressed playback and full asynchronous timing remain open.

The canonical WAV loader validates format and lengths, bounds each clip to 2 MiB,
uses checked-size 64 KiB asset reads and explicit little-endian samples, and rejects
truncated/trailing data without adding a partial voice or reverting to a tone.
Failed loads do not repeatedly reread. All 99 logic checks and both 71-check
signed native/Wasm suites pass, including every sample of both clips through
completion and four simultaneous voices sharing both cached clips. The SDL
conversion reproduces the committed fixture byte-for-byte; strict lint/fmt pass.

Actual core/component WebAudio buffers match both complete original PCM hashes,
including channel separation, exact frame counts and zero-padded endings. The
longer explosion exposed drift from the old 16 ms browser delay, which could fill
the unchanged 100 ms queue and drop audio. Fractional 60 Hz fixed-step scheduling
now retains its phase and avoids catch-up bursts after lateness. A quantized
600-tick clock check, existing input/window/frame-clock/stereo checks and core/
component browser gameplay/integrity/shutdown regressions pass. No timing cap,
visual tolerance or test parallelism changed. Physical device, native graphical
window and broader platform workflows were not repeated in this round.


Recipe 0.1.60 restores all five original recorded effects. The remaining ball,
glass and launch MP3 files are byte-pinned to the upstream snapshot and its
README credit identifying sounds/music as Pixabay content. Their generic file
history supplies no individual item/artist links; metadata uses null artist
fields and records that exact credit basis. Those identifications remain open
rather than assigning an invented creator. The original two item-page credits
remain intact. All conversion hashes reproduce through the SDL reference.

The tone generator and old render::sound path are removed. The five prepared
clips retain original initial silence, complete duration and channel data;
SDL duplicates the mono glass source into both output channels. Total cached
PCM is 2,208,852 bytes. Five immutable entries share a 4 MiB resident budget,
retaining the earlier two-clip worst-case bound and the existing 2 MiB per-clip
bound. Four voices share PCM. Exhaustion rejects before allocating the next
clip; invalid IDs reject before indexing/reads. No runtime cap is increased.

All 100 logic checks and both 71-check signed native/Wasm suites pass at the
existing limits, visual tolerances and default parallelism. Complete sample
comparisons load all five into one session and preserve their endings. Gain
and onset/large-frame checks now inspect an audible fourth buffer, avoiding a
vacuous assertion on initial silence. Input/physics overlap compares complete
combined clip output; signed keyboard/controller fixtures and actual core/
component browser buffers check an audible overlap window. Both browser
profiles also match all five full PCM hashes. Strict lint/fmt and the existing
fractional-clock check pass. Music, fully asynchronous timing, individual item/
artist identification for three files and broader native platform/device/window
verification remain open.

The host-only music decoder prototype in tools/audio-decoder streams all nine
original MP3 tracks through the same import-free scalar Wasm companion in native
Wasmtime and Chromium. Game source still has no Cargo dependencies or direct
decoder API. The companion declares a 16 MiB memory maximum, accepts at most
8 MiB encoded input, bounds encoded reads to 512 KiB per operation and returns
at most 1,600 sample values per read. Native execution adds epoch deadlines;
instruction fuel remains absent. These prototype bounds have not yet been
integrated into combined game memory accounting.

Complete native decoding is invariant across different output chunk sizes and
rewind; malformed inputs, hostile companion results and an infinite decoder
loop have bounded failure checks. Chromium matches full-track fingerprints and
exact audible sample windows. All nine SDL decoded lengths match. Some integer
samples differ by one unit, including after nearest-even final conversion, so
bit-exact SDL decoding remains unproven. The independent streamed Music API
oracle verifies the existing effect-loading reference at source and original
44.1 kHz mixer rates, with final silent callback padding recorded separately.
This foundation does not yet enable music playback: conversion to the output
rate, asynchronous mixing, playlist/menu behavior and platform/device tests
remain required. No music recordings or compiled decoder binaries are added.

The decoder now also supports requested 44.1/48 kHz stereo output through a
shared streaming Rust adaptation of SDL 2.30's bandlimited filter. A fixed
2,048-frame ring retains edge history/lookahead; each append/render is bounded
to 800 frames. Global integer phase preserves all fractional time across calls.
SDL's original coefficient values and license notice remain intact. Independent
actual SDL float conversion matches every sample bit across 18 input/output-rate
pairs, including stereo separation and edge impulses. Variable-size reads,
short/empty streams, exact duration, invalid formats, nonfinite input, bounded
backpressure and unmodified rejected buffers have tests. The same converter is
compiled into the import-free companion; rewind retains its requested rate.

This is not a claim of matching SDL_AudioStream scheduling. Full-track testing
found its chunk phase truncation changes duration: one 44.1-to-48 kHz track has
17,753,782 sample values through SDL_mixer versus 17,763,784 with global phase.
The converted corpus oracle therefore explicitly uses native-rate SDL decode
followed by whole-buffer SDL conversion. Original chunked-playback behavior
remains a separate open fidelity item. New decoder rounding differences remain
observable and can become two integer units after filtering; no tolerance is
relaxed or bit-exact claim made. Game API, asynchronous playback/mixing, combined
memory accounting, playlist controls and platform/device verification remain open.

Original mixer block conversion is now available separately from global-phase
conversion. A trace of the actual original Music API establishes S16 stereo
4,096-byte/1,024-frame input puts. The compatibility mode reproduces SDL 2.30's
phase restarts, retained padding, short-put staging and flush behavior, including
its omission of a final padding flush when nothing is staged. Independent actual
SDL_AudioStream comparisons match every float sample bit and output length in
306 cases across nine rates, both output rates and 17 boundary-sensitive lengths.
The same 2,048-frame ring uses explicit backpressure; no cap is raised. Consumer
read sizes do not affect original block boundaries. Stereo source is required;
the nine current original recordings meet that requirement. Mono sources are
rejected rather than using incorrect original byte/frame boundaries.

Complete corpus checks now use the actual original chunked SDL reference when
original-mixer mode is selected, retaining the strict length assertion. All nine
tracks match the original 44.1 kHz mixer's full output lengths and retain chunk
invariance and rewind; native and Chromium streams agree. Original decoder PCM
differences of one/two integer units remain and are reported without tolerance
relaxation or a bit-exact decoding claim. This resolves the isolated original
block-conversion mismatch, not asynchronous game playback. Public game APIs,
playback/mixing, playlist controls, combined memory accounting and platform/device
verification remain open.

The reference capture itself now pauses SDL_mixer's actual device with
Mix_PauseAudio during setup/shutdown, fixing a leading-silence race caused by
pausing SDL's unrelated legacy device. All 13 original source/mixer-rate cases
pass exact PCM comparison after this correction. A bounded metadata-only trace
checks the actual source/destination formats and first 16 input put sizes in
every case. The versioned reference metadata and oracle hashes are refreshed.

Music input now has an exact-length reader path into companion memory, with
64 KiB read requests and no complete duplicate encoded buffer. Verified package
assets supply borrowed readers without extraction, retaining path/size checks,
stored/Zstd support, resolved shared-snapshot bounds, restart after cancellation
and a final ZIP CRC check for complete reads. A post-consumer CRC failure destroys
the returned resource. Tests cover corrupt stored data, unverified/invalid/
missing paths, oversize requests, changed shared-snapshot sizes, consumer errors
and repeated readers.

Native per-stream ceilings and optional page-aligned build maxima support smaller
budgets. All nine original-mixer tracks complete native/browser checks under
8 MiB and 6 MiB limits; the largest observed native linear allocation is 5,373,952
bytes. Output lengths and stream comparisons retain their previous assertions;
malformed/over-budget inputs fail without poisoning the factory. The corpus
loads files directly and checks limits through rewind. Shared reservations
reject concurrent overcommit and release ownership on drop. Budgeted decoding
charges the full ceiling before reading and holds it until after store destruction;
tests combine 10 MiB guest and 6 MiB decoder reservations under a 16 MiB total
and reject an extra stream before loading.

These host-only mechanisms are not yet connected to the game runtime. It must
reserve guest/decoder/host-buffer memory together, account for decompressor
workspace, enforce signed audio permission, and connect asynchronous playback
and SDK controls. Decoder sample differences and broader platform/device proof
remain open. No existing game cap, recipe version or application version changes.

The package-reader corpus option now builds a temporary fixture using a fixed
synthetic signing seed, verifies its embedded signature, and loads all nine
tracks directly through the verified archive reader at a 6 MiB decoder ceiling.
An existing asset is denied before verification. Full decode/rewind, original
mixer lengths and native/browser comparisons pass; no music files, user/release
signing keys or compiled helpers are published. Browser tests still fetch encoded ArrayBuffers,
so streaming browser input and its additional buffer accounting remain open.

The native host now has a background music service and output-clock handoff.
Verified package clones share the original open descriptor and private snapshot
ownership while retaining independent logical cursors. Seek/read operations are
atomic with respect to other readers and bounded to 64 KiB. Concurrent stored/
Zstd reads, cache-path replacement and parent/last-reader snapshot lifetimes have
regressions. No archive path is reopened or asset extracted for the worker.

The service has one pending latest-request slot, 6 MiB decoder accounting in the
corpus and a separate 64 KiB staging reservation. Its six-block atomic handoff
retains a 4,800-stereo-frame maximum. An observed try-lock defect could substitute
silence for queued PCM while a control lock was held; the positive regression
first failed and now requires the actual frame during that contention. Safe
atomics, generation tags and sticky unpark notifications remove that dependency
and prevent stale/future track corruption or lost capacity wakeups. Concurrent
stereo-block tests cover wraparound, partial blocks, bounds and exhausted counters.
Pause, gain, silent advancement, replacement bursts, decode faults, stop and
ownership cleanup have checks. Decoder deadlines now use monotonic elapsed time
and interrupt on the first expired epoch wakeup without instruction fuel.

All nine full tracks pass signed-package output-clock playback while another
thread reads drawing assets, with zero game ticks. Complete lengths match the
original 44.1 kHz mixer then 48 kHz output conversion; input and full PCM hashes
are pinned in the regression fixture. Remaining one/two-unit decoder differences
are measured rather than treated as bit-exact SDL parity. Actual CPAL callbacks
through a private ALSA null device advance at zero master gain, produce nonzero
output when unmuted, and pause/resume without main-thread ticks. Physical speakers,
native graphical gameplay and other platforms were not verified here.

This host backend does not yet expose music to the ports. Runtime/SDK permission
and guest-memory wiring, helper bundling, browser playback, original playlist/
settings controls and decompressor workspace accounting remain required. Compressed
embedded music fails explicitly until its workspace is charged. The generic PCM
callback also cannot reproduce original music-first per-effect saturation from
already clipped guest effect PCM; original per-voice mixing and fully independent
effect timing remain open. No existing cap, game recipe or host version is raised.

A host-only four-voice mixer now accepts immutable, budget-reserved stereo clips,
allocates the first free slot, and mixes music followed by each effect with
per-channel S16 saturation. It consumes no per-frame allocations and retains one
recording charge across shared voices until the last owner drops. Pause freezes
positions; zero volume advances them. Slot exhaustion, reuse, invalid clips,
volume rounding, clipping order and reservation cleanup have regressions.
An independent SDL 2.30.0 `SDL_MixAudioFormat` oracle covers every S16 source,
all 129 legal volume levels and seven destinations spanning clipping boundaries:
59,179,008 samples match exact pinned hashes. The existing single-layer callback
uses the same arithmetic. This establishes the mixing primitive, not end-to-end
port integration: the host mixer is not game-visible or connected to effect
commands yet. Original 44.1 kHz combined mixing must precede shared output-rate
conversion; independently converted music/effects and tick-driven premixed PCM
cannot establish that parity. Browser, command timing and device proof remain open.

The native host now connects original-rate music, the four effect voices and a
single shared 44.1-to-48 kHz conversion to the device callback. A distinct music
constructor exposes raw original-mixer PCM with an explicit rate; the old 48 kHz
attachment rejects it. Combined playback rejects legacy premixed submissions
instead of silently consuming them. Host play/volume/pause/stop controls operate
on voices before conversion. The converter and adapter reserve their resident
storage from the same supplied budget, and shared recording leases remain live
until their final voice/owner is dropped. Existing 48 kHz music output remains
covered by the complete nine-track regression.

An independent SDL oracle now mixes four different-length, different-stereo
recordings after music, then performs the whole-stream output conversion. Full
lengths and exact PCM hashes match at six effect volumes across three producer/
consumer chunk schedules, including single-frame chunks. A real private ALSA
null-device test verifies original-rate music and effects through CPAL with zero
game ticks, wrong-rate rejection, rejected premixed submissions, paused/muted
voices, effects after music stops and complete reservation cleanup. Full library
verification passes 172 tests with 17 explicitly ignored environment-dependent
checks; the selected ignored music/callback checks run separately.

This remains host-only integration. Signed SDK/runtime commands, original-rate
port recordings, helper bundling, browser playback and actual game migration
are still required. Commands currently affect source frames not yet buffered by
the converter; exact original block/event timing and stop/pause tails need
independent verification. Device-rate conversion beyond the 48 kHz intermediate, physical speakers
and Windows/Android gameplay remain unverified. No cap or version is increased.


The device callback now exclusively owns playback state. Host producers write to
one fixed atomic PCM ring with the unchanged 4,800-frame/100 ms capacity and
1,600-frame packet maximum; no second PCM copy or unbounded channel is added.
Source/effect controls use a separate 32-entry queue. The callback attempts a
brief command handoff, releases that lock before applying commands, and keeps
rendering current state and queued PCM whenever producers hold the lock. A
regression requires exact stereo PCM plus music under a held control lock. The
actual CPAL/ALSA null callback consumes all 1,600 submitted frames before that
lock is released; the reusable music check runs this gate explicitly.

Clear commands capture a producer boundary so later PCM survives. A requested
source detach preserves future PCM until the callback applies it, instead of
consuming it beneath the old combined source. Bounded queue rejection, ordered
commands, full-voice rejection reporting, producer closure, counter exhaustion,
concurrent stereo packet integrity and ring wrap have regressions. Host effect
methods now acknowledge enqueueing; later voice exhaustion is reported through
Player::error rather than silently replacing a channel. Source changes apply at
callback boundaries and remain subject to original event-timing verification.

Shared budget reservations now use atomic compare/exchange and lease releases
use one atomic subtraction, removing the reservation mutex from last-clip drops
on the callback. Full library verification passes 178 tests with 17 explicitly
ignored environment checks; selected null-device and all nine full-track tests
pass separately. This removes host-control/ledger-lock contention from normal
rendering, not allocator or OS scheduling latency. Signed SDK integration,
browser playback, exact event timing, platform/device proof and the broader port
fidelity requirements remain unfinished. No package/binary release is made.

Preparing the game-facing music boundary exposed an existing rejected-tick output
gap. Core/component ticks cleared PCM on a save-write error but retained drawing;
a play-history error retained both. The native wrapper also retained PCM/drawing
after history failure, and native save failure retained drawing. These paths now
reset the transient frame and PCM format/data before returning the failure, while
existing window/file-request rejection remains in place.

A regression first reproduced retained PCM after a history write failure. It now
proves a successful tick emits stereo PCM and drawing, then forces independent
save/history directory failures and requires both outputs to be empty, for core
and component Wasm. `tools/check_tick_output_rollback.py` builds a temporary
signed no-dependency native fixture with a fixed synthetic key, verifies actual
native execution, and checks the same save/history rejection behavior. It reads
no user library or signing key and publishes no package/artifact. This establishes
transient-output cleanup for these ordinary-game paths. Atomicity across already
committed saves/history, engine background audio, signed music commands and the
remaining port/runtime requirements are still separate unfinished work.
The full library suite passes 179 tests with 18 explicit environment-dependent
ignores; the selected signed native fault-injection test runs separately. Nineteen
component, progress, window and file-request integration checks pass as well.
The Wasm save-failure fixtures use the real guest save-write import with an
allowed save permission, rather than pre-marking host save state dirty.

The host can now embed its import-free audio decoder at app build time. A bounded
root build-script input copies core Wasm into the executable; optional builds
without it explicitly report unavailable, and removing an input clears the prior
asset. `Factory::bundled` verifies the 6 MiB declaration and required function
signatures in addition to normal import/memory validation. Hidden packaging
checks in both entry points keep the asset linked and run before any window,
settings or library access. Relocated CLI/GUI binaries validate identical bytes
with missing runtime decoder paths and no display.

All nine signed-package full-track hashes and the actual PCM/original-rate null
callbacks pass using embedded bytes with both decoder-path variables removed.
Build-input size/header/missing/reset checks and companion dependency-source/
license metadata checks pass. Packaging source accounting now includes the
companion crate and its separate Wasm dependency graph. Hosted native/32-bit/
Android recipes build this asset before the app; those edits do not constitute
local Windows/Android gameplay proof, and no release is created here.

The decoder remains a trusted host implementation detail. It adds no guest
crate allowance or ambient file/network permission. Game-facing signed commands,
automatic per-game service/budget setup, browser export embedding and current-port
migration remain required; no game/music completeness claim follows from this
bundling work.
The optional/unbundled library suite passes 181 tests with 18 explicit ignores;
three focused decoder tests pass with embedding enabled. Strict default/GUI
checks, optimized CLI/GUI builds, relocated-entry-point verification and updated
workflow YAML/input/metadata checks pass locally. No other-platform runtime
result or game-facing music activation is implied by those checks.

Hosted verification follow-up found two concrete failures outside the local
Linux evidence. The Linux Rust matrix linked the direct SDL audio oracle before
installing its development library; SDL is now installed before both test
configurations. The test remains enabled. Native Windows and macOS library jobs
failed the existing updater invariant for an already existing empty destination.
The generic rename fallback could replace that directory.

The updater now uses native exclusive publication on Linux, Windows and Darwin,
with no check-then-overwrite fallback. Windows retains Unicode/extended path
support without following the destination leaf. Tests require preservation of
existing files, empty/nonempty directories, dangling Unix links and invalid paths;
a concurrent directory-creation race must have exactly one winner. Four Linux
cases pass. Both Windows GNU harnesses cross-compile, and all four Windows cases
(including long paths) pass under an isolated 64-bit Wine prefix. An initial
compatibility failure on long-path conversion was corrected without relaxing the
path test. Real Windows/macOS evidence is now available from the dedicated hosted run below.

A small independent workflow tests the actual publication module on Linux,
Windows and macOS, plus 32-bit Windows, without the full desktop build dependency.
The existing full workflows remain enabled. Prior source revision e79ac11 has a
successful hosted 32-bit workflow for Linux, Windows and Android selected runtime/
portability suites; this proves those named checks, not every game's visual,
audio or feature fidelity. No release, updater download or user installation is
performed during these tests.
The local full library suite remains green (181 passed, 18 explicit ignores),
with strict GUI/all-target checks, independent publication-harness checks and
optimized CLI/GUI linking. The rebuilt entry points still validate the identical
embedded decoder after relocation. Native hosted results are recorded below.


[Exclusive app publication run 39186503336](https://github.com/centrexity/GamePKG/actions/runs/39186503336)
completed successfully at source revision `0710239`. Direct job logs confirm four
actual native tests passed on Linux, macOS, Windows64 and Windows32: 16 checks in
total, with no ignored or filtered tests. Windows logs include long-path cases
on both architectures; Unix logs include dangling-link preservation. All hosts
check the destination-creation race and file/empty/nonempty-directory invariants.
The native strict-check steps also pass. This verifies the named publication
invariant; the full application workflows and broader port/game fidelity remain
separate in-progress requirements.

The shared memory ledger now supports child capacities charged inside the parent
limit. Child leases anchor the parent reservation, so closing an effect cache
cannot make its capacity reusable while a voice still retains a recording.
Nested child shutdown, concurrent admissions, denial/cleanup and actual clip
completion have regressions. The original-rate real callback uses an effect
child pool, drops its cache object while the clip is retained, and still proves
playback and complete cleanup alongside the nine embedded-decoder track checks.
No memory cap, queue size, deadline or tolerance is raised.

Three public budget integration cases pass on native 64-bit and actual ELF32
Linux: nested ownership, full `u64` counter width and cross-thread final release.
The local 32-bit setup needed official compiler runtime files extracted into a
private temporary directory and native execution permission; no system package
installation was performed. Windows32/Android32 qualification now builds/runs
these same cases; new hosted results are still pending. Child capacity support
is groundwork for per-game guest/store carving, not evidence that the game-facing
music commands or automatic runtime budgets are already integrated.
Full Rust runs were also being superseded by subsequent source/documentation
pushes before they completed. Their concurrency policy now preserves an active
run and queues the newest revision; both feature-matrix jobs can finish even if
one reports a failure. Job/test parallelism, deadlines, limits and enabled checks
remain unchanged. Cancelled snapshots are not recorded as passing verification.
For this change, the full local library suite passes 184 tests with 18 explicit
ignores. Strict GUI/all-target checks and optimized CLI/GUI builds pass; the
rebuilt executables still contain the same validated decoder. Native Windows32/
Android32 ownership results and full workflow completion remain outstanding.


## Rust compiler compatibility verification (2026-10-18)

The full hosted Rust check exposed new Clippy diagnostics in Rust 1.99. Fixed-size
PCM/pixel iteration now uses typed array chunks, preserving incomplete-tail
behavior; stereo mixing still applies the same sample ordering and arithmetic.
Port changes use equivalent integer-divisibility checks and a guarded editor
match arm. The minimum supported Rust remains 1.89. No limits, tolerances,
versions, manifests, assets or catalog packages change.

Strict GUI/all-target checks pass on both Rust 1.89 and 1.99; the default-feature
all-target check also passes on 1.99. Arkanoo's 100 logic tests and both 71-check
signed native/Wasm suites pass. The 2048 logic suite passes nine cases with one
explicit upstream-oracle ignore, and all five signed native/Wasm state checks
pass. Host GUI library regressions pass 236 cases with 19 explicit ignores;
decoder regressions pass seven with two explicit ignores, and resampler
regressions pass four with two explicit ignores.

The separately requested real-audio checks cover all nine full original tracks
using the embedded decoder, actual null-device callbacks, effect-child ownership
and complete reservation cleanup. Both independent SDL comparison tests pass:
full floating-point conversion and original mixer staging/flush. These retain
existing assertions, including the previously documented decoder differences;
they do not establish bit-exact MP3 decoding or game-facing music integration.
The networking checks also pass 15 local-socket cases plus two manifest cases;
the public HTTPS case remains explicitly ignored in this local run.

This source compatibility update does not close the remaining port fidelity,
guest music API, per-game memory carving or platform/gameplay requirements.
Optimized CLI and GUI builds pass; both relocated executables validate the same
embedded decoder with runtime decoder paths unavailable and no display.
Hosted full-workflow results for the new revision must be evaluated separately.


## Original-rate verified effect assets (2026-10-18)

The host's original-rate mixer now has a canonical 44.1 kHz stereo S16 WAV
asset loader. Audio permission and a verified scoped package reader are required.
It reserves the final PCM/ownership charge and fixed 4,140-byte input staging
inside the existing caller budget before allocation, converts little-endian
bytes directly into the final sample storage, and publishes no clip before the
ZIP checksum succeeds. The unchanged 2 MiB per-clip limit includes an exact-limit
regression; a one-frame excess is rejected before reading. Insufficient PCM
capacity fails after only the header, with every reservation released. Malformed
headers, truncation and post-verification payload corruption are tested. Embedded
compressed effects fail explicitly until decompressor workspace is accounted for.

A private independent SDL conversion of all five pinned original MP3s produced
48384, 39791, 38616, 234253 and 146304 exact stereo frames. All five load from a
synthetically signed package inside the unchanged 4 MiB effect pool. Full decoded
playback matches the independent SDL bytes; four live clips keep the parent pool
charged after its owner drops, and stopping them releases it. Neither generated
WAVs nor test packages are published. The current port assets remain 48 kHz;
this verifies the new host path without prematurely migrating the port.

Signed synthetic cases cover permissions, package verification, invalid/missing
paths, final checksum rejection/cleanup and explicit compressed-asset denial.
The test recipe inputs are now embedded in the test executable, so they do not
require the build machine's source tree on Android. Linux32, Windows32 and
Android32 workflows include these cases; hosted results remain pending.
This is a loader prerequisite, not guest music/effect integration: transactional
commands, per-game guest/store memory carving, browser support, original-rate
port migration and event/stop timing remain open.

Local verification: all eight focused voice/loader tests pass, including the
exact size boundary and existing exhaustive SDL arithmetic corpus. The GUI
library regression run passes 238 cases with 19 explicit ignores; the additional
size-boundary case passes separately. All three synthetic signed-asset tests pass
on native 64-bit and actual ELF32 Linux. The original-recording case is an
explicit ordinary-suite ignore and passes separately with the private SDL inputs.
Strict GUI/all-target checks pass on both Rust 1.89 and 1.99.
Optimized CLI and GUI linking also passes with the decoder embedded.


## Whole-tick effect command publication (2026-10-18)

The host audio producer now accepts an ordered batch of effect commands under
one lock. This is needed before guest staging can publish a successful tick
without partially enqueueing it. The existing 32-command bound and preallocated
queue remain unchanged. Play/volume/pause/stop helpers share this path. Invalid
trailing values, queue exhaustion, inactive sources or closed consumers reject
the whole nonempty batch without retaining clip owners. Empty batches are open-
device no-ops. Voice exhaustion remains an asynchronous callback error.

Focused tests check an original-source attachment plus exactly 31 commands,
rejection at capacity, invalid Stop/volume rollback, clip lifetime, ordered gain/
pause/play application and eight concurrent four-command producers with no
interleaving. The real combined callback check also stages startup/resume as
batches, rejects an invalid Stop batch and proves the retained paused effect
subsequently becomes audible. Guest ABI exposure, tick/save/history staging,
per-game memory carving and original event/stop timing are still required.

For this handoff change, the complete audio-enabled library suite passes 189
cases with 18 explicit ignores. The private embedded-decoder run passes all nine
full-track hashes/lengths and actual callback checks, including the new rejected
batch/resume path. Strict GUI/all-target checks pass on Rust 1.89 and 1.99.
The default-feature hosted Rust job at source `092f042` passed. At this
checkpoint its full-feature job was active; the later result is recorded below.
This was not a complete workflow pass.

The 32-bit workflow also now preserves an already active run rather than
cancelling it on the next source push, matching the full/native workflow policy.
This lets the existing Windows32/Android32 effect-asset qualification finish.
The newest pending revision can still supersede an older pending revision;
cancelled snapshots do not count as passes. Test concurrency and limits are
unchanged.
Optimized CLI and GUI linking passes with the embedded decoder after this handoff change.


## Independent updater checks in the GitHub desktop fixture (2026-10-18)

Full Rust run `39190363623` at source `092f042` completed: the default-feature
job passed, while the full-feature job reached the desktop GitHub download check
and failed its exact request-count assertion. Its earlier steps, including Rust
checks and native/core/component integrations, had passed. The private fixture
disabled automatic game updates but left the independent default-on app startup
check enabled, producing an additional request for the host's own release.
The same failure reproduces locally. This was test configuration, not failed
package downloading or a product opt-out bug.

The GitHub fixture now explicitly disables both independent background settings.
A new assertion requires no fixture requests before the Download action; the
existing exact two-request assertion remains, as do creator review, explicit
installation approval, library publication and signer/ID/ambiguous-release
rejection checks. The real desktop integration passes with unchanged waits and
assertions. No startup-update behavior is disabled in the application.

The separate real app-updater UI check also passes: default-on startup notice,
saved opt-out across restart, manual checks while disabled, native destination
picker, checksum rejection and staging cleanup, retry, explicit install/restart
and preserved library. All eight settings regressions pass, including independent
app/game polling toggles. These private fixtures use synthetic keys and temporary
libraries only; no real installation or player library is read or modified.

The preserved native run at source `d4d653d` has passed its macOS runtime suite
but failed strict checks on the same Rust 1.99 chunk-iteration diagnostics already
fixed in `092f042`; Windows remained active at this observation. Newer hosted
full/native/32-bit results remain separate requirements, and no failed or
cancelled workflow is counted as a complete pass.


## Original declared-homepage action (2026-10-18)

Recipe 0.1.61 restores the original `Github` menu button at (1170, 670, 100, 40),
including normal/hover art and SDL integer hit rectangles. The exact unchanged
original menu oracle now includes its nine boundary points; all 90 button cases
and the original slider cases remain enabled. Whole-menu pixel references add
the original width-100 button and its hover state without changing tolerances.

The existing scoped window API gains operation 4/value 0. Only the host resolves
the signed manifest homepage; the guest supplies no URL or process argument.
Input permission, a fresh gesture and a successful tick/persistence are required.
One request per tick commits transactionally and is consumed once. Unconfigured,
invalid, closed, unfocused, file-dialog-blocked and failed-tick cases do not open
a browser. URLs retain the existing HTTPS/v3-onion validation. Headless runs do
not dispatch browser requests. Older released hosts need a source rebuild.

Native/core/component desktop tests click the production port handler using a
recording browser executable. Both browser profiles use actual trusted clicks
and a loopback-mapped test hostname, with no opener connection to the opened
tab. A restart regression exposed replay of an old queued frame after the
frontend reset its consumed-request counter. Explicit restart generations now
reject old homepage requests while allowing new post-restart clicks.

One signed-suite attempt failed before gameplay because the workspace disk was
full. Only generated Rust incremental cache was removed, freeing approximately
33 GiB; no user library or source data was touched. A subsequent art check
exposed a separately hardcoded probe-state table that no longer matched the
expanded oracle case order. The probe now derives every state directly from
the independent fixture, preserving old cases and adding the Github hover.
Neither issue was worked around by changing deadlines, limits or tolerances.

Remaining music/resolution/VSync controls, version text and full menu/port
fidelity stay open. This source-only recipe change does not publish a signed
package or a binary release.

Final local checks pass: 101 port logic tests, both complete 71-check signed
native/Wasm suites, 242 GUI host-library cases with 19 explicit ignores and four
public window-control cases. Actual native/core/component window clicks dispatch
only the declared URL, once per click. Both Chromium profiles also verify fresh
clicks after Restart, deduplication and a null opener. Whole-menu comparisons
include normal/hover Github states across native/core/component Vulkan/OpenGL
and both browser profiles, retaining the existing two-unit RGB tolerance.

The expanded native/core/component persistence fixtures prove a valid homepage
request on success, then no retained request after either save or history failure.
The final full signed-suite pass uses ordinary parallelism and unchanged budgets.
An intervening full run hit a procedural-render epoch deadline during concurrent
integration work; the unchanged rerun passes after that work completed. Broader
performance qualification remains open rather than treating one passing rerun
as a guarantee under all system loads. Windows-specific external-browser
dispatch and other native platforms remain unverified.

Browser homepage dispatch also checks the actual save/history write result in
the player. An injected Storage.setItem failure must suppress the requested tab,
and restoring storage must not replay that discarded request. A fresh subsequent
click still works. The persistence helper reports success/failure explicitly;
other call sites retain their existing behavior. This verifies navigation
suppression at that checkpoint, not browser-wide save/history atomicity.
Rollback of other window commands is implemented in the subsequent entry below;
atomic save/history storage remains open.

Strict GUI/all-target checks pass on Rust 1.89 and 1.99. Optimized CLI and GUI
linking passes with the embedded decoder and updated browser export templates.
The seven public HTML pages and local links pass the site checker.


## Browser window and picker persistence acknowledgement (2026-10-18)

Browser workers now prepare window state without committing it. The player
reports the actual save/history write result using the matching frame serial and
restart epoch. Success commits cursor/fullscreen/close/homepage requests; denial
restores the prior committed state. New import/export picker jobs are published
only after success and are cancelled on denial or a committed close. The worker
retains at most one pending transaction, rejects overlapping ticks, ignores stale
or malformed acknowledgements, and clears pending state after errors. Existing
guest API signatures, memory/transfer limits and deadlines are unchanged.

The player maintains one tick in flight and one scheduled timer. Restart waits
for the worker's matching restart acknowledgement before issuing a new tick; old
epoch save/frame/picker messages are discarded. This prevents a late old frame
from publishing window changes, a picker, save bytes or a drawing frame after a
restart. A stale acknowledgement cannot commit a new frame whose serial was
reused in another epoch. Close affects audio only after persistence succeeds.

All browser Node regressions pass, including hostile asset paths, input, clock,
image/clip handling and the new commit protocol. Protocol checks cover prepared
versus committed state, denial, invalid/stale/replayed acknowledgements, restart
serial reuse, import/export publication and overlapping ticks. Actual signed
core/component Chromium fixtures first prove physical confinement/hiding and
then inject separate save/history write failures. Cursor/fullscreen/close/picker
requests must be acknowledged as denied, remain unapplied and never replay on
recovery. Fresh controls and picker requests still work afterwards.

This closes control/picker publication under parent storage failure. It does not
make save plus history a single atomic record, restore mutated guest memory or
roll back all drawing/PCM on browser storage failure. Those broader consistency
requirements and other platforms stay open. New tests use private synthetic
packages and browser profiles; downloads, when requested, stay in temporary
folders. No real user library, ROM or creator key is used.
The later browser picture/PCM publication section closes the drawing/PCM gap
described here; cross-store atomicity and guest-memory restoration remain open.

Final actual-browser checks pass for both profiles and both failure kinds:
import and export requests stay hidden on denial, cancellation works after
recovery, and an explicitly requested fallback download contains exactly the
three fixture bytes in its private download directory. Fresh physical fullscreen
and close still work, and restarting preserves protocol progress. The original
port's core/component homepage clicks, opener isolation, restart and storage
failure/recovery checks also pass with this protocol. No time, memory, transfer
or image tolerance was relaxed.


Hosted 32-bit run `39193999039` at source `93154d4` completed with successful
Linux32 and Windows32 jobs. Direct Windows32 logs confirm all three shared-memory
ownership/counter-width cases and all three signed effect-asset cases passed;
the original-recording case stays explicitly ignored without private inputs.
The Android32 emulator also passes its three memory-budget cases, plus the
selected component/portability/runtime suites. Its three effect-asset cases fail
earlier while creating their signed fixture: `pack` returns Permission denied
at `tests/common/music_fixture.rs:69`. The audio loader assertions are not
reached in that hosted run, which remains a failed workflow.

A separate API 29 x86 Android emulator reproduces the failure as uid 2000 with
the shell SELinux context. Stage-specific pack diagnostics identify the denied
operation as hard-link publication after signing and verification; a minimal
hard-link probe also fails with Permission denied. Android package publication
now uses the kernel's atomic `renameat2(RENAME_NOREPLACE)` syscall. Calling the
kernel directly avoids bionic's API 30-only exported function and retains our
older Android API build target. Unsupported kernels fail explicitly, with no
overwriting fallback. Other platforms retain their existing hard-link path.

On the same API 29 device, all three signed effect-asset tests now pass, including
post-verification CRC corruption and compressed-workspace denial. Three new
publication tests pass on both Linux and Android: complete publication with
existing-file/directory denial, dangling-symlink denial without following it,
and simultaneous publishers with exactly one winner and complete winner bytes.
The original-recordings case remains explicitly ignored without private inputs.
Android CI now runs these publication tests using Cargo's exact library-test
artifact path. This is local emulator evidence, not a claim that a new hosted
workflow or full Android gameplay/audio qualification has passed. Local
portability, path/dependency attacks and signed effect regressions also pass;
strict library Clippy passes. No limits, tolerances or test parallelism changed.

The final fixtures also inject a valid-JSON history record with an invalid
field schema. The player denies that pending window transaction, reports an explicit error
and terminates the worker instead of leaving it waiting indefinitely. The
damaged record is unchanged; the test explicitly repairs it, starts a fresh
worker and proves a new close request succeeds. Missing legacy history fields
receive defaults. This is schema/error-path handling, not a complete browser
history repair tool or a change to every legacy JSON fallback.

All 14 browser Node suites pass with the final implementation. Optimized CLI
and GUI builds pass with the updated self-contained export templates; original
core/component homepage and restart/storage checks pass as well. The seven
public HTML pages and local links pass validation.

## Host effect transactions and shared recording ownership

A host-only effect service now owns one verified package reader, a fixed
32-recording cache and one 32-command tick transaction. It uses the existing
shared memory budget: fixed cache metadata, pending control storage, WAV staging
and immutable PCM all reserve before allocation. Paths use fixed 1,024-byte
storage rather than an unbounded map or separately allocated names. Cache hits
share the exact recording allocation. A full cache rejects new names without
evicting an active recording; the existing 2 MiB per-clip bound is unchanged.

`begin` starts staging, and `finish` calls the trusted all-or-none device batch
publisher only after the caller reports successful execution and persistence.
Failed execution never invokes that publisher. A rejected output batch drops
its pending commands and all newly loaded recordings; an earlier committed cache
entry survives. Finishing twice cannot replay commands. Output-owned clips retain
the parent's pool reservation after the service/cache closes.

Seven public signed-asset/transaction cases pass on Linux64 and actual Linux32,
including permission/verification/path denial, exact command ordering, shared
cache identity, both queue/cache bounds, failed execution, rejected output,
subsequent empty transactions, memory exhaustion and last-owner cleanup. The
Android test binary cross-compiles; these new cases have not yet been executed
on Android. All five original SDL-decoded 44.1 kHz effect recordings also match
complete PCM through the transaction cache in the unchanged 4 MiB pool.

The actual CPAL/ALSA null-device combined music/effect check now loads its effect
from a synthetically signed package through this service, commits the batch to
the real callback, denies a failed tick's unmute/play request, and proves the
output still owns PCM after cache shutdown. All nine complete music references,
embedded-decoder relocation, held-control callback consumption and cleanup pass.
Nine ordinary callback regressions and strict audio-library/integration Clippy
pass. Test parallelism, timeouts, audio limits and reference tolerances are
unchanged. This service does not yet expose game SDK commands, change a running
port's audio, account all package/compiler metadata, or complete native/browser
per-game memory wiring. Those remain required before port migration.

## Saved-list GUI regression target

Direct failure logs from hosted Rust run `39193999093` at source `93154d4`
identify the all-features failure as the second saved-list import never appearing.
A private-display reproduction logs the first list request followed by the
GitHub official-ports release URL. The fixture's generic `Import` OCR target
selected `Import official ports library` before reaching the separate URL-list
import button. The typed URL was present in the input; the fixture clicked the
wrong action. It now targets the distinctive visible `Import .gamepkglist` label.
The unchanged end-to-end saved-list checks pass: restart persistence, targeted
and global refresh, changed/new metadata, failed-source snapshot retention,
cancelled/confirmed removal, installed-package preservation and zero package
downloads. The request log still must contain exactly the two fixture list URLs.
No product behavior, timing limits or assertion scope changed. A future hosted
run must confirm the corrected fixture; the earlier failed run remains failed.

## Desktop game-facing recorded-effect playback

Signed source games can now opt into `limits.effects_memory_bytes`, with audio
permission required and a 64 KiB–4 MiB share inside the existing hard limit.
The effective limit remains the lower of the manifest cap and player ceiling.
The runtime reserves the guest share and the audio child pool together: a 1 MiB
effect share under 16 MiB leaves 15 MiB for Wasm linear memory or native worker
address space. Cached PCM, staging, service metadata and the output converter
use that same child budget. This is not complete accounting for all existing
package/compiler metadata, drawing allocations, device or OS memory.

Generated native/core/component SDKs now provide `effect_play`, `effect_volume`,
`effect_pause` and `effect_stop`. Calls stage the existing bounded host service;
execution, save and play-history failure discard the transaction before device
publication. Native worker opcode 38 has a strict 1,032-byte control-payload bound, core pointers
are checked before copying at most 1,024 bytes, and component strings use the
same bounded path policy. This mode rejects premixed PCM submissions, and calls
without a declared share or trusted output attachment return false. The desktop
app attaches an effects-only original-rate stream and the atomic publisher;
muted sessions accept committed controls without opening a device. Browser export
explicitly rejects this mode until browser processing/accounting is implemented.

Four ordinary runtime checks pass on Linux64 and actual Linux32: verified core
and component control ordering/cache identity/complete PCM, traps/save/history
rollback, output rejection followed by a clean tick, and actual initial-memory
rejection at the reduced guest ceiling. Manifest cases cover legacy omission,
round trips, audio permission, bounds and a share exceeding the hard cap. A
component trap prevents Wasmtime re-entry; another attempted tick cannot replay
audio. The current app stops failed sessions rather than promising guest-memory
restoration or continued component execution after a trap.

Private genuinely signed no-dependency source fixtures compile through all three
generated SDKs. Tests explicitly select Wasm or native execution (no native
fallback), verify full recording PCM and shared cache identity, then force trap,
save, history and output failure. Backend phases use fresh history so a new
achievement forces the history write. Actual CPAL/ALSA null-device callbacks
receive these game-issued effects, continue beyond the last successful tick,
and cannot replay a trapped tick. The expected blocked-history diagnostic is
retained while the fixture's directory is deliberately unavailable.

`examples/good/recorded-effects` contains a generated Apache-2.0 chime and no
guest dependencies. Its real native/Wasm game windows respond to held Enter and
continue rendering in Vulkan and OpenGL with private null audio. Existing stereo
SDK cases still pass in native/core/component execution and actual Chromium
WebAudio. Optimized CLI/GUI builds pass. The complete all-features host library
passes 249 cases (19 specialized cases explicitly ignored); selected public
asset/component/dependency/manifest/example/path/runtime/source suites pass 46
more cases (one private-original case explicitly ignored). Strict
library/bin/test Clippy passes after moving the Android publication test module
to the end of its source file. No execution deadline, reference tolerance,
resource cap or test parallelism was relaxed.

CI now runs the signed SDK/device and real-window checks. These are local Linux
results; Windows/Android game-facing playback, physical output, browser support,
compressed workspace, music SDK controls, original port scheduling and migration
remain open. No existing port, catalog package or binary release changes here.

Hosted 32-bit run `39197971735` at source `06f0d7a` is now complete and successful
for Linux32, Windows32 and Android32. Direct Android job `117884994236` logs
confirm 11 component, four portability, ten runtime, three memory-budget, three
signed effect-asset and all three exclusive-publication cases pass. The private
original-recordings test is explicitly ignored there. This confirms the earlier
Android hard-link publication fix and the selected suites at that source; it
does not establish Android execution of the later effect cache/SDK/device code.
The actual CLI also packs, verifies, installs, compiles and runs all 20 good
examples, including recorded effects, while rejecting all seven bad examples
without publishing partial packages.

## Browser picture and PCM publication after persistence

A stronger signed core/component browser fixture reproduced a save-denied frame
that still advanced the visible state, changed canvas pixels and created another
WebAudio buffer. The worker acknowledgement was already false; the page rendered
before persisting history and its audio condition still admitted rejected frames.
The same real-browser assertions now pass with publication held until commit.

The player prepares drawing in one reused 640×480 canvas. Shader and filtered
image helpers receive that staging target, including the source pixels sampled
for compositing. Successful persistence copies the opaque prepared frame to the
visible canvas and publishes score/state metadata. Failed save/history writes
leave the previously committed pixels and displayed state unchanged and schedule
no new PCM. A known failed save also prevents the subsequent history write.
Already committed sound may finish; rejected-frame sound is never scheduled.
Restart and halt share cancellation of all queued sources and reset the audio
scheduling cursor, preventing previous-generation PCM from surviving Restart.
A successful recovery also clears the stale storage warning and restores the
Playing status; fatal or unrelated messages are not cleared.

All 15 browser Node suites pass. New staging checks prove one fixed allocation,
separate visible/staged targets, shader/filter target propagation, withheld
metadata, balanced drawing state after failure, and complete audio cancellation
when one ended node throws. Actual signed core/component Chromium fixtures prove
valid stereo PCM first, then separately deny save/history writes and observe the
canvas, displayed state and AudioBuffer creation after each rejected frame. Save
failure attempts no history write. Existing cursor/fullscreen/close/import/export
rollback and fresh recovery remain checked, including actual export bytes.
Restart synchronously stops the pending sources, and invalid history field
schemas still terminate without overwriting the damaged record.

Actual core/component fresh/absent/translucent/transparent/empty frame pixel
checks pass. The full SDL filtering corpus passes in native/Wasm Vulkan/OpenGL
and both browser profiles, including moved/swapped images, at unchanged existing
tolerances (browser maximum one 8-bit channel unit). Shader/RGBA/PNG/audio export
checks also pass in actual Chromium for both profiles. Optimized CLI and GUI
builds embed the updated player. No deadline, image/drawing/audio cap, tolerance
or test parallelism changed.

This closes transient picture/state/PCM publication on browser storage failure,
not atomic save-plus-history storage or guest-memory rewind. A save can already
have been written when history fails, and guest logic continues while output is
withheld. Browser recorded-effect processing, music controls, repair UX and full
port migration remain separate open work. CI includes the new Node check, all 15 existing/current browser Node suites,
and the expanded actual-browser persistence fixture.

Direct logs from the earlier hosted Rust run `39198679470` also identify two
strict-lint failures in the publication race tests. Moving that test module was
already fixed in `716f6e4`; the remaining Rust 1.99 byte-character-slice lint now
uses the equivalent copied two-byte array. Strict library/bin/test Clippy passes
on both Rust 1.89 and 1.99 without lint suppression. The three publication tests
retain their overwrite/symlink/concurrency assertions.

### Atomic browser checkpoints and asynchronous lifecycle

Exported games now commit save bytes and play history in one IndexedDB
transaction. A failed history write rolls back the save even if its individual
write request already succeeded. Legacy localStorage records are read only when
an IndexedDB record is absent and remain intact as recovery copies; malformed
records fail explicitly rather than being overwritten. Concurrent tabs merge
history increments and achievement IDs under the same transaction. Save bytes
retain last-writer semantics.

Actual Chromium tests verify request-success aborts, quota rollback, early
cancellation, connection recovery, creator separation, strict record validation,
and the complete hash of a maximum-size synthetic save. Both browser runtime
profiles verify that delayed startup cannot revive a stopped game and delayed
checkpoint completion cannot publish picture, audio or controls after Stop or
Restart. The lifecycle test is included in CI alongside the persistence test.
Original port tests pass both 71-check signed runtime suites and native/core/
component gameplay, editor and file workflows. The paddle observer now reads the
same two pixel rows directly instead of serializing a full image; an exact
byte-equivalence assertion verifies the crop. Movement thresholds, input duration,
execution limits and rendering tolerances are unchanged.

This supersedes the browser save/history cross-store limitation above. Native
cross-file persistence, guest-memory rewind, repair UI, browser recorded effects,
music SDKs and full port fidelity remain open. No binary or package release is
part of this change.

### Music Stop and replacement output boundaries

A deterministic regression held the music ring's output-consumer guard while
issuing Stop or replacement from another thread. Both controls previously
returned before that already admitted read left its guard. They now wait at the
control boundary, as Pause already did, after publishing the new generation.
Later reads reject the old generation. The output callback still never takes
the producer/control mutex. This bounds software read completion; it does not
retract audio already submitted to an OS device.

The test first failed on the previous implementation. With the fix it verifies
both controls against active playback, exact complete replacement-track samples,
stopped-read sentinel preservation and complete budget release. All eight music
unit cases and three combined music/effect conversion cases pass with unchanged
memory, queue and decoder limits. Music SDK activation and port migration remain
open.

### Whole-tick music control publication

The host music service accepts a maximum 32-command batch. It validates every
path, gain, active-state transition and generation increment before mutating
playback. A batch containing Stop followed by an invalid Pause, a bad path, an
invalid gain, an exhausted generation or too many commands leaves prior playback
unchanged. The final track request is prepared before publication; intermediate
same-tick replacements have no output interval. Generation, status and gain publish as one atomic setting, so an output frame
cannot observe a partially applied batch. Publication does not exclude the
output consumer or introduce silence while a control guard is held. The callback
never waits for a control mutex.

The new test verifies rejected batches against active playback, the exact full
old-track output, ordered Play/Stop/replacement/pause/gain behavior, the exact
full replacement output, both command-count boundaries, generation exhaustion
and complete budget release. A concurrent 10,000-cycle test verifies that pause and gain are always observed
as one valid pair, followed by exact full-track playback. All ten music cases
pass. This is the host commit
primitive needed for game SDK integration; no runtime currently exposes music
commands to games, and shared music/effect tick publication, SDK/budget wiring,
browser processing and port migration remain required.

The signed original-track fixture now starts each of all nine tracks with an
ordered Stop/Play/gain/Pause batch, verifies paused-read sentinel preservation,
then resumes through a second batch. Every complete pinned PCM hash and original
mixer/output length passes, with concurrent verified asset reads and the embedded
decoder's runtime paths removed. Private CPAL/ALSA null-device music, combined
mixing and effect transaction checks also pass. Strict Rust 1.89 and 1.99 checks
pass; atomic updates use explicit compare/exchange loops compatible with both.
No tolerance, memory/queue bound, decoder deadline or test concurrency changed.

### Budgeted staged music transactions

The host now owns a fixed, 32-operation music transaction buffer. Begin starts a
transaction; controls copy the final asset path into owned storage; finish only
publishes after the caller reports successful execution and persistence. Failed
transactions leave prior music running. Invalid successful-tick batches are
consumed without replay. Capacity, path and gain checks apply before staging.
Only the final same-tick track replacement needs path storage; all operation
ordering and generation increments are retained.

The initial per-operation path layout consumed decoder capacity and failed the
playback test. The corrected layout fits within the existing 64 KiB host
reservation, whose size assertion now includes transaction storage and its
command scratch array. No decoder, parent, queue or host allowance increased.
Tests verify owned path lifetime, failed-tick rollback, rejected-batch cleanup,
no replay, fresh playback, capacity boundaries and complete reservation release.
This is trusted host infrastructure; runtime SDK dispatch, shared music/effect
publication, browser controls and actual port migration remain unfinished.

All nine signed original-track fixtures now use the staged transaction rather
than direct controls: each first discards a missing-track/mute request, then
commits the real track paused and resumes it in a fresh transaction. Complete
pinned PCM hashes, output lengths, concurrent asset reads and reservation cleanup
pass inside the unchanged 16 MiB parent budget with the embedded decoder. Private
null-device music, combined mixing and effect checks pass. Rust 1.89 and 1.99
strict checks pass. These tests simulate the host's success/failure signal;
actual game tick, save and history wiring still needs implementation and proof.

### Desktop game music SDK and shared memory wiring

Signed `music_memory_bytes` now partitions the existing hard limit, alongside
`effects_memory_bytes`, before applying the guest's runtime/native process limit.
Both shares must leave guest memory under the selected player ceiling. Music
activation checks the bundled decoder declaration and output staging fit inside
the declared share. Decoder and fixed control storage retain existing bounds;
no new independent memory allowance is created. The source-only manifest field
requires audio permission and a source game. Existing manifests serialize without
a new field unless they declare music.

Dependency-free game helpers provide play, integer gain, pause/resume, stop,
natural-completion and asynchronous-failure queries. The existing bounded
native/core/component control transport carries them. Runtime begin/finish hooks
stage controls alongside effects. Music validation runs before effect output
acceptance; invalid music batches discard effects, and rejected effect output
prevents music publication. Failed guest execution, saves and history discard
both. Worker faults after acceptance remain asynchronous failures; a new Play
can recover. The desktop output attaches the combined original-rate music/effect
source to the existing device-clock converter. Premixed PCM cannot be submitted
in either recorded-audio mode.

Private signed fixtures exercise disabled, combined and music-only declarations
in native/core/component runtimes, including insufficient player ceilings. They
compare every generated MP3 sample across backends and against the independent
SDL original-mixer output with the existing two-unit decoder bound. Completion,
missing-track failure and recovery, traps, failed save/history writes, denied
output and invalid control batches pass. Actual null-device callbacks finish
music with no further game ticks and cannot replay after a trap. The older
signed effect SDK suite still passes. All 254 non-ignored host library cases and
the selected runtime/budget/asset/manifest integration tests pass. CI includes
the new signed SDK fixture, and developer/spec pages describe the APIs.

This closes desktop SDK/budget activation, not full music fidelity. Browser
music, migration of current ports, playlists/settings parity, exact combined
control-to-callback timing, physical devices, broader platform playback and full
OS/compiler/allocator memory accounting remain open. No signed package or binary
release is included.

The optimized CLI and GUI both build with the embedded decoder, and strict
checks pass on Rust 1.89 and 1.99. Actual 32-bit Linux executes all eleven music
cases available without the optional audio-device feature and the new manifest
share test. The all-features 32-bit attempt lacks this machine's cross-compiled
udev development setup; no 32-bit device or GUI proof is claimed. The portability
workflow now includes the manifest test on Linux32, Windows32 and Android32;
its Android runner expects the corresponding sixth test executable. Those
hosted runs are not counted as passed by this local evidence.

### Original music contract before port migration

Two independent oracles now check the music behavior that the port must retain.
`tests/music_loops.c` plays generated stereo WAV data through actual SDL_mixer
music callbacks. Values zero and one both produce one complete pass; two produces
two passes. Every sample and the absence of gaps between passes are verified.
Thus the original `Music::play(1)` does mean one pass for this API; the new desktop
SDK's one-pass behavior needs no extra repeat at that boundary.

`tools/check_original_music.py` compiles the unchanged pinned `src/audio.rs`,
checks its complete source hash and supplies traced SDL controls and deterministic
random draws. Real temporary directory enumeration verifies sorted lowercase-MP3
selection and ignored extensions. The oracle checks random startup and next-song
selection, startup play before saved mute settings, paused muted playback,
volume changes while paused, unmute/resume, no advance while playing or muted,
stop disabling automatic advance, empty playlists, load failure followed by a
new random choice, gain clamping and all effect controls. In particular, the
original pause is essential: its volume setter can update gain even while muted.
Both oracles pass locally and have CI gates. No original recordings or modified
upstream source are added by these tests.

The control oracle traces original decisions; it is not proof of the current
port's music playback. That port still uses guest-rendered effect PCM and lacks
music/settings integration. Migrating its shared package now would break browser
export because browser recorded audio is still rejected. Browser decoding,
output-clock mixing and transactional controls must be implemented before the
shared port can migrate with its desktop and browser behavior preserved.

### Browser recorded-audio decoder and supervised worker

A trusted browser decoder adapter now executes the same import-free companion as
the desktop host. Before instantiation it validates bounded 32-bit, unshared
memory, bounded tables, required i32 function signatures, the memory export and
absence of imports/start functions. The companion remains capped at 6 MiB of
linear memory, modules at 4 MiB, encoded input at 8 MiB, output at 1,600 interleaved stereo
samples (800 frames) per call and encoded-read work at 512 KiB. Returned ranges, sample counts,
format and work values are checked before copying explicit little-endian samples
to caller-owned output. Decoder faults are sticky; rewind and explicit close are
supported. These are decoder limits, not a claim about the whole browser heap.

The worker supervisor permits one pending operation and terminates the worker
on invalid responses or execution deadline expiry. Open/read/rewind retain the
existing 100 ms decoder deadline; module startup/compilation have separate bounded
five-second initialization waits. Replies arriving beyond their deadline are
rejected even if timer delivery was delayed. Startup can be aborted, pending
reads can be closed, and fresh workers recover independently. PCM responses must
own an unshared buffer of the exact returned size. No game receives worker,
decoder, network, filesystem or device capabilities.

Native Wasmtime and JavaScript execution match complete original-rate PCM hashes
and sample counts for all nine pinned tracks, across varied read sizes. Actual
Chromium matches a complete track both directly and through the supervised
worker. Fault fixtures cover malformed function signatures, capabilities,
unbounded/oversized/multiple memories, tables/start functions, invalid PCM counts
and ranges, over-work, failed open/rewind and sticky errors. Deliberately infinite
open/read functions are terminated by the parent deadline. Cancellation,
backpressure, close during a read and fresh recovery pass; all eight worker
handles created by the fixture are terminated.

A deterministic red regression also mutated supplied module bytes between
validation and compilation. The adapter now validates and compiles the same owned
snapshot. Input copies use captured bounds, and decoded output never aliases the
companion memory. No decoder/time cap or existing comparison tolerance increased.
CI runs the native/JavaScript corpus and actual Chromium checks from private
fixtures; no recordings, decoder binaries or packages are published by the tests.

This decoder layer is not yet activated by browser export. Browser output-clock
mixing, shared transport/cache/queue budget accounting, game transaction wiring
and port migration remain unfinished. Declared recorded-audio browser exports
still fail explicitly until those pieces are integrated and verified. The
Chromium proof's full-track hash buffer is test-only, not a production queue.

### Browser original-rate mixer and shared output conversion

The browser mixer now supports four first-free effect voices with immutable
owned PCM, integer gain, pause, stop, slot reuse and active-clip unload denial.
Music enters first; each voice adds and saturates in slot order. Muted effects
still advance and paused effects freeze while music continues. Cache records are
fixed at 32 IDs; each stereo recording is bounded to 2 MiB. PCM backing buffers
are charged before publication inside the selected share, together with the
converter's declared memory and fixed 64 KiB staging reservation. Invalid sizes
and exhausted capacity leave prior playback unchanged. These buffer reservations
are not a proof about the browser engine's entire object heap.

One new import-free converter companion reuses the existing Rust SDL adaptation
at 44.1-to-48 kHz. Its maximum linear memory is 256 KiB, including a fixed 128 KiB
stack. Its input/output and conversion storage are fixed; conversion performs no
allocation after initialization. The JS adapter and companion use explicit
little-endian S16 buffers, capped at 800 stereo frames per operation. Music
passed to this layer already has its gain applied, as in the native mixer.

Every one of the 59,179,008 SDL source/destination/volume arithmetic combinations
matches the pinned hashes. All eighteen combined music/effect conversion hashes
match at the existing three buffer schedules and six gains. Three more checks
freeze the effect voices and retain the original music-only output. Ownership,
first-free reuse, active unload denial, cache/clip bounds, pause, silent
advancement and rejected-buffer preservation pass. Actual Chromium AudioWorklet
output matches complete SDL sample hashes and lengths at all six gains and with
paused effects, without game ticks; completed output tails are zero.

Shared companion validation now handles both decoder and converter signatures
and does not depend on TextDecoder, which is unavailable in some worklet scopes.
The existing nine-track native/JavaScript decoder corpus and actual Chromium
worker deadline/cancellation/full-track tests still pass after this refactor.
No reference hash, tolerance, existing audio cap or decoder deadline changed.

This remains a separately qualified browser audio component. The production
worklet's live queue/transport accounting, game frame/control transactions,
companion embedding/export, and port migration still need wiring and proof.
Recorded-audio browser exports remain explicitly disabled. No converter binary,
recording or signed package release is published by this change.

The converter passes strict Wasm-target checks on Rust 1.89 and 1.99. Optimized
artifacts built by both compilers preserve every arithmetic/conversion hash and
the actual AudioWorklet output. An additional muted-master AudioWorklet graph
continues through all 6,530 output frames with entirely zero external output.
This distinguishes application master mute from pausing effect voice positions.
CI builds/checks the separate companion and runs the Node/Chromium qualification.

### Live browser queue and muted desktop music qualification

The browser recorded-audio components now include a fixed 4,800-frame stereo
queue and output-clock stream. Their owned PCM backing storage totals 22,656
bytes inside the existing host staging reservation. Replacement generations
reject stale packets; pause freezes music positions, zero gain advances them,
and failure/stop discard queued music while effects continue. Previously
converted samples retain the converter's existing boundary semantics.

An independent SDL C oracle verifies eight live music/effect output prefixes.
All 24 Node read-schedule comparisons match those hashes. Fourteen actual
Chromium AudioWorklet cases verify combined output, independent music/effect
pause, failure, stop, underrun and master mute, including clean output tails.
This uses a deterministic local producer: supervised decoder-to-worklet
transport, transport memory accounting, committed game controls, companion
embedding/export and port migration remain unfinished. Recorded-audio browser
export remains disabled; these component checks do not prove end-to-end browser
playback or whole-browser heap accounting.

Desktop music now retains its real audio output clock when master mute or zero
volume is selected. A missing device rejects playback rather than attaching a
successful no-op publisher. Sixteen private real-window cases pass across
native/core builds, Vulkan/OpenGL, audible/muted/zero-volume output and absent
devices. CI runs the same generated synthetic-audio fixture. No game recording,
key, screenshot or package artifact is published.

A staged pause also tolerates the same track naturally ending between tick
start and control commit, preserving completion and gain without rejecting the
tick. Thirteen music unit cases and the signed native/core/component SDK
qualification pass, including existing rollback checks. Strict all-feature
Clippy checks pass. A separate injected decoder trap now reproduces the corresponding failure
race: the same active-at-begin generation retains Failed while its staged gain
commits. The focused music suite passes all thirteen default-feature cases,
including strict invalid-batch rejection and existing stop/replacement checks.

The live browser components also have a point-to-point MessagePort transport.
The output side credits exactly one 800-frame decode only when its fixed queue
has capacity. The sender retires the old decoder on Stop/replacement; late
successful and failed responses cannot change the new generation. Credit IDs
must increase, malformed current-generation responses fail the track, and EOF
closes its decoder after queued PCM is admitted. Real Node MessageChannel tests
verify full PCM ordering, full-queue backpressure, replacement during an awaited
read, stale success/failure, Stop, wrong credits and codec rejection. The live
qualification tool includes these tests. The sender accepts the supervised
client interface, and real Chromium MessagePort-to-live-AudioWorklet delivery now matches all six
independent SDL hashes, complete 6,000-frame consumption and source closure.
The independent SDL reference uses a deterministic asynchronous source. An
additional generated MP3 passes through the actual supervised decoder worker,
credited MessagePort and live AudioWorklet: all 32,768 output scalars match
direct decoding/mixing, all 11,520 source frames play and the worker terminates
at EOF. This proves the component connection, not committed guest controls or
whole-browser memory accounting. Shared transport memory reservations, signed
permission/export integration and migration of the existing ports remain open.
This does not enable export.

Transport shutdown now explicitly retires the peer's source, including a read
awaiting completion and closure before the first Begin message is processed.
Regression tests first reproduced both leaked-source cases, then passed after
adding connection-level close notification independent of track generation.
Closing the sender marks an active receiving track failed rather than leaving
it loading indefinitely. Real Chromium additionally verifies actual MP3 worker
replacement and Stop/restart while a retired read's deliberately corrupted
result is delayed: each current track still matches all 32,768 direct output
scalars, consumes 11,520 frames and closes both current and retired workers.

A disconnect regression also reproduced 4,800 stale queued frames surviving
receiver closure. Closure now discards unconsumed music and preserves a failed
state, including EOF received before the queue drains. Already completed tracks
retain Ended. These cases pass through actual MessageChannel delivery; samples
already admitted to the converter/device remain subject to its existing boundary
semantics. This change does not promise retraction of already submitted output.

The latest host changes also pass all 256 enabled all-feature library tests
(19 remain explicitly ignored), strict Rust 1.99 checks and thirteen actual
Linux32 music cases, including both new tick-boundary races.

Browser recorded audio now has ownership-based parent/child reservations.
Closing a child prevents new reservations while retaining its entire parent
charge until its last lease releases. The supervised decoder accepts a shared
budget and reserves its declared linear-memory ceiling, two owned module
snapshots, one transferred encoded snapshot and three bounded PCM blocks before
creating its worker. Close, startup failure and decode failure release that
charge. Actual Chromium verifies concurrent-open refusal and cleanup after both
normal close and malformed MP3 startup; live PCM and replacement regressions
still pass. This parameter remains optional for component qualification. The
exported game runtime has not yet supplied the common guest/effects/music
ledger, and caller-owned source buffers, browser compiler/JIT and GC overhead
are outside this reservation formula. These checks do not establish whole-page
memory enforcement or enable recorded-audio export.

Real-worker budget qualification now also closes a child while its decoder is
live, proving the parent capacity remains unavailable until decoder shutdown.
Immediate startup cancellation returns its reservation. Normal playback,
replacement and Stop/restart use a common decoder ledger and return its usage
to zero at EOF while retaining every exact mixed sample. Values returned to an
arbitrary caller remain caller-owned; retaining unlimited decoded blocks is not
made safe by the worker reservation. The credited transport avoids that usage
pattern, but its own cross-generation outstanding-buffer accounting and runtime
parent ownership still need integration before export can be enabled.

The credited transport now optionally shares the same decoder ledger. Each
requested 800-frame stereo block reserves 3,200 bytes before decoding, retains
its charge through message delivery, and releases it on receiver admission/drop
acknowledgement. A retired read keeps its lease until it settles, even after
sender closure. A controlled outstanding old read proves that a replacement
cannot reuse that capacity or reach the decoder when the ledger is full.
Normal full-queue backpressure returns all packet charges after admission.
Real MP3 playback, replacement and Stop/restart now charge decoder and packet
reservations to one parent and finish with zero usage, preserving all exact
output scalars. Converter/cache/queue capacity and guest-memory reservations
still need the runtime's common parent; browser compiler/JIT/GC overhead remains
outside these explicit backing-store reservations. Connection teardown drops
sent packet ownership; it does not prove synchronous browser GC reclamation.

The actual-browser transport qualification now partitions one parent into a
64 KiB placeholder guest reservation, the real worklet's 4 MiB effect capacity,
and its decoder/packet allowance. It denies a further byte, retains guest and
effect ownership after music cleanup, and releases the effects share only after
AudioContext shutdown. All three MP3 lifecycle paths preserve exact output and
return parent usage to zero. This is a component ownership test: no exported
game worker occupies the placeholder guest reservation yet, so guest-memory
integration remains unverified rather than implied by this fixture.

The exported player's real game worker now owns a parent ledger and a guest
linear-memory ceiling reservation. Startup reserves before creating the worker;
Stop/error teardown calls terminate before releasing it. Failed worker creation
cleans up and reports an explicit startup failure. The budget module is embedded
in each export. Actual exported core/component Chromium lifecycle tests observe
reservation/creation/termination/release order across delayed startup, delayed
checkpoint, Stop and Restart. Both also inject worker-construction failure,
verify zero retained capacity and then launch successfully again. Existing frame
publication and 600-tick clock checks pass; the clock fixture now supplies the
clearTimeout stub required by the existing production scheduler. No production
clock, memory limit or timeout changes.

Recorded-audio exports remain rejected. Their effect/music shares still need to
partition this actual parent, and committed guest controls, companions and port
migration remain open. Reserving guest linear memory does not account for every
asset, DOM, graphics or browser/JIT allocation.

Browser guest bindings now recognize core/component recorded-audio calls and
stage at most 32 ordered commands in the current frame. Calls require signed
audio permission, the corresponding declared share and a host-ready snapshot;
read-only music completion/failure uses the same tick snapshot. Paths must be
valid existing package assets, and retained commands reference package-owned
keys rather than accumulating copied guest paths. Numeric/path bounds, absent
output, absent permission, per-tick reset, trap cleanup and failed checkpoint
no-replay pass worker tests. Matching checkpoint acknowledgement clears staging;
existing window/file acknowledgement, input and frame-clock regressions pass.
Declared recorded-audio modes reject premixed PCM, matching the desktop API.

The player does not yet supply recorded output readiness or consume these staged
commands. Backend cache/admission validation, all-or-nothing effect/music
publication after checkpoint, actual game SDK/browser device proof, companion
embedding and port migration remain open. Exports with recorded-audio shares
remain explicitly rejected; staging tests do not establish audible game playback.

The live stream also validates a complete host-resolved numeric control batch
before applying any changes in one synchronous worklet task. Cached clip IDs,
four voice slots, ordered Stop/Play, gains, pause values and strictly increasing
music generations are checked together. Invalid late commands leave gain,
voices, generation, queued PCM and exact audible output unchanged. A pause of
the same active-at-tick-start generation preserves asynchronous Ended/Failed.
Node tests cover rejected and ordered valid batches; actual AudioWorklet checks
exercise rejection and valid gain/pause while retaining all six SDL hashes and
all three exact MP3 lifecycle comparisons. Resolving guest asset paths and
committed frames into these backend batches is still unfinished; this primitive
does not independently publish game commands or enable recorded-audio exports.

A production recorded-audio AudioWorklet endpoint now owns the live stream. Its
session-scoped, monotonically numbered requests load/unload bounded cached clips,
apply validated batches, inspect playback, attach the credited music port and
acknowledge closure. Wrong-session and stale requests are ignored. Its fixed
3,200-byte quantum buffer plus existing stream buffers fit within the existing
64 KiB host staging reservation. Invalid output quanta/fatal render faults close
the endpoint and zero that quantum; decoder faults remain track states.

Actual Chromium verifies nonzero output through an analyser, rejection without
partial effect publication, stale/foreign request isolation, active clip unload
denial, output-clock effect completion and all 11,520 supervised MP3 frames,
followed by acknowledged shutdown. Existing exact SDL and direct MP3 comparisons
remain unchanged. This endpoint has no test capture buffer and runs independently
of game ticks. A supervised player-side request client, effect decoding/asset
resolution, committed-frame dispatch and export embedding still need integration;
recorded-audio exports remain rejected and current ports remain unmigrated.

The production player-side recorded output supervisor now owns a dedicated
AudioContext, one pending request, session/request identity and bounded response
validation. Module startup uses the existing five-second startup allowance;
requests use the existing 250 ms browser execution deadline, including rejection
of late replies. Startup cancellation is immediate. A parent reservation covers
the effect/worklet share and three simultaneously owned converter snapshots;
closure retires an attached music sender and releases capacity only after the
context's close promise succeeds. Failed context shutdown retains the charge.
Caller-owned assets and browser/JIT/GC overhead remain separate.

Actual Chromium passes request backpressure, master-zero output-clock startup,
connected decoder shutdown, reservation retention during context shutdown,
immediate cancelled startup and fresh recovery. Dropping a real snapshot request
reproduces an unresponsive endpoint and verifies deadline termination/cleanup.
All existing exact SDL/MP3 and production endpoint checks still pass. Frame
checkpoint dispatch, effect asset decoding/admission, manifest share wiring and
export embedding remain unfinished; recorded-audio exports remain rejected.

Effect asset admission now checks the desktop host's canonical 44-byte RIFF/WAVE
header, exact 44.1 kHz stereo S16 format, byte rates/alignment and 2 MiB PCM cap.
The parser returns an existing little-endian PCM view; it allocates no decoded
copy. All 44 individual header-byte mutations, short/misaligned/oversized clips,
exact minimum/maximum, offset views and shared/invalid backing stores are tested.
The output supervisor's WAV loader reserves its owned transfer staging before
copying, awaits worklet cache admission, and retains staging through context
shutdown when a pending load is cancelled. Real Chromium verifies generated WAV
playback, malformed rejection, unchanged caller input, returned staging capacity
and cancellation cleanup. The worklet cache share and transfer staging must both
fit the same supplied parent; no allowance or clip cap was raised. Frame-level
asset resolution/cache rollback and final manifest share wiring remain open.

A committed-generation transport regression first reproduced a permanent stall:
the output batch had already established Play's generation, so the later
transport Begin was rejected and no decode credit issued. The receiver now
tracks peer announcements separately and admits that already-committed loading
or paused generation once. It still ignores duplicate announcements and stale
generations; externally committed Stop also synchronizes transport retirement.
Node cases cover loading/paused attachment, duplicate Begin, bounded prefill and
Stop. Actual production endpoint and supervised-client Chromium cases commit
Play+Pause before decoder attachment, fill exactly 4,800 queued frames without
advancing, then resume and complete all 11,520 source frames or close cleanly.
Existing exact audio comparisons, reservations and supervisor faults still pass.
An older EOF assertion now waits for actual delivered EOF rather than assuming
worker closure and receiver message delivery occur synchronously.

The production audio endpoint now supports two-phase frame controls. Prepare
validates and copies at most 32 commands without changing output, retaining one
session/epoch/frame record. While prepared, conflicting cache or direct control
mutations are refused. Finish accepts only the matching frame and a boolean
checkpoint result; failure discards it, success applies it once. Consumed frame
identities cannot be replayed and overlapping prepares cannot replace a pending
frame. Stream validation itself preserves voices, queued music and audible PCM.
Actual Chromium verifies failed-checkpoint silence, stale/overlapping refusal,
cache retention and fresh successful publication, followed by the existing exact
audio/lifecycle tests. The guest worker now includes its active-at-tick-start
music generation in the staged frame, clearing it on acknowledgement/trap so a
future dispatcher can preserve natural EOF/failure semantics for that generation.
The player still needs to resolve/admit staged asset paths and wire its actual
storage checkpoint to these prepare/finish calls. Exports remain disabled for
recorded audio; this endpoint protocol alone does not prove game-frame playback.

A production frame dispatcher now resolves staged effect paths from the host's
package asset Map, admits canonical WAVs into first-free bounded cache slots,
and prepares their numeric control batch without starting playback. A successful
checkpoint promotes new cache entries; a failed checkpoint unloads them. The
complete wire schema is checked before asset admission, and only one prepare or
finish can run at once. Matching frame identity is required. Closing during
admission shuts down the output so late completion cannot revive playback.

Actual Chromium exercises the dispatcher with the production supervisor/worklet:
admission without playback, failed-checkpoint cache removal, fresh successful
publication, wrong frame refusal, existing-voice preservation when a later WAV
is malformed, rollback of a preceding valid admission in that same frame, fresh
recovery and close during an awaited load. Existing exact audio/lifecycle checks
still pass. Music Play/Stop resolution is explicitly rejected until its async
loading and generation ownership are wired; it is never silently skipped. Actual
player storage dispatch, full share allocation and recorded-audio export remain
unfinished. No package/binary release or game recording is published.

The frame dispatcher now resolves music Play/Stop too. It assigns increasing
worklet generations while preparing and retains only the final track action.
A failed checkpoint starts no decoder and cannot stop the existing track.
After a successful combined batch, the asynchronous music loader retires old
output, cancels older startup, opens the supervised decoder from the shared
budget and attaches only a still-current result. Decode/open failure publishes
that generation's Failed state; cancelled/retired loads cannot overwrite newer
music. Stop and dispatcher closure cancel pending startup and retire sources.

Actual Chromium combines paused effect/music publication under one 16 MiB
component ledger, verifies discarded Play and Stop, malformed MP3 failure,
fresh complete 11,520-frame recovery, committed Stop during a new startup and
closure during replacement. Two Play commands in the final frame create only
one decoder. All five created decoder workers terminate and reservations return
to zero. Existing exact PCM, cache rollback, production endpoint and supervisor
fault tests still pass. This supersedes the earlier effect-only dispatcher's
explicit Music Play/Stop refusal. Integration with the actual player checkpoint,
manifest share partitioning, companion embedding and port migration remain open;
recorded-audio exports stay disabled.

Effect transfer admission now reserves the target cache slot before making the
owned PCM copy. The worklet adopts that transferred, unshared, exact-span buffer
without a second PCM copy. Thus the cache share covers the payload while it is
in transit as well as after admission; the earlier separate full-payload staging
charge is no longer needed. Reservation tokens bind admission/cancellation to
one request; pending slots cannot play. Stale tokens, shared buffers, wrong
indices/sizes, overcapacity and active unload remain denied. Cancellation returns
only its matching unfinished slot. The ordinary snapshot-copy mixer load API
retains its previous behavior for trusted test callers.

A four-byte exact-capacity mixer test verifies transferred ownership and token
isolation. Actual production WAV loading now succeeds with zero additional
parent headroom beyond its reserved output share. All 59,179,008 SDL arithmetic
cases, eighteen combined hashes, stream comparisons and real browser endpoint,
checkpoint rollback, combined music and shutdown checks still pass. This fixes
temporary duplication without increasing any allowance or reducing the 2 MiB
clip limit. Manifest share/session allocation and actual player checkpoint
integration remain open; recorded-audio exports remain disabled.

A production recorded session owner now allocates declared effect/music children
and the remaining guest ceiling from one parent. It subtracts all three
converter snapshots from the selected output share, uses the effects child for
the converter/cache when present, and otherwise charges the minimum output
share to music. Decoder and packet leases stay inside the music child. Invalid
permissions, oversized shares, insufficient guest space and insufficient output
capacity fail without increasing any limit. Session closure terminates its
attached guest before releasing that ceiling, then closes audio owners before
retiring the child capacities.

Actual Chromium verifies combined (4 MiB effects/8 MiB music/4 MiB guest),
effects-only (4/0/12 MiB) and music-only (0/8/8 MiB) allocations under the same
16 MiB parent. A real test worker instantiates bounded Wasm memory in each case;
paused effect/music frames, shutdown order and zero retained parent usage pass.
Invalid startup, immediate cancellation and cancellation while attaching the
music port also release ownership. These are real worker/audio component tests,
not yet the exported game's worker: player startup/checkpoint dispatch, reduced
guest module capping, companion embedding and port migration remain open.
Caller-owned encoded assets and browser/JIT/GC overhead remain outside the
explicit guest/audio backing reservations. Recorded-audio exports stay disabled.

### Signed recorded-audio browser player integration

The actual exported player now owns the recorded session, attaches its game
worker, obtains one output snapshot per tick, prepares audio before the shared
save/history checkpoint and finishes that exact frame afterward. Stop cancels
startup, closes the old session and waits for owned output shutdown before a
new start allocates another session. Recorded Restart creates fresh game/audio
owners and reloads persisted progress. Companion fetches are bounded, cancelled
on failure/Stop, and checked against exported SHA-256 values before use.

The maintainer helper builds both companions; embedding remains optional and a
missing required companion gives an explicit export error. Recorded exports
include runtime sources, converter, optional decoder and integrity metadata.
Their compiled guest memory maximum is reduced by the declared audio shares.
Additional files use create-new writes like existing export files. Source-built
recorded-audio exports are now enabled; this supersedes the earlier disabled
export gates. Existing released binaries are unchanged and no release was made.

Actual signed dependency-free core/component fixtures declare 4 MiB effects and
8 MiB music inside 16 MiB, with exported guest maxima verified as 64 Wasm pages.
Both players reach natural MP3 completion, restart with a fresh worker, and
complete again after Stop/relaunch. Forced progress-checkpoint failure starts no
decoder; storage recovery cannot replay the rejected request, while fresh input
plays a new generation. Corrupt converter bytes are refused before guest worker
creation; restoring them permits fresh playback. Test observation reads existing
tick snapshots, preserving the production one-request safety guard.

Legacy core/component delayed startup/checkpoint Stop/Restart tests pass. All
256 enabled host library tests pass (19 explicitly ignored). Signed desktop
native/core/component music and effect SDK suites pass with successful new web
exports. Optional decoder/converter build-input reset/bounds tests and strict
all-feature Clippy pass. No existing deadline, resource cap, reference hash or
comparison tolerance was increased. Caller-owned assets and browser/JIT/GC
memory remain outside the explicit guest/audio backing ledger. Recorded port
migration, guest-memory rewind, and broader platform/device qualification remain
unfinished; this does not establish complete port fidelity.

The integrated player also retains failed audio shutdown as a startup barrier:
it does not allocate another session alongside an output whose context failed to
close. A signed-game browser test injects that failure in both profiles, verifies
no new guest worker is created, then explicitly cleans up the test context.
Corrupt-companion startup failures with zero retained ownership remain recoverable.
All final recorded-player checks pass with the one-request guard intact.

### Arkanoo recorded-backend migration inputs

The current port still mixes its five 48 kHz effects in guest ticks. Migration
must use original 44.1 kHz input before shared mixing/conversion, rather than
resample those already converted files back and compound conversion error.
The maintainer effect checker can now create an explicit new private output of
exact SDL-decoded original-rate inputs, verify pinned original hashes and credit
source, and optionally prepare the sorted original MP3 playlist. It records
source/oracle/PCM/WAV hashes without changing existing reference fixtures.

The private preparation passes signed-package exact-PCM loading and cleanup for
all five clips: 2,029,392 PCM bytes fit inside the existing 4 MiB effect share.
Nine music files and the playlist are retained only in the explicit private
output. No recording, signed package or binary release is published. The live
port is unchanged while its event/mute/pause/settings/playlist controller and
actual native/browser migration checks are prepared. Source preparation is a
maintainer operation; players require no Python, SDL tools or external decoder.

### Port playlist controller qualification

A dependency-free playlist controller is prepared in the port source, still
inactive in the game entry point. It uses only virtual asset reads and music
controls; it preserves sorted/random selection, startup gain, mute/pause,
completion, stop and failure retry without decoding in the guest. A staged-Play
flag prevents an old tick snapshot's completion from advancing a just-requested
replacement twice. Inactive SDL resume is represented as a no-op so it cannot
create an invalid GamePKG pause batch.

The maintainer tool runs the unchanged hash-checked original audio.rs and the
port controller against the same operation sequence and random draws. All 32
observed output/gain/mute/pause/track states match. Four unit cases also verify
missing/empty playlists, UTF-8 split across reads, sorting, portable path rejection,
size bounds, gain clamps and staged completion behavior. Those cases pass on
actual Linux32 as well as Linux64. A private synthetic signed recipe passes the
real source policy and isolated native/Wasm builds; strict lint passes. Existing
original references and tolerances are unchanged.

Full activation remains blocked by work to do, not by a relaxed fixture: the
largest original MP3 is 3,841,536 bytes, so the browser's full transferred encoded
copy plus its 6 MiB decoder exceeds the proposed 8 MiB music child. All nine
MP3s total 21,118,219 bytes alongside 24,416,401 existing asset bytes, exceeding
the current 32 MiB web export budget. Bounded/chunked loading and export residency
must be resolved without dropping tracks or raising existing limits. The working
port remains on its existing audio path; persisted music controls and full
native/browser recorded-backend migration still need wiring and proof. Only
controller/source tests are published; all migration recordings remain private.


### Chunked browser decoder input qualification

The full encoded transfer snapshot described above has been replaced by one
sequential, owned transfer of at most 64 KiB at a time into reserved decoder
memory. The worker releases each message's input reference before acknowledging
it. The client reserves linear memory, two module snapshots, one fixed 64 KiB
transfer and three bounded PCM blocks; caller-owned package assets and browser
compiler/JIT/GC overhead remain separate. Reserve, all input transfers and codec
open share one absolute 100 ms deadline, rather than resetting it per chunk.

Actual Chromium decodes all nine complete original tracks within one 8 MiB
music allowance, with unchanged native PCM hashes and sample counts. The largest
track uses 59 transfers, none larger than 65,536 bytes. Tests inspect exact-span
ownership and sequential offsets, reject incomplete/out-of-order/oversized/shared
input, and force delayed transfers to verify the aggregate deadline and cleanup.
The complete native/JavaScript/browser qualification finishes in 12.52 seconds
under the existing 30-second browser observation and 60-second tool timeout.
No runtime caps, reference hashes or tolerances were increased.

This resolves the decoder transfer-memory barrier, not the full port migration.
The complete music plus existing assets still exceed the 32 MiB browser asset
budget. Asset residency, persisted controls and full native/browser port playback
remain required; recordings remain private.

After rebuilding both desktop entries with the embedded companions, the actual
signed core/component exported-player regression also passes with chunked input:
music completion, fresh Restart, Stop/relaunch, checkpoint failure/no replay,
companion integrity rejection/recovery and close-failure allocation protection.


### Lossless full-dataset asset residency audit

`tools/check_arkanoo_asset_residency.py` creates a new private output outside the
workspace, re-encodes PNGs with maintainer Pillow/optipng, verifies every decoded
RGBA byte and dimension, and records input/output hashes. It substitutes the
prepared original-rate effects, verifies prepared recording hashes, includes all
nine original music tracks and the playlist, and accounts for the exporter's
additional precise-image RGBA snapshots. It never rewrites published assets.

All 76 PNGs retain exact pixels. Optimized assets with replacement effects use
12,945,752 bytes; music uses 21,118,219 and the playlist 330. Precise-image data adds
1,075,272, for 35,139,573 accounted bytes against the unchanged 33,554,432-byte
limit. Lossless compression alone therefore remains 1,585,141 bytes over budget;
this audit does not claim a successful full export or port migration. No images,
tracks or fidelity data are removed. Generated assets and reports remain private.
Players need none of the maintainer tools.


### Prepared portable music setting persistence

Arkanoo source recipe 0.1.62 prepares ARK4 save support for separate music/effect
volume and mute settings. The fixed little-endian header keeps the existing save
size and full pattern capacity. ARK1/2/3 remain readable with music volume 64 and
unmuted defaults. Unknown bits, either gain above 128, invalid patterns and
truncation reject before changing stored settings or patterns. Encoders clamp
caller gains to 0..128. Unicode pattern names and complete archives round-trip.

Archive encoding reserves its complete byte length before writing, preventing
the final pattern cells from doubling a near-limit buffer. The maximum-size
archive regression checks both length and allocation capacity at 1,048,576
bytes on actual Linux32 and Linux64; all 101 port unit cases also pass after
this change. This removes one allocation spike. It does not yet establish that
maximum archives, editor backups and dense scenes fit together with recorded
audio in every guest runtime; that combined-memory qualification remains open.

The private full-port qualification now installs a synthetic maximum-size ARK4
archive, restarts the actual guest, changes the music setting and checks that
all pattern bytes survive the resulting save. Desktop core and component Wasm
pass with the original recordings and unchanged audio shares. Native diagnostics
identified a failed 1,048,352-byte name allocation during decoding. The owned
single-pattern decoder now validates before mutation and reuses the input
allocation for that name. A pointer-identity regression and malformed UTF-8
rejection pass on actual Linux32 and Linux64. The later settings-change failure
was eliminated by lending that name buffer to synchronous archive encoding and
restoring it before save commit or rollback. The complete maximum archive now
loads, changes music settings and preserves every pattern byte in native, core
and component desktop runtimes, with all original audio inputs and unchanged
16 MiB combined limits. Pointer-identity tests verify reuse in both directions;
zero/multiple-pattern encoding and failed editor transactions retain their bytes.
The runner checks all three variants before reporting failures, so a native
failure cannot hide the component result. Browser maximum-state coverage,
editor backups and dense scenes remain open; the single-pattern regression
does not prove those broader cases.

The full private runtime regression additionally constructs an exact 1 MiB
archive with 5,041 unique patterns. Core and component desktop runtimes preserve
every pattern through a music-settings save. Native startup initially exhausted
memory while holding the raw archive beside its pattern table. The game now
reads directly into final name/cell allocations through a reusable 4 KiB cache,
and decoding reserves the exact validated pattern count. The smaller cache also
avoids a failed 65,537-byte native RPC response allocation without exceeding the
unchanged call budget. Saving now streams the archive through bounded host-owned
chunks rather than allocating another full archive in the guest. Native, core
and component desktop runtimes all pass loading and a music-settings save of
the exact 5,041-pattern archive, preserving all pattern bytes. Both full private
browser profiles also pass normal music/effect gameplay, settings saves and
restart with this API. Browser maximum archives and full editor backup/edit
transactions still require their own qualification.

The host retains the previous save until every ordered chunk arrives. Chunks
are limited to 64 KiB and the replacement remains limited to 1 MiB; incomplete
sequences are discarded before the next tick. Disk publication still uses the
existing successful-tick transaction. Host tests cover gaps, abandoned writes,
oversize chunks, integer overflow and unchanged disk data; end-to-end runtime
tests cover traps after completed chunk writes, incomplete multi-tick writes and
permission denial. Browser tests cover bounds/order, canonical bindings and the
unchanged shared 1 MiB per-tick save-transfer budget.

The maximum 5,041-pattern runtime case also opens the editor and browser, loads
the first pattern, erases one cell and stores it over the original. Native,
core and component runtimes preserve the complete archive with exactly that
one byte changed. Editor rollback now records changed cells and insertion
indices instead of cloning the complete collection. Replaying records in
reverse restores indices after interleaved insertions; repeated edits, failed
transactions and original name allocation identities pass standalone Linux32
and Linux64 tests. The working board remains available after a failed save.

Native editor entry exposed an additional 31,868-byte allocation failure while
retaining both a full static label raster and its tile. Label decoding now
produces only the requested tile directly from the original RLE data. Existing
256-column boundaries, one-column sampling overlap and draw order are retained.
Every tile row matches the unchanged original SDL label fixtures exactly, and
all 101 port unit tests pass. This establishes the tested small-name full
collection edit; extremely long active names, browser maximum-state execution,
failure injection in the complete guest and dense scenes remain open.

The game's streaming reader/writer now lives in `save_io.rs`, compiled directly
by standalone tests with bounded host fixtures. The loader agrees with the
original codec for ARK1/2/3/4, every truncation and single-byte mutation of the
Unicode multi-pattern fixtures, trailing data and oversized input. Invalid input
never invokes a write. Maximum single-name archives use exactly 256 reads;
failed and short reads at the first, second, middle and final chunks stop
immediately. Corresponding failed writes leave the previous blob unchanged,
and successful streaming output matches the existing ARK3/ARK4 encoders exactly.
These checks pass on actual Linux32 and Linux64. The refactored full private
port also passes native/core/component playback, maximum saves and collection
editing; complete-guest fault injection and browser maximum states remain open.

### Maximum browser archive and collection editing qualification

Both core and component browser exports now load exact 1 MiB archives for two
shapes: one near-limit identifier and 5,041 distinct short-name patterns. The
maintainer test supplies the initial fixture once through the normal progress
load boundary, changes the music setting through real UI input, waits for the
real persistent commit and compares every pattern byte. It then opens a fresh
playback session without the fixture override and verifies byte-for-byte reload
from the actual progress store. The collection case additionally opens the
editor/browser, loads the first pattern and erases/stores one cell; exactly that
cell byte changes, and the edited archive survives the fresh-session reload.

All original effects/music remain enabled. The existing 16 MiB parent partition,
3 MiB effects share, 7 MiB music share, two 1,200,224-byte asset workspaces and
3,891,008-byte guest allowance are unchanged. Every stopped session releases its
parent reservations before the next begins. No renderer, frame deadline, save,
transfer or observation limits were increased. The browser input trace caught
an initial test click half a pixel outside the grid; the final test uses the
interior of the same first cell. This establishes the two tested archive shapes
and short-name collection editing, not extremely long active names, complete
guest failure recovery or dense-scene parity.

The explicit `check_arkanoo_recorded_port.py --long-names` qualification now
opens the full-size single identifier in the pattern browser, then attempts
loading/storing it in the editor without altering its archive bytes. This is
currently failing in native, core and component desktop runtimes. Core and
component fail while entering the browser (logical L): `unicode_layout` grows
the complete shaped-glyph vector through `unicode_append`, requesting
7,143,424 bytes against the unchanged 6 MiB guest allowance. Native stops at the
same action. This is separate from save serialization, which already passes.
The required fix must preserve complete names and shaping/sampling semantics
while avoiding full offscreen glyph retention; input truncation and larger
memory ceilings do not close this requirement. The opt-in gate reports each
runtime failure and remains an outstanding qualification, alongside the normal
maximum-archive/short-name editing checks.

Printable ASCII labels now derive a conservative visible-prefix bound from the
pinned font's minimum pair advance, negative bearings and maximum three-input
ligatures. The bound includes additional offscreen glyphs for shaping lookahead;
stored identifiers remain complete. The fast path is used only for left-aligned
viewport labels after verifying positive advances and a fixed vertical envelope.
Centered and non-ASCII labels retain the existing full shaping path. Editor
prefix/name/suffix assembly uses the same bound, avoiding a complete temporary
copy solely for a clipped label.

Visible glyph pixels match full shaping for long kerning/ligature/punctuation
cases on Linux32 and Linux64, and all 102 port unit tests pass. The long-name
qualification now passes browser entry, active loading and storing in both
desktop Wasm profiles within their unchanged memory allowance. Native passes
browser entry but still stops on Enter while loading the full identifier into
the working editor; that separate failure remains to diagnose and fix. Broad
Unicode maximum-name support and browser-export long active-name qualification
are still open. No arbitrary text-length limit or source-policy exception was
introduced.

The remaining native long-name load is now fixed by sharing identifiers between
the archive and working editor. Short identifiers use a compact shared string;
large identifiers retain and share their original String allocation. The name
value occupies no more space than the former String field on actual Linux32 and
Linux64. This avoids both a near-1 MiB copy on editor load and per-short-name
wrapper overhead that would otherwise regress maximum collections. Explicit
name edits copy on write, preserving the archived identifier and its allocation
until the user stores a replacement. Unicode backspace/input, rollback, exact
save bytes and streaming fault tests retain their expected values.

Native and both desktop Wasm profiles now pass `--long-names` alongside the
5,041-pattern collection load/edit/save cases. Both browser profiles also open
the near-limit ASCII identifier in the pattern browser and working editor,
store it intact, and reload it byte-for-byte from persistent storage. The full
collection edit/reload and all parent reservation release checks still pass.
All 110 port/codec tests pass; the eight storage/streaming cases also execute on
actual Linux32. No memory ceiling, source restriction, save format or archive
name length was changed. Maximum-length typing that requires a distinct working
copy, broad Unicode maximum-name shaping and dense-scene fidelity remain open.

The separate `check_arkanoo_recorded_port.py --typing` probe now loads the full
ASCII identifier, enters name editing, removes its final character, types a
different character, then restores the original character and stores the name.
Both desktop Wasm profiles complete this input sequence and preserve the final
archive bytes. Native stops on the first logical Backspace (code 8), where the
shared large-name path requires copy-on-write. This remains a failing native
resource regression; it does not invalidate the passing select/load/store case.
The probe currently observes runtime survival and final archive equality, rather
than independently reading intermediate working-name contents, so it is not a
complete proof of maximum-length text editing. No memory ceilings were raised
and no input was truncated or silently denied to make this probe pass.

The native tail-edit failure is now fixed with segmented identifier storage.
Deletion adjusts a UTF-8 boundary in the shared prefix. A replacement retains
that prefix and stores only its changed suffix; shared edited names copy their
suffix metadata when necessary, preserving previously stored values. Name
comparison, archive serialization, ASCII export and filename sanitization read
the segments in order. Rendering joins only the font-derived visible prefix
for ASCII, while ordinary single-segment labels use borrowed slices. Unicode
fallback labels no longer gain an unnecessary full-size viewport copy.

The full `--typing` input sequence now completes in native, core and component
desktop runtimes within the unchanged shares. Both browser profiles perform
the same deletion/replacement/restoration through real keyboard input, commit
the original bytes and reload them intact from persistent storage. Maximum
collections still load, edit and save correctly. Independent tests check the
intermediate UTF-8 suffix contents, retained prefix pointer, stored-value
isolation, lexical ordering and exact equivalence to flat-string save bytes.
All 112 port/storage tests pass; ten storage/streaming cases and the borrowed
viewport pixel test also pass on actual Linux32. No archive format, memory
ceiling, source restriction or text-length limit changed. Broad maximum-name
Unicode shaping, larger edit workloads, complete guest failure recovery and
dense-scene parity remain open; these results cover the tested tail-edit case.

The broader production integration suite (`tests/arkanoo.py --runtime-only`)
also passes after the streaming save, segmented identifier and viewport changes.
All 71 signed runtime checks pass before native builds and again after the
native builds are installed. Coverage includes original SDL rendering fixtures,
dense backgrounds/boards/particles, editor and filename workflows, save
continuity/corruption/denial, controls, recorded effects and window commands.
Reference hashes, pixel tolerances, scene counts, deadlines and test parallelism
remain unchanged. Both CLI and GUI release entries rebuild with all features
and both embedded audio companions afterward.

This regression run uses the active port's existing guest-effects path and its
normal memory allocation. It does not close dense-scene qualification of the
private complete recorded-music candidate with its smaller guest allowance.
The runtime-only run also omits interactive desktop file-picker/window QA;
those workflows require their own live checks.

### Dense campaign scenes with complete recorded audio

`check_arkanoo_recorded_dense.py` now compares six controlled campaign states
with 200 blocks and 3,000 particles against the active source port. The probe
uses the actual initialized Game and production rendering code; only scene
state and a direct original-effect request are supplied by the private fixture.
The complete candidate retains the fixed 16 MiB parent, 3 MiB effects and 7 MiB
music shares, leaving the existing 6 MiB desktop guest allowance. Original music
is initially audible and remains Playing throughout every scene; original
effect commands reach the real shared playback consumer and finish naturally
between cases, without enlarging the four-voice limit.

All six states pass in native, core and component desktop runtimes. Each matches
all 234 ordered draw commands, coordinates, colors, clips and image bytes exactly
against the active port, using 7,896,124 bytes of the unchanged 8 MiB frame image
budget. No scene count, pixel tolerance, memory ceiling or deadline was raised.
The reference port's original rendering is separately covered by the passing
71-check production suite. These comparisons qualify the six controlled dense
states with audio; they do not establish every animated/background state,
browser dense-scene execution or interactive platform parity. The maximum
archive combination is qualified below. CI runs the private comparison before
full-browser qualification, and publishes none of its generated packages or media.

The standalone codec cases execute on actual Linux32 and Linux64. All 101
existing port unit cases, the two new codec cases, strict lint and approved-source
isolated native/Wasm compilation pass. The synthetic source-build check now
compiles the actual editor codec alongside the prepared playlist controller.
The active game still writes ARK3 and uses its existing effects path; ARK4 writing
and music controls are prepared for the complete migration, not enabled as a
partial port. No recordings or private proofs are published.


### Full private recorded-audio port qualification

`tools/prepare_arkanoo_recorded_port.py` now prepares the complete private port
candidate, including all five hash-checked original-rate effects, all nine
hash-checked music tracks and the exact playlist. New private outputs must be
outside the workspace. The source switches effects to host-owned playback,
adds the original music toggle/slider geometry, persists separate ARK4 settings,
preserves controls across game resets and uses the qualified playlist manager.
The published active port is not silently switched or given missing recordings.

The first full source check caught a conservative source-policy collision:
`text` in the playlist parser matched the renderer's function name. Renaming that
local to `playlist_text` makes the full package pass without relaxing policy.
All 32 original controller observations/random draws still match. Source recipe
0.1.63 contains this rename; gameplay remains on its previous active audio path.

A 4 MiB guest partition failed native startup. The private candidate now assigns
3 MiB effects, 7 MiB music and 6 MiB guest, preserving the original 16 MiB hard
ceiling. Its full native and Wasm sources compile under the normal policy.
`tools/check_arkanoo_recorded_port.py` uses separate temporary signed libraries
for core/component profiles, retaining normal update/version checks. Actual
native, core and component executions produce audible original music, freeze
music progression when muted, accept original menu slider gestures, preserve
volume 32/muted in ARK4 across restart, and resume playback from the UI. Launched
gameplay commits an original collision recording through the shared host mixer
while music is paused; guest PCM remains empty. Strict lint passes.

These are real private full-port controls/output tests, not yet complete gameplay
or pixel parity, all-track in-game selection, platform coverage or a successful
browser export. The previous full-dataset browser size barrier remains. Private
recordings, generated recipes, signed packages and binaries are not published;
only preparation and qualification source is published. Players need none of
the maintainer Python/compiler/test tools.


### Exact browser PNG decoding without duplicate raw assets

Source-built exports now select validated noninterlaced 8-bit RGB, RGBA and
grayscale with alpha, plus 1/2/4/8-bit grayscale and indexed PNGs, for a trusted
exact worker decoder. Package PNG bytes remain available to guest asset reads.
Image preparation decodes them directly to the
existing image cache, avoiding separately exported raw RGBA copies. Unsupported
representations retain the previous precise-pixel fallback when needed.

The decoder checks PNG signature, CRCs, chunk order, palette/transparency,
dimensions, scanline size and filter values. The dependency-free zlib/DEFLATE
inflater copies at most 2 MiB of compressed data, uses two fixed 32,768-entry
Huffman scratch tables and allocates the exact bounded scanline capacity before
decoding. Stored blocks, literal writes and overlapping back references check
capacity before every write; no opaque browser decompressor allocates output
before its size can be checked. Zlib headers/window bounds, reserved codes,
back references, end position and Adler checksums are checked. Existing 512-pixel
dimensions, encoded-image/image-cache/draw limits and worker deadlines remain.
These named buffers do not establish a whole-browser/JIT/GC memory bound.

`tools/check_browser_png.py --browser` independently decodes all 76 current
source images with Pillow as a reference, then verifies exact RGBA hashes in
Node and an actual classic Chromium worker. The same check passes for all 76
private losslessly optimized PNGs, including packed palettes and grayscale.
Synthetic cases cover all five filters, RGB/RGBA/grayscale/gray-alpha expansion,
partial packed rows, split input, exact palette and color-key transparency,
invalid chunks/CRCs/palettes/dimensions/filters and output overflow. Sixty-four
independent zlib encodings cover stored/fixed/dynamic/RLE behavior; a 4 MiB
expansion with a 16-byte allowance fails before exceeding the allowance.

Actual signed core/component exports emit exact indexed, packed-palette,
grayscale, gray-alpha, RGB color-key and RGBA draw-command pixels without
duplicate raw assets. A valid 16-bit RGBA case retains its precise host-pixel
fallback. Hidden RGB at zero alpha is preserved. Corrupt PNG CRCs fail explicitly and
restored exports recover freshly. Existing browser export gameplay/storage/media/
shader/shutdown checks, signed recorded-audio player lifecycle, host exporter
checks and strict lint pass. CI gates the corpus and signed-player tests.

The optimized-corpus CI check exposed an omitted representation: optipng
converts the ball to 8-bit grayscale with a transparent color key. The grayscale
path compares that key before expanding packed samples; it retains exact RGB
even for fully transparent pixels. Both Chromium 155 and 156 workers preserve
all 76 independent image hashes. Full signed core/component exports using this
optimized corpus retain all 76 images, five effects and nine tracks, with no
raw image duplicates and no memory, export-size or execution-limit changes.

The revised complete private dataset audit reports 12,945,752 optimized asset
bytes with original-rate replacement effects, 21,118,219 music bytes and a
330-byte playlist, with zero extra precise-image bytes. Total 34,064,301 remains
509,869 bytes over the unchanged 33,554,432 export limit. A slower private optipng
pass saved only another 7,084 bytes, so neither it nor this decoder establishes
a successful complete browser export. Full gameplay/browser/platform migration
remains open. No images, recordings, generated recipes or binary artifacts are
published by this change.


### Full original browser dataset fits with bounded lossless WAV storage

The complete private recorded-audio candidate now exports successfully for both
core and component profiles. It retains all 76 pixel-identical images, five
original-rate effects and nine unchanged MP3 tracks. Actual host-encoded asset
payloads total **33,185,755 bytes**, below the unchanged **33,554,432-byte** limit.
The previous size barrier is resolved for this dataset; no assets or tracks were
removed and no existing resource limits were raised.

The exporter optionally zlib-encodes canonical effect WAVs between 128 KiB and
the bounded inflater capacity, only when storage shrinks and the existing guest
minimum still fits after workspace reservations. Other assets retain their raw
representation. Package/source files and ordinary asset-read bytes are unchanged.
A trusted package map lazily allocates one reusable decode workspace per side,
retains only the most recent decoded asset and forbids overlapping borrows.
Fixed Huffman and metadata tables remain in that workspace. Original WAV loading
copies PCM into its already-reserved output slot; it does not retain a second
complete decoded collection of all effect files.

The session reserves both the main-side and worker-side workspaces before use.
For this dataset each is 1,200,224 bytes: the maximum 937,056-byte WAV plus a
263,168-byte scratch allowance. The 16 MiB parent still contains 3 MiB effects,
7 MiB music and the remainder for guest plus both workspaces. The browser guest
allowance is therefore 3,891,008 bytes. Actual signed fixture exports also verify
the binary Wasm memory maximum is reduced accordingly. Shutdown terminates the
guest before releasing its lease and retains the main workspace until every
asynchronous borrower has settled. A forced Stop during admission proves that
capacity remains charged until the borrower returns, then is released; fresh
playback recovers. Static package inputs and whole-browser/JIT/GC accounting
remain separate as previously documented.

Full browser qualification initially exposed a real renderer bottleneck:
consecutive filtered images copied the complete framebuffer for each sprite.
Filtered runs now share the framebuffer, preserving draw order and per-draw
RGBA8 quantization, and flush before other drawing or clip changes. Ten actual
Chromium comparisons match the hash-pinned previous renderer byte-for-byte,
including mixed drawing, fractional positions, clips and transparent background
requests. A 235-image run now requires one framebuffer copy instead of 235.
No logical game timing, runtime deadline or observation timeout was increased.
Pointer capture is also skipped while pointer lock already owns the surface;
platform rejection of ordinary pointer capture does not discard the input event.

`tools/check_arkanoo_recorded_web.py` verifies both complete browser exports and
actual original music, music mute clock freeze, durable slider selection,
compressed collision effects, normal Escape-to-pause, Restart with restored
music settings, resume and parent-capacity release. `tests/browser_compressed_assets.py`
independently verifies full original WAV reads through both SDK forms, actual
playback, exported memory maxima, pending-loan shutdown retention and fresh
recovery. Node storage/decoder tests, 18 existing JavaScript checks plus the new store check, production
worklet/transport/dispatcher/session regressions, 258 enabled host library tests
(19 resource/platform cases remain explicitly ignored), and strict Rust 1.89/1.99
checks pass. CI includes the bounded-store, signed compressed-asset, renderer
batching and full original-dataset preparation/playback checks. All generated recipes, images, recordings, packages and binaries
remain private; only implementation and qualification source is published.

This qualifies the private full-audio migration on the tested desktop/browser
paths. Public active-port migration, complete original menu/pixel/gameplay parity,
other platforms and whole-host memory accounting still require work. The active
public recipe and released app binaries are not represented as already migrated.

### Maximum archives with recorded audio and dense desktop scenes

The final packed-pattern candidate passes `check_arkanoo_recorded_dense.py
--full-save` in native, core and component desktop runtimes. An exact 1 MiB
ARK4 save containing 5,041 patterns remains loaded throughout all six dense
states; the probe verifies the pattern and particle counts as well as exact
ordered rendering commands and image bytes. Audio shares, guest limits, voice
counts, frame budgets and deadlines are unchanged. CI now runs this combined
case rather than only the empty-archive variant.

Patterns retain all original cell values in lossless nibble storage and expand
to the same 200 wire bytes when saved. Exhaustive valid cell-pair tests and
every invalid cell value verify decoding, atomic rejection and unchanged wire
bytes. Native image calls send the existing header and borrowed pixels directly
under one output lock, avoiding an extra full-packet allocation without changing
the protocol or its bounds.

The final candidate also passes actual Chromium core and component gameplay,
original recorded music/effects, mute and restart checks, and both maximum-size
archive shapes. Real editor actions commit byte-exact saves and fresh sessions
reload them from persistent storage. All 76 images, five effects and nine tracks
remain present in the 33,185,755-byte export. These browser checks do not yet
combine the dense-scene fixture with the maximum archive.

The full `cargo test --all-features` run completes successfully, including the
259 passing host unit tests and the standard integration suites. Fixture-bound
ignored tests retain their separate qualification requirements; a standard
green run does not establish those cases or cross-platform completion.

### Dense browser scenes with maximum archives and recorded audio

`check_arkanoo_recorded_dense_web.py` uses the same private scene initializer
as the desktop comparison and the production renderer unchanged. Actual
Chromium core and component exports each pass all six scenes with 200 blocks,
3,000 particles and the full 5,041-pattern save loaded. Returned state confirms
the actual pattern and particle counts. Every ordered command, geometry value,
color and image-byte SHA-256 equals the active-port browser reference. Each
scene retains 234 commands and 7,896,124 image bytes.

Recorded music remains Playing, an original effect reaches the real browser
audio session, and stopping releases every tracked parent budget. Guest
accounting remains 3,891,008 bytes within the unchanged 16 MiB parent, 3/7 MiB
audio shares and two 1,200,224-byte asset workspaces. No execution deadline,
image cap, voice limit or comparison tolerance changed. Test observations and
Chrome's own allocations remain outside that named guest/audio accounting.

CI now includes this private comparison after the existing full-browser
editor/persistence check. These checks qualify six controlled scene states,
not all animation sequences, broad maximum-length Unicode shaping or
interactive platform parity. No generated package, media or real key is
published. The all-feature optimized CLI and GUI rebuild also passes.

### Unicode cluster lookahead and reproduced maximum-name failure

Unicode shaping no longer allocates a duplicate scalar vector for each
combining cluster. Iterator lookahead detects variation selectors before
normalization, preserving the previous append/composition/substitution order.
The pinned pre-change eager shaper in
`tests/fixtures/arkanoo_unicode_shape_eager.rs` independently checks exact output
for 576 mixed base/mark/variation combinations, three 32,768-mark clusters and
empty/NUL cases. All 103 port unit tests and strict all-target/all-feature lint
pass, including the existing independently recorded SDL text rasters.

The full private runtime probe now accepts `--unicode-name SAMPLE`, retains a
valid full 1 MiB archive with a final ASCII character for the existing edit/
restore sequence, and forces the long-name editor/browser workflow. The
reproduction with `--typing --unicode-name Aé` fails while opening the pattern
browser: core and component attempt growth to 7,143,424 bytes beyond the fixed
6 MiB guest share; native exceeds its existing deadline. This is an open
production limitation, not a passing qualification. The duplicate-cluster
removal alone does not solve the full-name point vector and full-name shaping
cost. No name is truncated, no rendering/input cap or deadline is increased,
and the failing runtime check remains available for the next correction.

### Maximum Unicode base-name correction

The previously failing full-size `Aé` name now passes the complete private
native, core and component port probe with `--typing --unicode-name Aé`. Actual
Chromium core and component exports also pass the same full 1 MiB name through
editor selection, character replacement/restoration, byte-exact commit and
persistent reload. Recorded music/effects, mute, restart and shutdown checks
remain enabled; the original memory shares and execution deadlines are unchanged.

The viewport path verifies independent single-glyph normalization against the
pinned font's complete cmap, decomposition and space-fallback scalar tables.
It derives a conservative advance from every possible used-glyph ligature and
kerning pair, checks bearings and the vertical envelope, then copies only a
UTF-8-complete visible prefix with shaping lookahead. The complete original name
remains in the archive. No arbitrary name length or glyph-count limit is added.
Centered text and unproven cases retain the existing complete shaping.

Exact visible pixels match full layout for mixed accented Latin, Greek,
Cyrillic, missing-glyph text, kerning and ligatures, including segmented names.
An additional table-driven test checks each scalar against the pinned eager
shaper and compares every visible raster tile for each optimized scalar-table
entry. All 105 port unit tests pass. CI adds the full Unicode desktop and
browser workflows alongside existing ASCII checks. Maximum-length combining
clusters, variation sequences and glyphs outside the proven vertical envelope
remain open; passing base-name checks do not establish those cases.

The same four focused ASCII/Unicode normalization and viewport tests also pass
as an actual i686 Linux executable against the final source. The exhaustive
viewport test exercises 2,705 optimized scalar-table entries and compares all
five visible raster tiles for each. Strict all-target/all-feature lint,
formatting, CI YAML, private-probe syntax and parity TOML checks pass. This
32-bit evidence covers the tested text path, not a complete 32-bit app release.

### Decomposed-accent viewport correction

The full-size `A` plus U+0301 name first reproduced the earlier memory failure
in both Wasm profiles and stopped the native worker while opening the browser.
The corrected complete candidate passes `--typing --unicode-name` with that
decomposed sample in native, core and component desktop runtimes. Actual
Chromium core and component exports also pass editor save, exact-byte
persistent reload, recorded-audio controls and parent-budget release. Original
UTF-8 name bytes are retained; no normalization is written back to the archive.

The viewport scanner retains one current composed scalar rather than a full
name-point vector. It accepts only nondecreasing combining classes whose exact
font normalization consumes the complete cluster into one approved nominal
glyph. It verifies each composition against the existing complete shaper,
tracks the largest proven cluster, and ends any visible prefix at a complete
cluster boundary, including boundaries split across name segments. Ligature,
kerning, bearing and vertical-envelope proofs remain required.

All 107 port unit tests pass. The composition test traverses every chain in the
pinned 941-rule composition graph (maximum three successive compositions),
compares normalization with the pinned eager implementation and checks exact
visible tiles for every optimized case: 365 pairs/chains. Strict lint passes.
CI adds decomposed-name desktop and browser cases alongside the existing ASCII
and composed-scalar cases. Unconsumed marks, reordered classes, variation
selectors and other unproven clusters retain full shaping and remain open. No
guest memory, time, input, drawing or audio limits have been increased.

All six focused checks also pass as an actual i686 Linux executable against
the final source, including 365 optimized composition pairs/chains and 2,705
scalar-table viewport cases. This verifies the tested text path on Linux32;
Windows32, Android and broader interactive parity still require their own
qualification.

### Variation-selector viewport and loading-recovery corrections

The full-size repeated `A` plus U+FE0F name reproduced the previous memory
failure in core/component and stopped native playback. The corrected private
port passes desktop native/core/component edit/restore and both real Chromium
profiles' editor save and persistent reload. Full original UTF-8 bytes remain
in saves. A selector is admitted to the viewport proof only when the pinned
shaper emits an unflagged glyph with zero bearings, dimensions and advance.
Only trailing selectors after a proven base/composed cluster are accepted;
marks following a selector retain complete shaping because their normalization
can differ. Ligature/kerning lookahead and complete prefix cluster boundaries
remain enforced. All 108 port unit tests and strict lint pass.

The browser run also exposed a transport failure before reaching the name
fixture: a replacement decoder failed before sending begin while an older
track was terminal, leaving the committed replacement Loading indefinitely.
A deterministic Node MessageChannel regression reproduced it. The receiver
now processes the current replacement's failure despite older terminal state,
records its peer generation, and retains stale/late message rejection and
completed-track queued PCM. Six ended/failed/stopped versus loading/paused
replacement combinations, subsequent recovery, and queued-EOF preservation
pass. Actual Chromium production-worklet tests verify all six combinations,
existing independent SDL hashes, decoder supervision, dispatcher rollback,
combined playback and session shutdown. The rebuilt all-feature CLI/GUI and
complete selector-name browser workflow pass with unchanged limits.

CI adds the selector desktop/browser cases alongside prior ASCII, base Unicode
and decomposed-accent cases. Existing transport qualification automatically
runs the new Node and production-worklet regressions. Giant individual
clusters, unconsumed/reordered marks and other unproven text contexts remain
open; this does not establish universal maximum-length Unicode support.

The final seven focused tests also pass as a real i686 Linux executable,
including 1,295 optimized selector/context raster comparisons, 365 composition
pairs/chains and 2,705 scalar-table entries. Selectors that do not satisfy the
pinned-font zero-metric proof retain full shaping. This is 32-bit text-path
evidence, not completion of Windows32, Android or other platform qualification.

### Maximum single selector-run correction

The new `--selector-run` full-port fixture contains one `A`, 349,450 identical
U+FE0F selectors and final `x` padding in a complete 1 MiB ARK4 save. Before the
correction, both Wasm profiles attempted growth beyond their 6 MiB share and
native exceeded its existing deadline while opening the pattern browser.
The corrected complete private candidate passes native/core/component editing,
character replacement/restoration and unchanged save bytes. Both actual
Chromium profiles pass the corresponding seeded fixture, editor commit and
persistent reload, original audio controls and ordered budget release. The
browser probe explicitly verifies the full original selector-run fixture bytes.

Only the rendering representation compacts adjacent identical selectors,
retaining one selector per run when the immutable font proves unflagged zero
metrics and skippable lookahead. This retains raw normalization mode, the
class-zero barrier and final starter scalar, including contexts with later
marks, join controls, spacing fallback, leading selectors and ligatures. The
original name remains untouched in editor/archive storage. Two passes compute
exact display capacity without first cloning the large source text. The giant
run's five-byte display representation has capacity five.

Across 2,331 contexts, normalization matches the pinned eager shaper after
excluding only proven zero glyphs; complete dimensions and all five visible
raster tiles match exactly. Every UTF-8 segment boundary preserves compaction.
All 109 port unit tests and strict all-target/all-feature lint pass. CI adds
the single-run desktop/browser cases without removing prior ASCII, Unicode,
decomposed-accent or repeated-selector coverage. Unsupported/nonidentical
giant clusters and other unproven contexts remain open. No resource limits
or comparison tolerances change.

All eight focused text checks pass as an actual i686 Linux executable, including
2,331 compaction contexts, 1,295 selector contexts, 365 composition chains and
2,705 scalar-table cases. The broader signed production suite also passes all
71 checks before native fixture builds and all 71 again afterward. These cover
original complete editor/browser pixels, Unicode names and clipping, original
background/HUD/gameplay rendering, input, save corruption/denial and recording
semantics with existing references and budgets. The runtime-only run omits
interactive GUI workflows; other platform and unproven text requirements
remain separate.

### Original master retry and registration-state checks

`tests/cube2_master_retry.py` verifies the pinned upstream source set, extracts
unchanged master-state/output/parser functions plus the original heartbeat
condition, and runs them with deterministic clock and socket-call stubs. It
checks the zero-clock sentinel, pending connection timeout at 60 seconds,
no retry at either five-minute boundary and retry immediately afterward,
connected hourly registration, synchronous connection refusal scheduling,
fragmented `failreg`/`succreg` replies, custom reply forwarding, blocked and
partial sends, and disconnect buffer/address/callback cleanup. A `failreg`
reply logs failure but does not itself disconnect or invent immediate retries.

AddressSanitizer, UndefinedBehaviorSanitizer and LeakSanitizer all pass with
halt-on-error enabled. CI includes the regression in both feature matrices.
The production Rust network broker separately passes its actual loopback TCP
refusal/error-query and disabled-network test. The fragment harness records
network calls; it does not run an actual engine or contact a public master.
Full engine-to-master timed recovery and wider service/platform behavior remain
open. No original retry interval, network permission, quota or deadline changes.

### Long-session opaque network-handle correction

The new host regression first failed after 992 allocations even though every
socket had been closed. The old host token range was coupled to `fd_set`
indexing. The reviewed engine now uses fixed 16-entry virtual sets and the
host issues fresh positive 32-bit tokens without reusing closed identities.
No OS descriptor enters the engine. The wire integer representation and packet
limits are unchanged; 16 live handles, eight opens per second, call/transfer
quotas and all existing deadlines remain enforced. Token-space exhaustion at
the positive-int boundary fails closed without wrapping or reusing stale IDs.

All 14 host network tests pass. They include 1,280 mixed UDP/TCP/snapshot
replacements, stale-token denial, unchanged per-window open quotas, mixed
16-live-socket admission, accepted TCP at the maximum token, real data transfer
and clean representation-boundary failure. The guarded ENet shim passes
large/sparse-token polling, 16-entry and combined-set overflow, duplicate set
entries, alias preservation, malformed readiness bits and atomic poll failure.
Address/undefined/leak-sanitized set tests check forged counts, invalid tokens,
removal, repeated replacement and surrounding memory canaries.

Both the Rust player builder and maintainer builder compile the original
engine with the same shared header patch. Original source hashes are verified
and left untouched; repeated/changed overlay anchors fail closed. The native
cache fingerprint includes both the new adapter and patch description. The
original engine's private-master registration, cryptographic authentication,
wrong-key/unrelated-response rejection, successful retry and saved-key recovery
test passes with the new sets. These checks establish the tested Linux paths;
Windows/Android and full live master timing remain separate requirements.

The final adapter also passes a real 32-bit C executable ABI/behavior check
(32-bit pointers, 32-bit tokens, fixed 72-byte sets), the complete standard
`cargo test --all-features` suite, and a fresh original-engine private-master
registration/authentication/saved-key run. Both final engine build paths
complete. Fixture-bound ignored cases retain their separate requirements;
these results do not establish all port/platform parity.

Final all-feature CLI/GUI release builds pass. Strict all-target/all-feature
lint passes on Rust 1.89 and 1.99. The newer compiler's iterator lint required
two test-only `as_chunks` updates; the 365-case composition regression still
passes on the declared Rust 1.89 minimum without changing reference data or
case counts.


The original native demo record/retrieve/save/replay path is verified by
`cube2::session::demos::original_demo_recording_survives_snapshot_and_plays_after_restart`.
It records 120 gameplay ticks, validates the compressed original packet stream,
restarts with the scoped save, observes original demo playback and changing
rendered frames for 60 ticks, and checks byte-identical demo persistence. Run
this ignored fixture-bound regression with `GAMEPKG_CUBE2_DATA` pointing at the
original local data folder. It does not publish or modify that folder. Large
recording import/export and movie output remain separate requirements.

Custom demo paths are now verified too. The old save filter silently dropped
root-level recordings and uppercase `.DMO` files despite the original game
reporting successful receipt. Both host and engine adapters now preserve
case-insensitive demo extensions at validated relative paths inside the game's
isolated save root. Default, empty and custom `demodir` workflows pass actual
record/save/restart/rendered-replay tests. Existing traversal, link, depth,
visit, file/count and total-size limits remain enforced; the change grants no
ambient filesystem access. Rebuild the engine to use the fingerprinted adapter.

Original movie capture no longer steals game audio. The permanent host mixer
callback feeds both the host stereo ring and the original recorder; synchronized
removal fences the recorder's state before cleanup. A failing-then-passing real
engine regression verifies audio before, during and after recording. Shared
sample/wire tests and 1,000 concurrent callback registration/removal cycles pass
with address, undefined-behavior and leak checks enabled. Both build paths apply
the same verified, fingerprinted source overlay. Movie-file persistence is qualified separately below; large recording export
remains unfinished.


Finished original AVI recordings now persist within the unchanged scoped save
quotas. The full original-engine regression verifies 320x240 video, nonempty
frame output and 48 kHz stereo PCM through FFprobe and complete FFmpeg decoding,
then restores a fresh engine and checks identical movie bytes. These tools are
maintainer test dependencies only. Case-insensitive AVI paths retain the same
traversal/link/depth/visit/file/count/total protections as demos. Recordings above
the existing 16 MiB per-file or 32 MiB tree quotas, explicit import/export UI and
wider capture/platform coverage remain unfinished.

Native engine save writers now hold one kernel-backed lease per library, game,
creator and save schema. A second session or future recording import must wait
until the active game closes; it cannot later overwrite that session's state.
Validated committed snapshots remain readable for recording lists and exports.
Cross-process exclusion/release, hostile metadata-file types and the real
background worker's recovery/final-save behavior are verified. The signed CLI
recording test also lists and exports while a separate process owns the writer
lease. Bounded recording import uses this protection as described below.


Recording import is now available through `gamepkg recording-import GAME_ID
FILE` and the game-page **Import recording** picker. Close the game first.
Imports verify the installed creator/schema scope, validate demo gzip integrity,
version/protocol and bounded packet framing or AVI chunk/stream/frame structure,
then merge canonically without replacing names or losing existing preferences.
Legacy preference slots migrate into the same bounded archive. Files remain
limited to 16 MiB and the save tree to 32 MiB; decoded demo validation uses that
same 32 MiB budget. Three original demo record/import/restart/playback paths and
original AVI record/decode/import/restart pass against fresh signed libraries.
CLI tests cover active-writer, malformed, duplicate, path, size and count failures
without save changes. Real native Open/Save dialogs pass cancellation and exact
import/export. Large-recording streaming and wider platform coverage remain open.

Creator-scoped save readers also reject hard-linked current and legacy archives.
The check uses the opened no-follow descriptor, so another pathname cannot
substitute for the file being validated. A failing-then-passing regression covers
both formats; signed recording commands verify that listing, export and import
through linked archives fail without copying private bytes or changing targets.
Normal committed snapshots remain readable while the game's writer is active.

### Original main-menu version caption (recipe 0.1.64)

The main menu now restores `Version: 0.5.0`, the pinned original game's version,
beside its GitHub button. The GamePKG recipe has its own version. Original Roboto
width, gray color and alignment are retained. Forty independent full SDL menu
frames, including title/paused and Settings states, pass in signed Wasm and
native runtimes at the existing two-level rendering tolerance. Each execution
profile also passes all 71 runtime regressions without changing resource caps.

This does not complete the original settings menu. Resolution selection,
confirmation/revert and VSync still need host support; the recorded-music
candidate and broader browser/platform fidelity remain separate work.

### Scaled solid controls and glyph edges (unreleased recipe 0.1.73)

Solid editor and menu primitives now use the existing rectangle API. A one-texel
texture has different coverage at half-pixel edges, which shifted thin outlines
and low-volume slider fills at smaller window sizes. This follows SDL's
[scaled solid-primitive implementation](https://github.com/libsdl-org/SDL/blob/release-2.30.0/src/render/SDL_render.c#L2686).
Text tiles keep their internal seam clips, while their outer edges use the image
quad and world bounds. This retains the first visible pixel of a glyph when a
label begins at a half-pixel position.

The original photo-backed full-size menu comparisons still pass. A separate
uniform-backdrop fixture compares the complete controls directly against
original SDL at full and half size, without masking pixels or changing the
two-level tolerance. Native and Wasm desktop playback pass with Vulkan and
OpenGL; both browser profiles pass. The new fixture rejects the old renderer
with a maximum difference of 156. The corrected half-size desktop controls have
maximum differences of zero or one. Independent font, palette, pattern-browser,
editor-label and text-tile comparisons also pass. The pattern-browser fixture
uses the current pattern constructor, preserving its original names, empty
cells and selection states. Recorded-audio dense scenes and maximum saves pass
in native playback and both browser profiles with the existing memory shares.

This does not establish direct original-photo parity at arbitrary resolutions.
At half size, the original background still differs by up to 53 levels. An
independent SDL comparison confirms the cause: sampling the port's already
resampled 1280-by-720 background matches the port within one level, while
sampling the original 2816-by-1536 image directly does not. Resolving that
requires sampling original-resolution image data, or an equivalent lossless
representation, at the output resolution. The current memory and transfer
budgets are unchanged; the background difference is not counted as fixed.

### Canonical recorded-audio delivery, recipe 0.1.74

The canonical recipe now selects the previously qualified host recording path.
It includes all five original SDL-decoded 44.1 kHz effects and all nine original
MP3 music tracks. Its signed asset provenance records upstream hashes and the
76 losslessly optimized PNGs. Music retains the upstream-credited Pixabay
Content License separately from the code license; no standalone audio library
is offered. The 16 MiB hard limit is unchanged: effects reserve 3 MiB, music
7 MiB, leaving 6 MiB for the game. Both complete browser exports are 33,212,842
encoded asset bytes, below the existing 32 MiB cap, with no omitted images or
tracks and no duplicate raw images. Players need no Python or external decoder.

Actual canonical native/core/component checks pass music controls, mute clock
freeze, effects, saved preferences, restart and damaged-save preservation. Both
browser profiles pass maximum-size single-name and pattern-collection saves,
all six damaged-save cases, cancellation/memory release and all six dense scenes
with the full 1 MiB archive resident. The old 48 kHz SDL effect assets are frozen
independently in test fixtures. The legacy signed suite passes 72 checks each
in Wasm and native execution; original pixel, sample and geometry references
remain unchanged. Structural assertions now recognize the corrected solid
rectangle commands and identify text tiles by clip plus original geometry.

The deadline-sensitive legacy runtime cases run one at a time: concurrent
fixture copying and reference rasterization must not consume another game's
wall deadline on a two-core runner. Each complete 72-case suite has a 300-second
maintainer observation allowance; the individual game deadline remains 100 ms.
Particle counts, exact pixels, geometry and memory assertions are unchanged.

The longer canonical browser run caught a real intermittent upload failure:
59 sequential transfers of the largest MP3 could exhaust a shared 100 ms
wall-clock allowance before codec open. Upload now has one fixed five-second
aggregate bound and 100 ms per acknowledgement; actual codec open/read/rewind
still have their separate 100 ms execution limits. This supersedes the earlier
aggregate-upload description. A deterministic clock regression rejects the
previous client, accepts finite scheduling delays, rejects aggregate renewal
and per-operation stalls, and proves worker/budget cleanup. Actual Chromium
passes all nine full tracks with deliberately delayed input transfers, unchanged
116,310,370 native PCM sample values and memory charges. Infinite open/read,
malformed output, cancellation and fresh-worker recovery tests also pass.

This is an unreleased recipe change, with a dedicated canonical-delivery CI
gate. It does not establish complete port fidelity: noncanonical-resolution
original-photo sampling, decoder arithmetic differences from upstream SDL and
broader platform/gameplay qualification remain separate work.

The first canonical-delivery CI run exposed missing runner prerequisites rather
than an omitted media asset: its independent Rust/SDL reference tests need
`libsdl2-dev`. The dedicated menu job also built the app without either audio
companion, which the canonical recipe now requires. An explicitly incomplete
app reproduces the failure in a private display; the native error dialog reads
"this app was built without the audio decoder companion". The complete app
passes the same menu matrix. The menu job now builds both companions and checks
both relocated entries before pixel comparisons. The fixture selects its stated
native/Wasm target explicitly and captures private-display startup dialogs and
window titles on failure. The package, reference pixels and tolerances are not
reduced to bypass audio preparation. Its 2021-edition formatting check also uses
the project formatter rather than an explicitly selected 2024 style.

The corrected canonical recorded-port CI gate passes on source `61522b2`
([run 40506358814](https://github.com/centrexity/GamePKG/actions/runs/40506358814)).
The remaining menu gate has a browser state-transition timeout after desktop
comparisons pass. Private Rust 1.89/1.91 and exact CI Rust 1.99.1/Chrome 156
runs pass the complete matrix locally; five exact-version browser cycles also
pass. The fixture now retains bounded worker/input traces, browser status,
focus, console output and a screenshot on failure, and the scaled CI case runs
three complete cycles. The original timeouts and two-level pixel tolerance
are unchanged; no production input change is claimed without a reproduced cause.

The retained browser trace narrows the timeout: the canvas stays focused and the
new key reaches it, but playback stops with `Invalid or late audio output
response`. The fixture's synchronous readback serializes 921,600 individual
JavaScript values through WebDriver while that same main thread receives output
replies. Live comparisons now use native lossless PNG encoding and retain the
same crop and RGB comparisons. After Stop, both profiles additionally compare
every decoded RGB byte of the final retained frame against the previous raw-array path. With the exact CI
compiler/browser and Chrome restricted to two CPUs, all repeated flat controls
and photographic-menu comparisons pass. Measured PNG main-thread delays are
about 9–23 ms versus 91–95 ms for the stopped raw readback; output replies stay
well inside their unchanged 250 ms deadline. The old local raw stress run did
not reproduce the CI stop, so remote validation remains required. Bounded audio
request/reply timing is retained if the issue recurs; no runtime deadline or
pixel tolerance is relaxed.

A separate networking gate failure came from treating 1,000 unpaced headless
ticks as a wait for asynchronous sockets. The fixture also assumed a four-byte
TCP message would arrive in one read. Its Rust runtime driver now waits on a
bounded real ten-second deadline, yielding one millisecond between ticks.
An acknowledgement protocol forces one- and three-byte receive checkpoints
before the complete `pong`, and the guest retains partial writes/reads. A private
250 ms delayed server reproduces the old fixed-frame failure; both core and
component cases complete with the new driver. HTTPS, cached-HTTPS, TCP, UDP and
listener permission denial still pass. The related listener fixture uses the
same elapsed-time driver for native/core/component execution, retaining close
and stale-handle assertions. CLI headless pacing and production networking caps
are unchanged.

### Single-pass desktop image reads and original-resolution tiling study

Cold image assets now use one bounded reader over a verified package member.
The old 64 KiB chunk loop restarted ZIP decompression for each offset. The new
path checks the 33-byte PNG header and available frame allowance before its
bounded encoded allocation, consumes the member once, and completes the ZIP
checksum check before decoding or caching an image. Invalid container CRCs are
rejected even when the embedded PNG itself is valid; failure leaves the cache
empty and a subsequent valid member still loads. Stored and Zstandard members,
exact pixels, cache identity, verification/path rejection and frame exhaustion
have direct regression coverage. The 512-pixel dimension, 2 MiB encoded image,
1 MiB cache and 8 MiB per-frame image allowances are unchanged.

A separate private study retains all pixels of the six pinned upstream
2816×1536 backgrounds in 24 overlapping tiles per image. Exact reconstruction
matches each original RGBA hash. Thirty independent SDL comparisons at 640×360,
1280×720, 1366×768, 1920×1080 and original size stay within the existing two-level
channel tolerance. This is a preparation/rendering proof, not delivery of
original-resolution backgrounds: one tiled background requires 17,430,768
decoded bytes and cannot fit the current 8 MiB frame allowance. Integrating that
artwork still requires explicit bounded resource accounting and browser image
loading/export changes; no resource limit is raised by this reader fix.

The broader hosted suite exposed a separate display-settings fixture failure:
after saving, it could not find the original-width dropdown option, and its
retained screenshot shows a closed popup and a changed vertical-view toggle.
The unchanged test passes locally, so this is not a reproduced production UI
fix. The fixture now observes the UI's save-completion message after checking
the on-disk selection, and observes the actual popup before searching for an
option. It keeps every persistence, restart, custom-width and image-size
assertion, and retains a bounded screenshot/state diagnostic on popup failure.
Remote validation is still required; no UI deadline or expected value changes.

### Reloadable browser image cache

Browser startup previously stopped preparing images after its 32 MiB decoded
allowance was filled, silently making later valid images unavailable. The
worker now retains bounded source plans and an LRU decoded-image cache with the
same 32 MiB allowance. It checks the declared exact PNG chunk structure and CRCs
at startup, warms at most 32 MiB of pixels, and decodes cold or evicted images
when requested. Eviction happens before a replacement pixel allocation. Frame
and command checks run before a cache miss can trigger decoding. The separate
8 MiB frame allowance, 512-pixel image dimensions, 2 MiB PNG limit, 32 MiB encoded
export cap and worker deadlines remain unchanged. These named buffers are not
a cap on the whole browser, JavaScript heap, garbage collector or GPU.

The exporter supplies exact-PNG or precise-RGBA plans for valid PNG assets used
through nearest, filtered, straight-alpha or shader-image imports. Valid
unsupported representations, including opaque 16-bit images, receive the same
host-decoded precise pixels and count toward the encoded export cap. Reloading
uses the shared synchronous bounded PNG decoder, without Canvas color or alpha
conversion; the existing asynchronous decoder entry remains available.

A regression against the previous worker reproduces a valid image being lost
after the first 32 MiB. The current worker draws 37 MiB of exact/precise images
through two complete cache cycles with unchanged hashes and hidden RGB. It
checks bounded startup pixel work, exact/precise eviction and reuse, canonical
and shader bindings, frame rejection without decode, invalid metadata, corrupt
cold-image CRCs without cache/command publication, and recovery. Signed
nearest-only core/component exports additionally verify 36 large PNGs and an
opaque 16-bit fallback, including corruption at startup and a fresh restored
worker. The reusable maintainer command is `python3 tests/browser_image_cache.py`.

A private `--assets` run uses all 48 verified lossless tiles from two pinned
original backgrounds: 34,861,536 decoded bytes exceed the cache allowance.
Both browser profiles preserve every independent tile hash through two complete
cycles, corruption rejection and restored playback. Each probe frame draws one
tile: this establishes cache/data delivery, not full-background composition.
Full original-resolution composition still exceeds the existing per-frame
allowance and needs explicit bounded resource integration. The current full
76-image/five-effect/nine-track port retains its 33,212,842 encoded asset bytes,
original memory shares and saved/corrupt-save behavior.

A final whole-suite run exposed an unrelated negative-test race: an ordinary
worker rejects a control header before the sender necessarily finishes its
body, so the correct rejection can produce `BrokenPipe`. A 2 MiB rejected body
deterministically reproduces the old test failure locally. The fixture now
joins a bounded rejection writer, requires worker failure with the specific
trusted-guard diagnostic, and accepts only `BrokenPipe` on that negative path.
Valid request writes remain strict. Empty controls, ordinary small controls
and the large early-close case pass; production worker framing is unchanged.

The `edf2819` hosted asset-lifetime job stops on the compressed-WAV fixture's
first-tick deadline; its separate PNG corpus job passes. The unchanged source
passes locally with both the default guest compiler and a guest compiled with
CI Rust 1.99.1, using Chrome 156 restricted to two CPUs. Bounded first-tick round trips across twelve fresh
worker launches measure about 11–20 ms against the unchanged 250 ms deadline.
This does not reproduce or fix the hosted stop. The fixture now retains at most
48 worker timing/framing events, eight first-frame timings and eight truncated
console records on failure, with no WAV, guest-memory or draw-command copies.
CI preserves that small diagnostic. Every original whole-WAV checksum,
playback, quota, pending-loan close/release and recovery assertion remains;
production decoding and deadlines are unchanged pending stronger evidence.


The diagnostic follow-up workflow (`40556260274`, source `eff565f`) passes both
asset lifetime and PNG startup jobs. This is a successful retry with bounded
failure diagnostics, not an identified root cause for the earlier deadline.

An explicit signed `limits.render_profile = "large-images-v1"` now selects
32 MiB of image transfer per frame, 128 MiB/256 paths of desktop decoded-image
cache, a 128 MiB browser decoded-image cache and 128 MiB of encoded web assets
plus precise fallbacks. It requires at least 256 MiB declared hard memory and
any recommended memory, and refuses a lower effective player ceiling. Built-in
engines and unknown profile names are rejected. Omitted profiles retain all
previous defaults; individual PNG/dimension/command and time bounds remain.

The maintainer-only original-image runner privately prepares six pinned
2816×1536 source images as 24 lossless tiles each. All 18 complete scenes across
native/core/component preserve every tile hash and charge 17,430,768 image
bytes per frame. Seventy-two desktop comparisons (Vulkan/OpenGL, two sizes,
three runtime paths) and twelve exported-browser comparisons match independent
SDL full-texture scaling within the unchanged two-channel-level tolerance.
Lower user ceilings are rejected in all three runtime paths. These fixtures
qualify the framework profile, not activation of new artwork in the actual
port. Original media and generated fixture packages are not published.


Recipe 0.1.75 now activates the source-resolution artwork path locally. All six
2816×1536 backgrounds use lossless tiles with filtering halos; the menu reuses
background1. The package contains 178 images, all five original-rate effects
and all nine original music tracks. The selected signed profile raises the game
hard/recommended memory to 256 MiB; the effect/music shares remain 3/7 MiB.
The complete browser export measures 61,023,757 encoded asset bytes. No artwork,
track, particle or drawing is omitted to fit the old resource profile.

Actual port rendering passes 36 full-size and 36 half-size desktop menu checks
across Vulkan/OpenGL and native/core/component, plus twelve browser comparisons
against the independent original SDL reference, within the unchanged two-level
channel tolerance. Eighteen dense gameplay scenes retain 200 blocks, 3,000
particles, the full 1 MiB save, 300 commands and 21,640,492 image bytes. Every
actual background is reconstructed from those frame tiles and matches an
independent pinned untiled-source pixel hash. Both browser runtimes also retain
all six dense scenes, every tile hash, original audio and exact parent/guest
budgets and complete release. The complete browser save/corruption/restart
matrix passes with the new resources. All 178 PNGs match independent Node and
actual Chromium decoder hashes; malformed PNG rejection is unchanged.

The earlier 0.1.74 renderer, 42 derived background images, provenance and license
texts are frozen in the host's `tests/fixtures/arkanoo-legacy-artwork.zip`, with
per-file and archive hashes. `copy_legacy_port` restores only that artwork and
renderer, removes the opt-in profile and retains the 16 MiB legacy hard cap and
8 MiB frame cap. Other game logic remains current. The separate 48 kHz clips
remain unchanged. These are test fixtures, not additional files in the playable
package; no player Python dependency is introduced. Historical audio/profile
qualification above remains specific to the smaller legacy fixture.


The preserved `eff565f` broad Rust runs 40556249072 and 40556260664 both fail in
the native/Vulkan portal scene at timer 166. An unchanged old 0.1.74 recipe with
the CI Rust 1.99.1 guest compiler on two CPUs reproduces the same failure.
Bounded diagnostics identify a rejected 1,572,864-byte allocation; the old
recorded profile leaves six MiB for the native process inside its 16/3/7 MiB shares.
This is an actual native allocation failure, not an image-comparison timeout.

The source repair composes the filled portal glow into its exact visible RGBA
bounds in two passes, then applies the original white-disk overlay in order.
The original point generator and all golden point/SDL data are retained.
All 121 timer values 151–271 and all 18 original-reference cases retain exact
compositor geometry and RGBA hashes. The repaired full-media old-budget probe,
new tight-memory generated-geometry probe, and active 0.1.75 full-media probe each
pass 56 actual desktop/browser comparisons, including maximum expansion at 180,
with a measured maximum channel difference 2 against the unchanged limit 5.
The last fixture retains all 178 images and all recordings. CI runs both the
complete playable recipe and the separate 16/3/7 MiB geometry regression; only
the extra geometry fixture omits unrelated assets. The full local Rust suite
passes 806 tests with 171 opt-in ignores, and strict Clippy passes. Hosted
requalification remains required. Temporary private GUI error instrumentation
was removed and the production source/executable restored before positive runs.
