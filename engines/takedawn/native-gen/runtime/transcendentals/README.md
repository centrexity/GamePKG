# x87 transcendental numerical components

This directory contains a C adaptation of the Bochs F2XM1, FYL2X/FYL2XP1, FPATAN and FSIN/FCOS/FSINCOS/FPTAN numerical evaluators,
pinned at revision `b0c88410501f48bbe1a966cab1c0592fb504915a`.
Original source and notices are retained in `upstream/`; `upstream-lock.json`
records all original file hashes. `gamepkg_f2xm1.c`, `gamepkg_fyl2x.c`, `gamepkg_fpatan.c` and `gamepkg_fsincos.c` replace the Bochs numerical
and CPU/status interfaces with GamePKG's integer multiword helpers. It returns
a binary128 intermediate and an exact exponent adjustment. The separate
`../gamepkg_x86_x87_trans.c` host service implements stack, status, deferred
exceptions and final result publication.

The adapted evaluators are LGPL-2.1-or-later. The full license is
[upstream/COPYING](upstream/COPYING); original legacy SoftFloat extension notices
are retained prominently. This component's license is separate from the root
Apache license and the BSD-licensed Berkeley SoftFloat 3e subset. The pinned
Bochs C++ reference under `reference/` is only compiled by maintainer tests;
its original notices apply. It is not linked into the player runtime.

Keep these notices, the complete corresponding modified source, build inputs
and source offer/access information with any binary delivery incorporating the
component. Native builds must preserve a way to rebuild and relink against a
modified component; do not ship only a stripped executable without the required
source/relink materials. Source files here are ordinary separately compiled C
translation units. No signed player package incorporating this component has
been produced by these qualification tests.

## Numerical contract and qualification

This is a fixed P5/P6 approximation profile adapted from the pinned evaluator.
It is checked bit-for-bit against an independently compiled original Bochs
implementation for nonzero normal inputs inside (-1,1). Modern physical x87
can differ in the last result bit and C1 for approximate transcendentals.
Tests retain exact physical comparisons for stack/exception behavior and the
remaining registers. Endpoint behavior is handled by the host adapter and
checked against an explicit fixed table, including C1 and directed rounding.
Current Intel Xeon 8573C endpoint C1 differs from earlier physical qualification;
CI must not choose a different numerical profile based on its host processor.
Exception flags, stack/commit responses and the other registers still use the
physical oracle at endpoints. This table is not original-Xbox qualification.

The maintainer oracle also checks the output against MPFR at 512 bits, with
Intel's strict error bounds (less than 1 ulp nearest, less than 1.5 ulp for
other rounding modes), and checks adjacent-input monotonicity across table and
polynomial boundaries. PC does not reduce the 64-bit significand result.
Original Xbox/Pentium III bit-for-bit qualification remains unverified; these
checks do not establish complete game execution or full instruction coverage.

Run `cargo test --locked --release --manifest-path
engines/takedawn/native-gen/Cargo.toml --test x87_f2xm1 -- --nocapture` from the
repository root. Maintainer dependencies are C/C++, GMP and MPFR development
headers (`libmpfr-dev` on Debian/Ubuntu). `GAMEPKG_MPFR_INCLUDE` can select a
private MPFR header directory. Players require none of these test dependencies.
`GAMEPKG_TAKEDAWN_X87_F2XM1_CORPUS` exports explicit little-endian state records
for the integer-only cross-target replay tools in `tools/`.

References: [pinned Bochs source](https://github.com/bochs-emu/Bochs/tree/b0c88410501f48bbe1a966cab1c0592fb504915a/bochs/cpu/fpu),
[Intel SDM volume 1, section 8.3.10](https://cdrdv2-public.intel.com/671436/253665-sdm-vol-1.pdf).


FYL2X additionally handles positive finite x and finite nonzero y through
 gamepkg_fyl2x_evaluate. y is normalized before multiplication and its exponent
is applied at final raw80 rounding. The host adapter owns special values,
source flags, status and conditional ST1 write/pop. Exact powers of two use
exact signed integer logs and are checked directly against physical hardware;
general finite inputs use the pinned approximation profile. The independent
original source list is fyl2x-reference-sources.txt. Its oracle and replay
commands use x87_fyl2x / GAMEPKG_TAKEDAWN_X87_FYL2X_CORPUS and the corresponding
fyl2x tools. No original-CPU bit-for-bit qualification or player delivery is
claimed by these component checks.


FPATAN uses gamepkg_fpatan_evaluate for finite nonzero x/y and shared trans op2
for classification, pi constants, state and conditional pop. The unsigned
32v/u reduction keeps all carry bits with a quotient bounded0..32; table and
polynomial evaluation use the same fixed intermediate widths. Exact tiny
ratios receive a lower tail because atan(q)<q; tiny divisions are normalized
before evaluation to avoid early binary128 precision loss. This adaptation
corrects directed-rounding/underflow behavior in the original profile. The
physical oracle checks corrected dyadic inputs directly; other finite inputs
match the independently compiled original source, listed in
fpatan-reference-sources.txt, and MPFR512 verifies accuracy. FPATAN replay tools
use GAMEPKG_X87_FPATAN_CORPUS_SOURCE and x87_fpatan names. The pinned original
sources are unchanged; no complete CPU/game fidelity or player delivery is
claimed by these tests.


The gamepkg_fsincos adapter evaluates standalone FSIN/FCOS
(which0/1). Its logical64-bit words and integer192-bit restoring reduction use
the original66-bit pi/2 and standalone polynomial layout. Tiny sine/cosine
preserve a lower tail before final raw80 rounding, correcting the original
tiny fast path's directed-rounding/underflow behavior. General numerical results
match the independently compiled original standalone reference in
sincos-reference-sources.txt. MPFR checks strict local approximation bounds
against the legacy-reduced argument and a separate phase-drift bound against
true sin/cos; true large-argument results are not claimed to remain within one
ulp. The x87_sincos fixture/replay names group the two standalone instructions;
combined FSINCOS and FPTAN have separate APIs and qualification described below. Original source remains pinned
and unchanged. No player package or original-CPU bit-for-bit qualification is
implied by these checks.


Combined FSINCOS uses gamepkg_fsincos_pair, its original separate head/tail
layout and two explicit result structures; FPTAN uses gamepkg_fptan_evaluate
and its own folded sine/cosine division path. The host trans ops5/6 own stack
admission and publication. FSINCOS C1 comes from sine; the independent original
reference bridge observes it via a truncation reevaluation rather than the
original helper's final cosine flag. FPTAN keeps the pinned tiny profile and
retains a wider divide rounding hint only if final narrowing has no fraction.
Modern tiny tangent numerical/C1 variations are tracked, and both selected and
observed values pass independent MPFR error checks; exact exception/stack gates
remain. Tangent's full-function error bound includes cosine-denominator pole
amplification. The x87_trig_push fixture/replay names cover these two pushing
instructions. All original52 file pins and notices remain unchanged. Full
original-CPU/game qualification and signed player delivery are still open.


FYL2XP1 uses gamepkg_fyl2xp1_evaluate and shared trans op7. Its log1p path keeps
x separate near zero and delays both source exponents; exact1+x powers use
integer logs. Mathematical accuracy is qualified only within the Intel input
interval |x|<=1-sqrt(2)/2, with broader deterministic legacy behavior tested
separately. The fyl2xp1 reference bridge reuses pinned fyl2x.cc; original52 pins
remain unchanged. The shared adapter now retains a preceding wide multiply's
rounding direction if it lands on an exact raw80 result and narrowing has no
fraction, independently for wrapped exponents. Algebraic regressions cover
FYL2XP1/FYL2X and tiny F2XM1; no general F2XM1 reconstruction hint is introduced.
Reproduce with x87_fyl2xp1 tests and corresponding replay tools. Maintainer-only
MPFR/independent C++ inputs add no player dependencies. Full CPU/game fidelity
and signed player delivery remain open.
