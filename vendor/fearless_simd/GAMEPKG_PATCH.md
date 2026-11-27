# GamePKG vendoring note

This is the unmodified crates.io release of `fearless_simd` 1.3.0,
vendored through the host's `Cargo.toml` path patch. GamePKG uses it only
inside GamePKG-owned host code. It is not a guest dependency, not a guest
import, and does not expose CPU identity or SIMD selection to games.
