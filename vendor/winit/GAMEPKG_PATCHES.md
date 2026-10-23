# GamePKG host window backend patch

Based on winit 0.30.13, upstream commit e9809ef54b18499bb4f2cac945719ecc2a61061b.
Original source and license notices are retained.

The X11 backend now updates its cursor-position cache when emitting synthetic
CursorMoved events for enter and focus. Previously these events reached clients
without updating the cache used to suppress duplicate motion; moving back to the
previous cached position could be suppressed, and a click used stale coordinates.
The real two-window X11 regression in tests/x11_cursor_reentry.py reproduces the
old wrong-coordinate click and checks the corrected event stream.
No guest dependencies or other platform backends change.
