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
| Arkanoo | Gravity, speed-triggered portal behavior and ball-to-ball responses are restored with upstream reference and actual-engine tests. Original world proportions and movement/contact order now match upstream reference checks. Per-ball launch timing, life recovery and victory/endless transitions also match original-code fixtures; prior item/score/level caps are removed. Brick health and original blast rules are also verified. All twelve original procedural patterns are restored. Focused mouse movement and original Space/Escape controls are restored. Editor painting/erasing, brick selection, names, save/load, clear confirmation, test return and the custom endless pool are restored. Original ASCII pattern import/export now uses player-selected files. Gameplay cursor confinement/hiding, F11 fullscreen and Q-to-quit outside play are verified in native and browser runtime checks. Normal-size/fullscreen/maximized preferences now persist through the native host. The six main menu actions and fullscreen/effects settings controls are restored. Remaining menu presentation and music/resolution/VSync controls, project-link action and original menus/music still need work. Original shard particles are restored; the metallic ball, glass paddle and normal-brick textures are restored; speed lettering and six campaign image backgrounds are restored and directly verified; editor image selection is restored; grid composition, editor/menu lettering, whole-window scaling and other art/effects still need work; original penguin/heart-loss behavior is now verified. Current runs do not persist; high score, unlocked level, gravity and effects preferences do. |
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

Music controls/playback, resolution and VSync controls, the safe project-link
and version display, splash/HUD presentation, arbitrary original viewport
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
path test. Real Windows/macOS execution of the fix still needs hosted evidence.

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
embedded decoder after relocation. Native hosted results for the new publication
primitive remain pending until the dedicated workflow completes.
