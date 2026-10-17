# GamePKG host input integration

This is egui-winit 0.31.1 from the revision and original file hashes in
`upstream-lock.json`, under the included MIT and Apache licenses.

The only upstream Rust changes are the `gamepkg_input` module declaration and
calls at the start of window-event and relative-motion handling. The new module
retains physical keys (including Pause and keypad distinctions) before egui
normalizes them. Capture must be explicitly enabled for the active game viewport;
other viewports and unfocused motion are ignored. Focus changes remain ordered.
Disabling capture discards queued events. Overflow emits one failure event rather
than silently dropping input and leaving held controls stuck.

This is a desktop host dependency. It does not permit crates in game sources or
provide guest filesystem, network, process or window-system access.

The module's regression test checks viewport isolation, focus, queue overflow,
drain behavior and disabling capture. Original dependency files remain unchanged
except for the three integration points described above.
