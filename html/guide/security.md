# Security model

There are three intended boundaries: package verification, isolated compilation, and capability-scoped Wasm execution. Package verification and an initial Linux-only isolated single-file build exist today; The experimental grid-v1 Wasmtime runtime executes with no imported functions or WASI, and enforces memory, fuel and epoch limits. Broader host APIs remain pending.

Verification checks the signature against the manifest creator key before source policy checks. Embedded signatures establish integrity, not independent creator identity. First installation requires creator approval; subsequent installations pin the same key and reject rollback versions. It bounds decompression and rejects ambiguous paths, duplicates and symlinks. Asset reads use the archive index and never extract a directory.

The initial compiler runs under bubblewrap with an isolated network, a private working directory, and read-only system/toolchain mounts, without the user home directory. It invokes rustc directly on the verified dependency-free source file and fails closed if sandbox setup fails. Build scripts and proc macros are unsupported. A pinned offline dependency set and compiler memory caps remain future work. Source scanning alone cannot enforce those restrictions. Rust macros can conceal tokens; do not use scan success as permission to run Cargo on untrusted source.

Execution must link only approved host capabilities and enforce memory, fuel and epoch limits. The grid-v1 runtime exists with an empty import surface and no native execution fallback; the broader WIT host API is pending.

Future tests must cover compiler/macro bypasses, zip bombs, malformed PNG/preview decoding, dependency closures, shader compiler isolation, and runtime filesystem/network escape attempts.

Host-controlled HTTPS and Tor onion package downloads do not grant guest network access. Downloaded files are bounded, verified and staged before installation. Signed update URLs point directly to new packages; every candidate must keep the installed ID and creator key.

Onion downloads use a local SOCKS5 proxy with remote hostname resolution and no direct fallback. NO_PROXY cannot bypass onion routing. Redirects are checked before each request; an onion request cannot redirect to an ordinary host. HTTPS certificate verification remains enabled.
