# Security model

There are three intended boundaries: package verification, isolated compilation, and capability-scoped Wasm execution. Only package verification exists today.

Verification checks externally trusted signatures before source policy checks. It bounds decompression and rejects ambiguous paths, duplicates and symlinks. Asset reads use the archive index and never extract a directory.

Compilation must eventually run in an OS sandbox with no ambient network or filesystem, no build scripts or proc macros, and a pinned offline dependency set. Source scanning alone cannot enforce those restrictions. Rust macros can conceal tokens; do not use scan success as permission to run Cargo on untrusted source.

Execution must link only approved host capabilities and enforce memory, fuel and epoch limits. No runtime or native fallback exists yet.

Future tests must cover compiler/macro bypasses, zip bombs, malformed PNG/preview decoding, dependency closures, shader compiler isolation, and runtime filesystem/network escape attempts.
