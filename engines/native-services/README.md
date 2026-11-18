# Native port client

`client.rs` is the audited, dependency-free Rust client/bootstrap implementation
for the shared port service. It exports the game-facing C trampoline in
`include/gamepkg_port.h` and safe Rust helper functions. It is host-generated
build input, not a permitted game Cargo crate or a package-owned replacement.

The launcher supplies already-opened, dedicated private pipes to `connect`, then
starts checked game code. The host serves those pipes using
`gamepkg::port_host::serve`. The transport fails closed, serializes concurrent
callers and bounds all copies. `probe.c` is an asset-free C conformance fixture.

See [the service contract](../../docs/PORT_SERVICES.md) for ownership, operations,
desktop providers, source/object checks and required process confinement. The
client does not grant a process permission to call system APIs, and installing
it alone does not complete an existing native port's migration.


The child-local `local_memory_page_states(window, offset, pages)` helper (local
op 17) copies at most 256 page-state bytes from an owned opaque RAM window.
It neither reads contents nor changes protections, and does not expose native
addresses. See the exact alignment/framing/state/lifetime contract in
`PORT_SERVICES.md`; snapshot states 0..3 differ from protect modes 0..2.
Older SDKs may return Unsupported; a port must not guess or call OS APIs instead.

Typed local event helpers now use operations 27 (cancellable event-only wait,
0..1000 ms) and 28 (non-consuming manual/signaled snapshot). A mutex is rejected
without acquisition. Snapshots confer no lease, allocate no quota and cannot
replace guest event-map/timer synchronization. Cancellation, close and release
wake blocked typed waits; the cancellation notification shares the value lock
with the condvar sleep. See PORT_SERVICES.md for framing, errors, lifetimes and
qualification limits. These sources participate in SDK/cache identity.

The generated Rust `local_task_join` returns private-field `LocalTaskJoin` evidence
only after the existing consuming local join succeeds. Read `task_id()` and
`result()`; retain and borrow the receipt across independently owned controllers
sharing that exact job. Completion snapshots and copied integers cannot replace
it. See docs/PORT_SERVICES.md, “Shared completion evidence for one native worker”,
for errors, lifetime limits and current qualification scope. No wire operation or
native permission is added.

Typed local semaphore helpers use ops 43 create, 44 release, 45 wait and 46
state. They share the existing 128-object sync budget, enforce fixed-width bounded
counts, preserve atomic previous-count returns and use cancellable waits of at
most one second. Existing op 25 revokes the ID. State copies are not leases;
wrong-kind operations never consume an event or acquire a mutex. Legacy op 23
does not accept semaphores. Exact framing, errors and cancellation/lifetime
contracts are in PORT_SERVICES.md. This changes SDK/cache identity; it supplies
no NT handle/status adapter and does not admit an unmigrated game.
