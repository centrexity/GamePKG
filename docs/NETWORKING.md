# Game networking

Games have no direct socket or HTTP-library access. Their native and optional Wasm runtimes call the same host-controlled API. Network access is off by default and is unrelated to package downloads or update URLs.

Networking is a signed on/off permission in the current source-built host:

```toml
[permissions]
input = true
audio = false
save = false
network = true                 # false disables game networking
```

When enabled, HTTPS, TCP and UDP may use any internet or local-network address;
there is no IP, hostname or endpoint allowlist and no separate local-network
opt-in. TCP listeners use numeric bind addresses, including wildcard addresses.
Endpoint arguments still need valid `host:port` syntax. The OS, firewall and
available network interfaces determine which addresses can actually be reached.

Earlier signed manifests using `"none"` are read as disabled; `"https"`,
`"multiplayer"` and `"all"` are read as enabled. Their legacy `network_hosts`,
`network_listen` and `network_allow_local` fields no longer restrict destinations
and are omitted when writing a manifest. Released 0.1.17 binaries retain the older
policy; this change requires rebuilding the host from current source.

HTTPS verifies certificates, supports explicit HTTPS ports, disables inherited
curl configuration and proxies, and pins each connection to its DNS result.
It does not follow redirects or send the user's cookies or credentials.
Requests have bounded connection/transfer deadlines and a 64 KiB response limit.
The host needs `curl`; game code cannot supply curl options.

TCP tries later resolved addresses after connection failures within one shared
two-second deadline. UDP tries later addresses if socket setup fails; a successful
UDP connect does not establish that the remote peer is reachable. HTTPS passes
the complete bounded DNS result to curl for address fallback within its existing
deadlines. Numeric IPv4/IPv6 URLs use their literal address directly. HTTPS DNS
fallback requires curl 7.59 or later.

Socket budgets count bytes actually sent or received. At the rate limit, calls
return `Pending` so games can retry later. TCP can transfer the portion that
fits; UDP waits until the requested outgoing datagram or next incoming datagram
fits, preserving queued data. Incoming UDP buffers should still be large enough
for the datagram. TCP end-of-stream returns zero bytes even when its transfer
budget is exhausted. Failed and pending sends consume no transfer budget.

## API

All calls are under `gamepkg::`; games still use no Cargo crates.

- `https_get(url)` returns a request handle.
- `network_poll(handle, output)` returns HTTPS bytes, or zero bytes when a connection is ready.
- `network_request(url, output)` starts or polls a cached HTTPS GET for that URL.
- `tcp_connect(endpoint)` and `udp_connect(endpoint)` return connection handles.
- `tcp_listen(endpoint)` and `tcp_accept(listener)` support TCP server endpoints.
- `network_send(handle, bytes)` returns the number of bytes sent. TCP can send a partial buffer.
- `network_receive(handle, output)` returns received bytes. TCP can return a partial message; UDP buffers should fit the expected datagram.
- `network_close(handle)` releases that game's handle.

Requests and connection establishment are asynchronous. Keep handles in game state and try again on subsequent ticks when a call returns `NetworkError::Pending`. Other errors are `PermissionDenied` and `Failed`. Do not spin waiting for a result inside one tick.

`network_request` keeps pending work and successful responses cached. If a
request fails, that call returns `Failed` and removes its failed cache entry;
a later call for the same URL can start a fresh request under the normal limits.
A too-small output buffer leaves a successful response cached so the caller can
retry with more space. Use `https_get` with explicit handles for requests whose
completion and cleanup the game needs to manage directly.

A game has at most 16 handles and can create at most eight per second. DNS/connection jobs share a process-wide limit of 16. Calls and HTTPS responses are limited to 64 KiB; socket transfers have a 256 KiB per-second budget per game. Handles and cached responses are scoped to one running game.

If all network workers are occupied, new requests return `Pending` without
spending a game open or handle ID. Closing or stopping a game drops its owned
sockets. Already-running background work may finish later: it continues to count
against the worker limit until it ends, and results for closed handles are
discarded with their resources. Late results cannot replace a newer handle.

A TCP listener leaves incoming connections queued while its game has no free
handle or has exhausted its open-rate budget; retry accepting after freeing a
handle or the rate window resets. Rejected allocations do not consume additional
opens or reuse retired handle IDs.

Networking is available in the desktop native worker and desktop core/WIT Wasm runtimes. Browser export does not provide these socket capabilities and rejects unsupported network imports.

`examples/good/network-client` demonstrates an HTTPS request started by Start/Enter and polled over later frames, with no network traffic before that input.

## Remembered launch addresses for reviewed native selectors

An exact host-approved native profile can sign
`game_data.user_selected_servers = true` to expose remembered join, hosting and
master-server fields in the app or `configure-network`. These are launch
preferences, not networking restrictions. Clearing them clears startup commands;
it does not disable an enabled network permission. `network = false` remains
off regardless of saved launch preferences. Local addresses need no extra opt-in.

The reviewed engine's master browser reads a bounded, sanitized server-list
snapshot and never executes arbitrary returned text. Its original registration
and authentication paths use real nonblocking TCP sockets through the broker.
Connect, bind, listen, accept, polling, socket error/options and shutdown all
operate on host-owned sockets. Combined UDP/TCP/snapshot handles retain the
16-handle/eight-opens-per-second bounds; stream transfers are limited to 4 KiB per
RPC and share the engine's 1 MiB/second budget. The listen backlog is capped at 128.
No OS descriptor reaches game code.

Original-engine tests verify registration, the master's UDP information ping,
cryptographic challenge/response, authenticated privilege and saved-key reload
using a private local master. This is not a live public-service compatibility
claim. LAN discovery and further multiplayer/error/platform fidelity remain open.
DNS work is bounded and a lookup exceeding the broker deadline fails explicitly.

Reviewed engine socket sets hold at most 16 opaque virtual tokens rather than
indexing OS descriptor bitsets. Closing a socket releases its resource while
its token remains retired; replacements receive fresh positive 32-bit tokens.
The token namespace fails closed at its representation boundary instead of
wrapping. The 16-live-socket, open-rate, transfer and RPC bounds are unchanged.
The native build fingerprint includes the socket-set overlay, so old engine
builds require rebuilding against the current host. Pinned upstream sources
remain unchanged; only the verified build copy receives the adapter.
