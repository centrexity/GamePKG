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
