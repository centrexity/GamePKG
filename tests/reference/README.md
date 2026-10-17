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
