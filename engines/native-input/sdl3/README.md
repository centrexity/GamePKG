# SDL3 host input adapter

This adapter feeds already-open SDL3 gamepads into the same Rust service used
by the SDL2 and Windows XInput adapters. It uses the engine's existing SDL owner
thread and event pump. It does not open or close SDL devices, change deadzones,
smooth axes, reorder an assigned player or replace a game's action bindings.
It retains all 26 SDL3 button positions, including Misc 2 through Misc 6. These
add button bits without changing the C ABI layout or existing button meanings.

Call `gamepkg_sdl3_focus` from the actual game's window focus, then call
`gamepkg_sdl3_gamepads` after the existing event pump. It returns the highest
occupied player slot plus one, retaining null holes after a disconnect. The
first new button press chooses a player; stick drift and USB connection order
cannot choose the keyboard player's controller. Call `gamepkg_sdl3_snapshot`
once per action-mapping pass and pass that snapshot to the pure button/axis
helpers. They reconstruct SDL coordinates exactly for an existing mapper.
The full signed stick range and original Y inversion are preserved; triggers
retain every nonnegative SDL gamepad value. Focus loss neutralizes controls;
held buttons need a real release before being delivered after focus return.
Forward every event through `gamepkg_sdl3_window_event` before the original
game handlers. Controller button events preserve quick taps and first-press
ownership even when both edges arrive before the next state sample. The event
handler uses the previous report rather than reading future queued buttons
from SDL. State snapshots describe the final level; consumers that require
every tap must read the common event queue.

Pass the actual game window ID. Events directed at another window are rejected.
Before pumping events, call `gamepkg_sdl3_bind_window` with the actual window ID
and its SDL focus state. Use zero after destroying the window. A changed binding
neutralizes input and returns reset flags for clearing legacy views; rebinding
the same window does nothing. Releases queued for a retired window, including
the interval without a window, can clear invisible held latches. They never
release a visible key or mouse button in the replacement window or reach game
actions. Focus still comes from the engine's owned window. The Halo bridge
checks actual SDL focus before resuming after a foreground, shown or restored
notification.
Physical keyboard scancodes, committed Unicode text, precise absolute/relative
mouse movement, five mouse buttons, wheel input and full-width touch surface and
finger IDs enter the same service. Repeated key downs are filtered. Multiple
keyboards and mice retain independent state; the legacy held-state helpers
combine them. Touch-generated mouse input can pass to existing UI handlers but
is not duplicated in the canonical mouse stream.

Check the event return before running the original action handler. Zero drops
the event. A nonzero result includes `GAMEPKG_SDL3_PASS`; when reset flags are
present, rebuild only those legacy keyboard, mouse or touch views from the
shared service. Focus loss, hiding, minimization and app backgrounding release
controls. A foreground notification alone does not grant window focus.
`gamepkg_sdl3_legacy_touch` maps an accepted touch event's local copy to a stable,
non-reused finger proxy for older APIs that cannot represent a surface/finger
pair. The shared contacts retain the original IDs. The engine still owns text
input activation, action bindings, pointer capture and device lifetime.

Disconnect before the engine closes its SDL gamepad, and shut down the adapter
before `SDL_Quit`. The engine retains its controller and rumble lifetime. This
API rejects device access from another thread; pure snapshot conversion does
not touch SDL. Backend ID 3 identifies these devices in the common service.

Maintainer verification uses virtual controllers and no video or audio output:

```sh
python3 tools/check_native_sdl3.py --source /path/to/reviewed-opence \
  --library /path/to/libgamepkg_native_input.a \
  --sdl3-include /path/to/SDL3/include --sdl3-library /path/to/libSDL3.so.0 \
  --output /tmp/gamepkg-sdl3-check --cc clang-18
```

The runner checks the pinned original Halo mapper and stand-in declarations.
It compares every raw coordinate report with four preexisting keyboard-state
patterns, tests all buttons and their queue edges, and checks focus latches,
stable player slots, generation handles and engine-owned device lifetime.
Swapped buttons, incorrect Y conversion, ignored focus loss and slot compaction
must each fail at their specified assertion. The queued-tap negative control
must fail ordered ownership. Window tests exercise physical keys, Unicode
validation, multiple mice, touch identity, background latches and disconnects;
Six further negative controls prove duplicate, malformed-text, foreign-window,
touch-collision, replacement-reset and old-window-release regressions are
detected. `--sanitize` enables C address,
undefined-behavior and leak checks, with no suppressions. Native 32-bit runs use
`--cflags='-m32 -isystem /usr/include/x86_64-linux-gnu'` and matching libraries.
Python is a maintainer test tool, not a player dependency.

The same fixtures can be compiled for the native Windows MSVC ABI. Build the
Rust service for `i686-pc-windows-msvc` with
`RUSTFLAGS="-C target-feature=+crt-static"` to match Halo's static C runtime.
Use an x86 SDK developer environment and the matching SDL3 import library:

```sh
python3 tools/check_native_sdl3.py --platform windows-msvc --build-only \
  --source /path/to/reviewed-opence \
  --library /path/to/gamepkg_native_input.lib \
  --sdl3-include /path/to/SDL3/include --sdl3-library /path/to/SDL3.lib \
  --output /tmp/new-windows-input-fixtures --cc clang
```

Run `positive.exe` and `window-positive.exe` on Windows. The tool copies the
corresponding `SDL3.dll` beside them from the import library's folder; use
`--sdl3-runtime` for a separate DLL location. Its checksum is recorded.
The Windows profile compiles positive fixtures only,
avoiding deliberate assertion dialogs. `--build-only` records compilation
without claiming execution. On native Windows, omit that flag to compile and
execute them. Linux continues to run all eleven deliberate regression controls
and memory instrumentation. These Windows fixtures passed on both test laptops
with the same service/gateway used by the private native Halo build. Controllers
are virtual and window events are test records; this does not qualify physical
Bluetooth input or Windows gameplay.

This component is under integration. A private native Halo executable routes
its existing SDL event pump through this gateway. Actual A30 campaign tests
have verified controller aim and walking, OS keyboard walking, OS mouse firing
and aim, and OS focus latching with dummy audio on a separate GPU desktop.
The Linux i686 player plan now embeds this service and the adapted platform
sources through the reviewed preparation data; its end-to-end build is under
qualification. Windows preparation, full campaign/co-op, split-screen/rumble and physical
Bluetooth checks remain. These checks do not establish frame-time performance
or Windows gameplay qualification.

Halo's integration additionally uses `engines/halo/input/halo_gamepad_ports.h`
to retain an existing player's game port when another controller joins or
leaves. Its original deliberate solo/split transition still waits for a lone
controller to be at rest. This game-specific keyboard-sharing policy is separate
from canonical device ownership; the SDL3 fixture checks both using real shared
generation handles. Complete split-screen gameplay and rumble remain to verify.
