# GamePKG host input integration

This is egui-winit 0.31.1 from the revision and original file hashes in
`upstream-lock.json`, under the included MIT and Apache licenses.

The upstream Rust changes are the `gamepkg_input` module declaration,
calls at the start of window-event and relative-motion handling, and text hooks
at accepted keyboard text, clipboard paste and committed IME text. The new module
retains physical keys (including Pause and keypad distinctions) before egui
normalizes them. Text follows the backend's existing shortcut rules and stays
ordered with keys; IME preedit text is never retained. Capture must be explicitly enabled for the active game viewport;
other viewports and unfocused motion are ignored. Focus changes remain ordered.
Disabling capture discards queued events. Overflow emits one failure event rather
than silently dropping input and leaving held controls stuck.

This is a desktop host dependency. It does not permit crates in game sources or
provide guest filesystem, network, process or window-system access.

The module's regression test checks viewport isolation, focus, queue overflow,
drain behavior and disabling capture. Original dependency files remain unchanged
except for the integration points described above.


The private `gamepkg_window` module also supplies actual fullscreen observation
for viewport info. On Linux/X11 it reads only the owned window's `_NET_WM_STATE`
property on winit's existing display connection. Reads are bounded to 64 atoms;
wrong types/formats, larger properties and failures return unavailable. Xlib's
native unsigned-long atom storage is respected on both pointer widths. Other
backends retain winit's window report; Wayland's implementation uses its last
compositor configure event. Actual runtime verification here covers X11 and the
separate browser DOM path, not every native backend.

A fullscreen viewport command still calls winit, then sends the same idempotent
own-window EWMH request on X11. This handles winit's stale cached state after an
external WM change, where it can otherwise skip a real enter/exit request. It
never selects a guest-provided display/window or exposes handles to source.
These two viewport integration points are additional documented changes to the
original dependency. No new Cargo dependency is added.

`tests/window_observation_game.py` checks malformed/empty/64/65-atom properties,
independent hidden windows, actual WM transitions, F11 after external transitions,
saved fullscreen reopening and real browser fullscreen/API/Escape exits. The
bounded property test compiles this module with the app's exact dependencies and
runs on a private display.
