# Game networking

Games have no direct socket or HTTP-library access. Their native and optional Wasm runtimes call the same host-controlled API. Network access is off by default and is unrelated to package downloads or update URLs.

Require host `engine_version = ">=0.1.10, <0.2.0"` and declare exact endpoints in the signed manifest:

```toml
[permissions]
input = true
audio = false
save = false
network = "all"                 # none, https, multiplayer, or all
network_hosts = ["example.com:443", "play.example.com:25000"]
network_listen = []
network_allow_local = false
```

`https` permits bounded HTTPS GET requests. `multiplayer` permits TCP and UDP connections and explicitly declared TCP listeners. `all` permits both. Endpoint lists contain exact `host:port` pairs, with no wildcards, credentials, paths, query strings or fragments. Listeners require numeric bind addresses. Local/private addresses require the additional signed `network_allow_local = true` permission, displayed during package review. Empty or undeclared endpoints are rejected.

HTTPS verifies certificates, accepts port 443 only, disables inherited curl configuration and proxies, and pins the connection to a validated DNS result. It does not follow redirects or send the user's cookies or credentials. Public-only permission rejects local, private, reserved and mapped private addresses; DNS answers are checked before the actual connection. Requests have bounded connection/transfer deadlines and a 64 KiB response limit. The host needs `curl`; game code cannot supply curl options.

## API

All calls are under `gamepkg::`; games still use no Cargo crates.

- `https_get(url)` returns a request handle.
- `network_poll(handle, output)` returns HTTPS bytes, or zero bytes when a connection is ready.
- `network_request(url, output)` starts or polls a cached HTTPS GET for that URL.
- `tcp_connect(endpoint)` and `udp_connect(endpoint)` return connection handles.
- `tcp_listen(endpoint)` and `tcp_accept(listener)` support declared TCP server endpoints.
- `network_send(handle, bytes)` returns the number of bytes sent. TCP can send a partial buffer.
- `network_receive(handle, output)` returns received bytes. TCP can return a partial message; UDP buffers should fit the expected datagram.
- `network_close(handle)` releases that game's handle.

Requests and connection establishment are asynchronous. Keep handles in game state and try again on subsequent ticks when a call returns `NetworkError::Pending`. Other errors are `PermissionDenied` and `Failed`. Do not spin waiting for a result inside one tick.

A game has at most 16 handles and can create at most eight per second. DNS/connection jobs share a process-wide limit of 16. Calls and HTTPS responses are limited to 64 KiB; socket transfers have a 256 KiB per-second budget per game. Handles, cached responses and endpoint permissions are scoped to one running game. Closing or stopping a game drops its sockets.

Networking is available in the desktop native worker and desktop core/WIT Wasm runtimes. Browser export does not provide these socket capabilities and rejects unsupported network imports.

`examples/good/network-client` demonstrates an HTTPS request started by Start/Enter and polled over later frames, with no network traffic before that input.

## Host-selected endpoints for reviewed native selectors

A selector whose exact built-in profile supports it may sign `game_data.user_selected_servers = true` with `network = "multiplayer"`, empty static endpoint lists and `network_allow_local = false`. Ordinary guest packages cannot delegate endpoint selection this way. The host validates and remembers a user's join or hosting address, then builds the concrete per-session policy before executing the reviewed engine. Blank choices remain offline. The selected game port and adjacent server-information port are allowed; local/private endpoints require the separate host-user opt-in. The game cannot write these settings or authorize another address through its networking API. Signed creator/declaration changes revoke inherited choices. See the reviewed engine's documentation for its transport, supported address family and additional bounds.

Supported reviewed profiles can additionally accept an explicit host-user master-server choice. This delegates discovery only to that chosen master: the host reads a bounded list, validates numeric endpoints against the user’s local/private-address choice, and materializes temporary per-session game and information-port grants. Blank choices authorize no discovery. Master text is never executed, and the engine receives neither a raw TCP socket nor authority to select a different master. Transport limits, cache/refresh behavior and supported address families are documented with the reviewed engine.
