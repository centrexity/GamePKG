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
| Arkanoo | Gravity, speed-triggered portal behavior and ball-to-ball responses are restored with upstream reference and actual-engine tests. Original world proportions and movement/contact order now match upstream reference checks. Per-ball launch timing, life recovery and victory/endless transitions also match original-code fixtures; prior item/score/level caps are removed. Brick health and original blast rules are also verified. All twelve original procedural patterns are restored. Focused mouse movement and original Space/Escape controls are restored. Editor, cursor/window behavior, menus/music and custom-pattern import/selection still need work. Art/effects remain replacements. Current runs do not persist; high score, unlocked level and gravity selection do. |
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
