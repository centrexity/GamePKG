# Package examples

`minimal/` and `good/counter/` are valid single-file Rust counter examples. `good/asset-catalog/` adds nested archive assets and a guest function identifying an asset path. `good/rom-required/` declares a user-supplied ROM without bundling one. These demonstrate packaging; the runtime host API is not implemented yet.

Every `bad/` directory is a complete, deliberately rejected package input:

| Example | Rejection |
| --- | --- |
| filesystem | Ambient filesystem path |
| network | Ambient socket API |
| include-bytes | Compile-time file inclusion |
| build-script | Extra build.rs file |
| proc-macro | Procedural macro configuration |
| unknown-crate | Dependency outside the empty allowlist |
| non-slang | Opaque shader binary |

Use `gamepkg pack <directory> -o <outside-path>.gamepkg --key <private-key-file>` to try them. Tests generate temporary signing keys in memory; no creator private keys are included. Input changes before signing are not tampering tests. The security suite separately modifies a signed archive and checks signature rejection.

`good/collect-stars/` is the first playable grid-v1 game. Build after installing, then use GUI Play or `gamepkg run com.gamepkg.collect-stars`. Its movement/scoring runs in the guest; the host renders the returned grid state.
