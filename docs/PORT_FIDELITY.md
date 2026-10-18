# Port fidelity audit

Audit date: 2026-10-17. Compare behavior against each credited, pinned upstream
revision. A playable port or passing smoke test does not prove feature parity.
The implementation tracker is [port-parity.toml](port-parity.toml); unfinished
work stays open until behavior is verified.

## Confirmed missing or changed behavior

| Port | Remaining gaps |
| --- | --- |
| Cube 2 | Wayland lock-state access, Windows lock-state runtime validation, remaining uncommon-key/platform behavior and full controller/touch equivalence; further LAN/platform and master authentication/registration fault/service coverage; custom asset/mod workflows; demo/movie output and persistence; hardware-accelerated engine rendering; native platforms beyond Linux. Vulkan currently presents software-rendered engine frames. |
| DOOM | Original WAD music, spatial stereo effects, six save slots, configuration, desktop controls and the original quit/ENDOOM flow are runtime verified. Music backend selection, multiplayer, additional WADs/mods and broader control/platform coverage remain unfinished. |
| Zelda3 | External MSU music, replay tools, save states, widescreen, language selection and desktop enhancement settings need support. The adapter fixes the view to 256×224. Original DSP stereo is verified in native and Wasm gameplay/save tests. |
| Arkanoo | Gravity, speed-triggered portal behavior and ball-to-ball responses are restored with upstream reference and actual-engine tests. Original world proportions and movement/contact order now match upstream reference checks. Per-ball launch timing, life recovery and victory/endless transitions also match original-code fixtures; prior item/score/level caps are removed. Brick health and original blast rules are also verified. All twelve original procedural patterns are restored. Focused mouse movement and original Space/Escape controls are restored. Editor painting/erasing, brick selection, names, save/load, clear confirmation, test return and the custom endless pool are restored. Original ASCII pattern import/export now uses player-selected files. Gameplay cursor confinement/hiding, F11 fullscreen and Q-to-quit outside play are verified in native and browser runtime checks. Normal-size/fullscreen/maximized preferences now persist through the native host. The six main menu actions and fullscreen/effects settings controls are restored. Remaining menu presentation and music/resolution/VSync controls, project-link action and original menus/music still need work. Original shard particles are restored; the metallic ball, glass paddle and normal-brick textures are restored; speed lettering and other art/effects still need work; original penguin/heart-loss behavior is now verified. Current runs do not persist; high score, unlocked level, gravity and effects preferences do. |
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
| `engines/zelda3/adapters/engine.c` | MSU enable is ignored and MSU track playback falls back to the internal audio path. Initialization fixes extra horizontal area to zero and selects the default language. The frame scheduler omits the desktop replay log. | Add selected optional resources, enhancement/language configuration, and bounded replay/save-state persistence with actual engine tests. Ordinary ROM gameplay does not verify these features. |
| `ports/arkanoo/source/src/menu.rs` and `render.rs` | The current menu and synthesized effects cover only part of the original presentation/settings. | Restore the remaining transitions, resolution confirmation/revert, VSync, homepage action and licensed art/audio; verify each against the pinned original. Window-size persistence alone does not implement a resolution menu. |
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
game-over, level-transition/victory and editor presentation. Background/menu/HUD presentation, bitmap lettering and synthesized audio still
contain replacements. Restored object art and portal stages have the measured
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
project-homepage action, fonts/background artwork and music remain open.

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
uses at most six 512×512 tiles; full-viewport corner coverage prevents silent
loss through the per-texture dimension limit.
Disjoint random-word fields and rejection sampling preserve independent uniform
ranges without per-field host RPCs. Native desktop and both browser profiles pass
ordinary gameplay/editor checks with the restored effects. Ball trails/fireball glows and penguin/heart-loss behavior are restored below;
remaining original art/audio stay open.

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
