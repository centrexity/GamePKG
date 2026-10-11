# Package examples

Every directory below is a complete source package. Generate a private creator key with `gamepkg keygen -o creator.key`, then pack an example to a path outside its source directory:

```sh
gamepkg pack examples/good/collect-stars -o /tmp/collect-stars.gamepkg --key creator.key
gamepkg install /tmp/collect-stars.gamepkg --trust-creator
gamepkg build com.gamepkg.collect-stars
gamepkg run com.gamepkg.collect-stars
```

The desktop library provides Build and Play for installed games. Graphics games need the GUI for visible rendering; `run --frames 3` exercises their guest logic without a window. Builds currently require Linux isolation, a Rust Wasm target, and pinned Slang 2026.19. Use the GLSL backend in Settings for the desktop shader example.

| Good example | Behavior |
| --- | --- |
| minimal | Move along a row using left/right input |
| counter | Count completed guest ticks |
| asset-catalog | Read a nested asset from the signed archive |
| collect-stars | Move on a grid and collect targets |
| byteorder-game | Use a checksum-pinned, offline guest dependency |
| modular-game | Split checked source across files and use the approved dependency |
| component-playground | Draw using the WIT component host API |
| shape-playground | Draw rectangles and circles from guest commands |
| shader-switch | Switch between two compiled Slang fragment passes |
| tone-pad | Submit bounded audio samples with pitch controls |
| save-counter | Persist the tick count across play sessions |
| rom-required | Read a ROM file explicitly supplied by the player |

`tests/good_examples.py` signs, verifies, installs, builds, and runs all 13 examples, checking representative scores and required-ROM rejection. It uses temporary keys and an isolated library; no private creator keys are included. Dedicated GUI tests check rendered graphics and audio behavior.

Every `bad/` directory is a complete, deliberately rejected package input:

| Example | Rejection |
| --- | --- |
| filesystem | Ambient filesystem path |
| network | Ambient socket API |
| include-bytes | Compile-time file inclusion |
| build-script | Extra build.rs file |
| proc-macro | Procedural macro configuration |
| unknown-crate | Dependency outside the pinned allowlist |
| non-slang | Opaque shader binary |

Packing these examples must fail. The security suite also modifies already signed archives to test tampering rejection, and tests malformed module layouts, hidden forbidden source, memory limits, shader restrictions, and archive bounds.

`sprite-playground` demonstrates bounded RGBA sprites, alpha transparency, and held-arrow movement through the host drawing API.
