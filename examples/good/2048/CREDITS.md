GamePKG adaptation of Adrien Ball’s 2048-rs.
Upstream: https://github.com/adrienball/2048-rs
Revision: 9e972c43e51b153f66b31a08c37eee140a0d2e57

Row sliding and merge logic is adapted from src/utils.rs under the MIT license.
The terminal UI, solver, lookup-table macros and dependencies are replaced with
direct GamePKG drawing and input. Tiles cap at 32768 to fit the upstream nibble representation.
Full upstream license is included in assets/LICENSE-2048.txt.
