# 2048 solver oracle

`2048_solver.rs` is a maintainer-only harness for the original executable's
solver and evaluators. It is never included in a game package. Players need
neither these dependencies nor Python.

Check out `adrienball/2048-rs` at
`9e972c43e51b153f66b31a08c37eee140a0d2e57`. Verify the four source hashes recorded
in `tests/fixtures/2048-solver.json`, then copy the harness to
`examples/gamepkg_golden.rs` in that checkout and run:

```sh
cargo +stable run --locked --example gamepkg_golden
```

The output records evaluation scores and next-move directions using the
original CLI's combined evaluator, depth 3, four probability 0.1 and minimum
branch probability 0.001. The original solver/evaluator/board sources stay
unchanged. The explicit toolchain override avoids installing the old upstream
rust-toolchain version just for this comparison.

Keep these reference results independent of the GamePKG implementation; do not
regenerate expected results by calling the port. Ordinary regression tests load
the committed fixture and need no network or upstream build. Tests also compare
cooperative chunk sizes, all original adaptive depth tiers, legal/terminal
positions, and preservation of the spawning random stream. Tiles beyond the
old nibble limit use the highest original depth tier and separate extended-tile
rule tests.

# Arkanoo force-law oracle

`arkanoo_forces.py` extracts and compiles the unchanged gravity/spin fragment
from pinned revision `9d613e78c9ed5657ae7fda9206cd4a82a893b7c8`. Run it against
a clean `c0m4r/arkanoo` checkout to reproduce `fixtures/arkanoo-forces.json`:

```sh
python3 tests/reference/arkanoo_forces.py /path/to/arkanoo
```

The fixture records full-file and fragment hashes. Expected vectors use original
units; port tests compare at the port's half velocity/spin scale. This proves
the force-law fragment, not full collision/world-geometry equivalence. Python
and the reference compiler harness are maintainer-only tools.

Arkanoo ball-contact oracle:

```sh
python3 tests/reference/arkanoo_contacts.py /path/to/pinned/arkanoo > tests/fixtures/arkanoo-contacts.json
cargo test --test game_arkanoo
```

This extracts and compiles the original contact block without changing its
collision logic. Minimal stand-ins record particle counts and ignore audio.
Seven cases cover overlap, strict touching boundaries, exact coincidences,
multiple simultaneous pairs, portal exclusion and diagonal distance. Fixtures
use original top-left coordinates; tests convert to half-scale ball centers.
The contact oracle proves contact responses, not full world geometry, collision ordering
or particle rendering/lifetime parity. The Python script is a maintainer tool;
players do not need Python.

Arkanoo world and paddle oracles:

```sh
python3 tests/reference/arkanoo_world.py /path/to/pinned/arkanoo > tests/fixtures/arkanoo-world.json
cargo test --test game_arkanoo
```

This compiles the unchanged original `Ball::update` and complete `Paddle` code.
Unattached ball trajectories cover wall clamps, the bottom loss boundary,
minimum horizontal velocity, spin and both gravity directions/speeds. Paddle
sequences cover movement, per-direction edge clamping, repeated long bonuses,
center preservation, velocity history, expiry and ghost timers. A minimal SDL
rectangle wrapper calls the actual system `SDL_IntersectRect` to record integer
edge-contact behavior. The unchanged original brick-resolution fragment is also
compiled against SDL intersections for all four sides, corner ties, fractional
coordinates, strict misses and ghost pass-through. Regenerating this maintainer fixture needs SDL2 development
libraries and Rust; players need neither SDL2 nor Python. Rendering, launch RNG,
attached-ball timing and other full-game transitions need separate coverage.

Arkanoo launch and transition oracle:

```sh
python3 tests/reference/arkanoo_transitions.py /path/to/pinned/arkanoo > tests/fixtures/arkanoo-transitions.json
cargo test --test game_arkanoo
```

The complete unchanged original Ball implementation supplies six 33-frame
traces: all three launch directions, each automatic and manual. A small RNG
stand-in selects the direction; expected countdown/velocity/movement is executed
by the original code. Unchanged life-loss, level-completion and next-level reset fragments cover
score deductions, respawn, portal exclusion, recovery, victory, endless mode,
and clearing items while retaining score/lives across level transitions.
Minimal audio/animation stand-ins ignore presentation; this does not verify
penguin rendering, original menus or full run persistence. Production randomness
comes exclusively from GamePKG's host API. The unit harness injects deterministic
words, including the modulo rejection tail and the no-relaunch/no-ammo-on-launch
rules. Python is only needed to regenerate maintainer fixtures.

Arkanoo special-brick and rocket oracle:

```sh
python3 tests/reference/arkanoo_bricks.py /path/to/pinned/arkanoo > tests/fixtures/arkanoo-bricks.json
cargo test --test game_arkanoo
```

This compiles unchanged original brick collision/explosion and rocket-update
phases, original Block/Rocket constructors, and actual SDL rectangle methods.
Ten fixtures cover normal/ice/metal, ghost contacts, ordered overlapping bricks,
explosive and rocket radii (60 and 120 original pixels), exact axial/diagonal
boundaries, outside points, no indirect chains, armor bypass, repeated hits,
multiple rockets and inactive rockets above the world. Original RNG calls use
a fixed valid 0.99 sample to suppress incidental bonuses; audio and queued
particle output are ignored. Production tests independently force a successful
bonus sample to verify indirect damage does not consume it. Fixture regeneration
requires Rust/SDL2/Python for maintainers. SDL2 and Python are not player
requirements; player builds use GamePKG's existing Rust toolchain and host APIs.
This does not prove editor UI,
original visual effects or full campaign coverage.

Arkanoo procedural-level and portable seeded-stream oracle:

```sh
python3 tests/reference/arkanoo_levels.py /path/to/pinned/arkanoo --rand-source /path/to/rand-0.8.5 > tests/fixtures/arkanoo-levels.json
cargo test --test game_arkanoo
```

The harness compiles the complete unchanged original `create_blocks` function
with exact rand 0.8.5/rand_chacha 0.3.1/rand_core 0.6.4 dependencies. It runs on
64-bit hosts to define canonical original layouts. Its custom-pattern loader
returns an empty list, representing no custom patterns installed. A separate,
explicitly modified diagnostic copy returns the first attempt to identify retry
cases; final expected layouts always come from the unchanged original function.
Fixtures cover all nine campaign levels and 97 procedural levels, all twelve
types, five low-density retries, large levels and u32::MAX. Six seeds also record
raw stream words across block boundaries, uniform integer/float values, standard
unit floats and Bernoulli samples. Dependency versions and source hashes are
recorded. The reference Cargo project/lockfile and build output are temporary;
its compiler output stays outside the guest package.

Production uses fixed-width scalar arithmetic and canonical 64-bit sampling so
wasm32 and native builds produce the same layouts. No Rand crate is a guest
dependency; the adapted helper's MIT terms and attribution ship with the port.
The signed-runtime fixture in `tests/arkanoo.py` compares all 106 rendered boards
on both native and Wasm targets, including exact brick coordinates. This does
not verify custom-pattern import/selection, original editor UI, or full level
playthroughs. Python and the original Rand dependencies are maintainer tools;
players use the ordinary GamePKG toolchain and safe host APIs.

Arkanoo editor and custom-pattern pool oracles:

```sh
python3 tests/reference/arkanoo_editor.py /path/to/pinned/arkanoo > tests/fixtures/arkanoo-editor.json
python3 tests/reference/arkanoo_levels.py /path/to/pinned/arkanoo --rand-source /path/to/rand-0.8.5 --custom > tests/fixtures/arkanoo-custom-levels.json
cargo test --test game_arkanoo
```

The editor harness compiles the complete unchanged original `editor.rs`, using
its actual temporary-folder ASCII reads/writes. Entities, colors and brick
construction come from the pinned original. A minimal rectangle/menu-button
scaffold permits compilation without a window; it does not verify rendering,
hover, SDL event dispatch or the main menu. Fixtures cover all nine choices,
integer grid boundaries, duplicate paint, left/right drag priority, erasing,
loaded versus newly painted health, ASCII old/new symbols and row truncation,
clear confirmation, Unicode backspace and background cycling.

The custom generator harness compiles the unchanged `create_blocks` and
`create_blocks_from_pattern`, with pinned rand versions and a controlled
three-pattern loader. The pool includes all brick types, a diagonal normal
layout and an empty layout. Its order is fixture input, independent of host
directory enumeration. All twelve procedural choices and three custom choices
are represented across the same 106 levels as the procedural oracle. The port
uses sorted saved-pattern names as its stable pool order. Native and wasm32
signed runtime fixtures verify resulting cell/color/type values, alongside
actual editor save/load/test return, denied saving and damaged archives.
This reference does not exercise file dialogs or original editor art/fonts. The signed runtime and GUI checks in tests/arkanoo.py separately cover player-selected ASCII file import/export.

`arkanoo_flow.py /path/to/pinned/arkanoo` regenerates `arkanoo-flow.json` by
compiling unchanged original constructors/reset/pause and main-loop splash,
Enter-skip and editor-exit fragments. Full-file SHA-256 guards pin both sources.
Seventeen observations cover startup boundaries, skipped and expired restart
timers, and editor return before/after starting play. SDL calls and entity
allocation are stubs; this verifies state flow, not artwork/audio or physics.

`arkanoo_particles.py /path/to/pinned/arkanoo` regenerates
`arkanoo-particles.json`. It compiles the complete unchanged original Particle
implementation and shard draw function, recording SDL draw points. Full-file
hashes guard both originals; 114 observations cover lifecycle, rotation, alpha,
point order/duplicates and negative/edge coordinates. Controlled constructor
randomness isolates the original range/default formulas. GamePKG's independent
field packing/rejection is tested separately for uniform ranges and host calls.

`arkanoo_trails.py /path/to/pinned/arkanoo` regenerates `arkanoo-trails.json`.
It compiles unchanged original Ball-update and shiny-ball effect fragments with
SHA-256 guards for both complete upstream files. The renderer records SDL point
and RGBA operations before the cached ball/font draws. Forty-nine history frames
and sixteen drawing cases cover thresholds, tier drop, pre-force timing,
attached-state freezing, pulses/ring, clipping/negative positions, wide history
and large frame values. It supplies 15,909 ordered point/color operations; cached
ball artwork and speed text are explicitly outside this fixture's scope.
