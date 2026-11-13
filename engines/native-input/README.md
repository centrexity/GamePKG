# Shared native input service

This host library gives native engines the same device handles, player-slot rules,
held-state snapshots, ordered events and focus behavior. It does not pump OS
events or remap a game's actions. SDL2, SDL3 and Windows XInput adapters must feed
it from their existing pump/poll and preserve each game's original mappings.
The Windows XInput and SDL2 source-kit adapters feed this service and read its
player snapshots before the engine's original Duke mapping. SDL2 keeps its one
existing event pump and its original rumble lifetime/exit handling. Both headed
and headless SDL2 now use first-button ownership. Its window dispatcher also
feeds keyboard, text, mouse and touch events into the shared service, gating the
legacy key/touch action view through accepted input. Windows window input and
Halo's SDL3 adapter are still pending. Existing installed packages need a new
source kit and rebuild before they use these adapters.

The library has no external Rust dependencies. Consumers use
[`gamepkg_input.h`](../../include/gamepkg_input.h); trusted window/device adapters
use [`gamepkg_input_backend.h`](../../include/gamepkg_input_backend.h).
This is a trusted host implementation. Game source still uses the supplied
GamePKG functions and may not add Cargo dependencies.

- Device handles are process-local generations, never reused after disconnect.
  A backend ID and full-width token identify the host device within an adapter.
- A fresh controller button press claims the lowest free player slot. Axis drift
  and physical USB/XInput slot order cannot choose the first player. Reconnection
  leaves other players assigned to their existing devices.
- Stick and trigger samples retain their full integer range. The service adds no
  deadzone, acceleration, smoothing, event-pump thread or OS poll. A backend must
  keep any original game-specific conversion in its existing mapping layer.
- Focus loss releases keyboard/mouse controls, neutralizes controllers and cancels
  touches. Held buttons, including the first controller sample after focus return,
  need release before another press. Relative mouse motion stays relative;
  text retains Unicode scalars; touch contacts retain full 64-bit identities.
- A device-only reset suppresses its held keys/buttons until release without
  resetting another device or moving a controller's player slot. SDL system
  shortcuts use this for the keyboard while leaving a controller unchanged.
- The event queue is bounded at 2,048 records. Overflow emits RESET and retains
  current held-state snapshots. Event consumers must resample state without
  synthesizing new press edges. Snapshot-only games can continue holding an
  accelerator even when no one drains the event queue.
- Provider callbacks never run inside the service lock. Consumers read coherent
  snapshots; rumble adapters must route through the selected device handle,
  rather than assuming the logical player number is its physical device slot.

Maintainer verification (Python is not a player dependency):

```sh
python3 tools/check_native_input.py --target x86_64-unknown-linux-gnu --output /tmp/input64
python3 tools/check_native_input.py --target i686-unknown-linux-gnu --cc clang-18 --output /tmp/input32
```

The same runner supports native Windows MSVC x86/x64 after selecting the matching
Visual Studio developer environment. It executes the Rust regressions and links
and runs an actual C caller against the Rust static library. CI also compiles
ARMv7 and i686 Android libraries with layout assertions. These checks establish
service and ABI behavior, not physical controller delivery, game integration,
Android gameplay or frame-time qualification.

The Windows adapter tests use fake OS callbacks and the real Rust library. They
check every 16-bit axis value and trigger byte, first-button ownership, neutral
unclaimed devices, reconnects, focus transitions, packet changes and rumble
routing. The provider polls connected pads at the game's existing cadence and
backs off empty physical slots; the engine's separate disconnected-player timer
does not hide new host snapshots. Windows source kits link the built-in XInput
1.4 library rather than requiring the optional legacy XInput 1.3 DLL.

Pass `--pinned-xinput PATH --pinned-input-core PATH` to the Linux runner to also
verify adaptation of the pinned SDK sources, rejection of altered source and
in-place writes, the unchanged SDL backend, and negative controls for both
logical-player and connection-mask probe policies.

`tools/check_native_sdl2.py --toolkit PATH --library PATH --output PATH` builds
the pinned actual SDK backend/core with the Rust archive and exercises virtual
controllers in both main-pump and headless modes. It compares 65,536 raw states
through the original Duke mapper and checks all 21 common physical buttons.
The original 682 Linux mapping checks and 15 rumble checks also run; the rumble
fixture supplies one virtual join press for the new ownership rule, retaining
all original timing and shutdown assertions. Swapped-button and incorrect Y
conversion negative controls must fail. SDK source remains unchanged, including
the Windows backend. CI executes this integration as native Linux x86 and x64.

The same runner exercises the actual window dispatcher against a hidden dummy
SDL window: physical USB HID usages and layout-dependent key labels, duplicate
and repeated key events, paired modifiers/Enter keys, layout changes while held,
Unicode validation, absolute/relative mouse motion, five mouse buttons, precise
wheel input, and independent contacts on multiple touch surfaces. The legacy
touch layout receives non-reused proxy handles for the full surface/finger pair;
neither ID is truncated. Background, hidden and minimized events clear both the
canonical state and the legacy key/touch action view. Reopening an application
does not grant input focus by itself. Synthetic finger-generated mouse events
are omitted from the canonical mouse stream. SDL2 supplies one aggregate keyboard.
The adapter does not start another pump or activate text input itself.

Removing the legacy input rejection gate or collapsing touch identity must fail
independent negative controls. `--sanitize` enables C AddressSanitizer,
UndefinedBehaviorSanitizer and leak checks. Only the original SDK mapping test
suppresses its documented retained environment strings across test reloads;
the new adapter tests run without leak suppression. CI runs this sanitizer
profile on native Linux x64.

Hotplug tests reconnect 500 virtual controllers while a second thread polls the
actual backend, then close it while polling continues. Controller lifetime,
ownership metadata and packet counters share the pad lock; main-pump ownership
uses an atomic flag. Glyph metadata is copied under that lock and listeners run
after it is released, so a listener can safely poll input itself. Both headed
and headless fixtures exercise such a reentrant listener.
