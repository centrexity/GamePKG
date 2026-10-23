# GamePKG changes to egui 0.31.1

Upstream: https://github.com/emilk/egui/tree/0.31.1

This host-only fork adds `Event::PointerButtonCancelled` and handles it in
`PointerState`. Cancellation releases the specified pressed button with a
release event containing no click. It does not manufacture movement, change
focus, or activate widgets. Other held buttons remain down. Normal pointer
release and disappearance behavior remain unchanged.

The native adapter emits this event for an owned cancelled touch and when its
touch owner loses focus. Guest packages cannot depend on this crate or access
these host events. Original source and dual MIT/Apache-2.0 licensing are retained.
`UPSTREAM.json` records the original package's VCS revision. License include
paths in the manifests refer to the local copies of the upstream license files.
