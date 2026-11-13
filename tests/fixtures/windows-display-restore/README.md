# Windows display return checks

The tests import the production Windows notification decoder and display state.
They check DWORD layout, malformed payloads, notification ordering, dimming,
repeated lid cycles, and waiting for both the lid and panel to become available.

Run the portable transition tests from the repository root:

```sh
python tools/check_windows_display_restore.py
```

On Windows, the same command also runs the native decoder tests. Choose an
installed Windows Rust target with `--target i686-pc-windows-msvc` or
`--target x86_64-pc-windows-msvc` to check either pointer width.

A Windows machine with Vulkan and an active, available display can additionally
run the background Vulkan integration window:

```sh
python tools/check_windows_display_restore.py --vulkan
```

This sends 14 synthetic notification sequences to its own window, checks the
resulting surface reconfigurations, and reads back rendered pixels. It does not
change monitor power, lid settings, game media, saves, or audio. It does not
replace testing an actual lid-close/reopen cycle with the complete application.
Python and the fixture are maintainer tools; players do not need them.
