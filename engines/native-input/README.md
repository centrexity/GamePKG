# Shared native input service

This host library gives native engines the same device handles, player-slot rules,
held-state snapshots, ordered events and focus behavior. It does not pump OS
events or remap a game's actions. SDL2, SDL3 and Windows XInput adapters must feed
it from their existing pump/poll and preserve each game's original mappings.
Engine adapters are still pending; the current ports do not use this service yet.

The library has no external Rust dependencies. Consumers use
[`gamepkg_input.h`](../../include/gamepkg_input.h); trusted window/device adapters
use [`gamepkg_input_backend.h`](../../include/gamepkg_input_backend.h).

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
