# Port fidelity audit

Audit date: 2026-10-17. Compare behavior against each credited, pinned upstream
revision. A playable port or passing smoke test does not prove feature parity.
The implementation tracker is [port-parity.toml](port-parity.toml); unfinished
work stays open until behavior is verified.

## Confirmed missing or changed behavior

| Port | Remaining gaps |
| --- | --- |
| Cube 2 | Initial OS lock-key state, remaining uncommon-key/platform behavior and full controller/touch equivalence; LAN discovery, master authentication and public server registration; custom asset/mod workflows; demo/movie output and persistence; hardware-accelerated engine rendering; native platforms beyond Linux. Vulkan currently presents software-rendered engine frames. |
| DOOM | Original WAD music, spatial stereo effects, six save slots, configuration, desktop controls and the original quit/ENDOOM flow are runtime verified. Music backend selection, multiplayer, additional WADs/mods and broader control/platform coverage remain unfinished. |
| Zelda3 | External MSU music, replay tools, save states, widescreen, language selection and desktop enhancement settings need support. The adapter fixes the view to 256×224. Original DSP stereo is verified in native and Wasm gameplay/save tests. |
| Arkanoo | Gravity, speed-triggered portal behavior and ball-to-ball responses are restored with upstream reference and actual-engine tests. Original world proportions and movement/contact order now match upstream reference checks. Per-ball launch timing, life recovery and victory/endless transitions also match original-code fixtures; prior item/score/level caps are removed. Brick health and original blast rules are also verified. All twelve original procedural patterns are restored. Focused mouse movement and original Space/Escape controls are restored. Editor painting/erasing, brick selection, names, save/load, clear confirmation, test return and the custom endless pool are restored. Original ASCII pattern import/export now uses player-selected files. Gameplay cursor confinement/hiding, F11 fullscreen and Q-to-quit outside play are verified in native and browser runtime checks. Normal-size/fullscreen/maximized preferences now persist through the native host. The six main menu actions and fullscreen/effects settings controls are restored. Remaining menu presentation and music/resolution/VSync controls, project-link action and original menus/music still need work. Original shard particles are restored; ball trails, remaining art and other effects still need work. Current runs do not persist; high score, unlocked level, gravity and effects preferences do. |
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
| `engines/cube2/adapters/network.h` | Broadcast enabling and socket listen/accept are rejected. Reverse lookup returns a numeric address. Bounded buffer sizes, timeouts, address reuse and TTL now reach the actual host socket; error/TTL queries read host results. Shutdown still returns success without a host call. | Audit each original ENet caller; finish the host operations it needs and test real discovery, authentication, server registration, disconnect and error handling. Keep permission denial explicit. Do not infer complete networking from ordinary packet exchange. |
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

Latest local validation: the full Rust suite passes 372 tests. Its 49
fixture-dependent or platform-specific ignored tests are not counted as passed.
The signed isolated Arkanoo harness separately executes 25 fixture tests on Wasm
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
ordinary gameplay/editor checks with the restored effects. Ball trails/fireball
glows, penguin/heart-loss animation and remaining original art/audio stay open.
