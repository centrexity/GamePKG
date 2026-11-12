# Fixed SSE reciprocal approximation profile

`gamepkg_approx.c` adapts the reciprocal and reciprocal-square-root tables and
numerical classification from Bochs revision
`b0c88410501f48bbe1a966cab1c0592fb504915a`, `bochs/cpu/sse_rcp.cc`.
The complete original file and its notices are retained in `upstream/`;
`upstream-lock.json` pins it and the original license. Table values are unchanged.
The C adaptation replaces CPU/SoftFloat classification with raw binary32 integer
fields and replaces the signed exponent shift with unsigned arithmetic. It
uses no native floating point, compiler128, TLS state or player dependency.

The original is LGPL version2-or-later; this adaptation selects
LGPL-2.1-or-later. The full license is [upstream/COPYING](upstream/COPYING).
This component's license is separate from the root Apache license. Preserve
notices, corresponding source/build inputs, source access information and a
way to rebuild/relink a modified component with any incorporating binary.
The source is a separately compiled C translation unit. These checks do not
produce a signed player package or satisfy its eventual delivery obligations.

`../gamepkg_x86_approx.c` owns scalar/packed staging and input validation.
RCP/RSQRT ignore rounding/FTZ controls and do not alter MXCSR or raise numerical
SIMD exceptions. Denormals become signed zero inputs; zeros produce signed
infinity. NaNs preserve their payload/sign and become quiet. RCP infinity
becomes signed zero and tiny results flush to signed zero. RSQRT positive
infinity becomes zero and negative normal/infinite inputs become indefinite.
Scalar high lanes remain unchanged; all source lanes stage before any output.
Memory, alignment, CPU-mode/segment and scheduler behavior belong to the host.

This is a documented deterministic Bochs profile, not original physical Xbox/
Pentium III bit-exact qualification. Native AMD observations differ on finite
outputs; those differences are recorded separately and never substituted for
the exact profile assertions. Independent integer GMP endpoint certificates
prove the Intel relative error bound3/8192 for every normal mantissa bucket,
with RCP's specified tiny-output flush excluded. Monotonicity of positive x and
sqrt(x) bounds each constant-result bucket by its two endpoints; exact exponent
scaling and both RSQRT parities cover the remaining normal range.

Maintainer tests compile the unmodified original table/function bodies in C++
with separate classification adapters. They compare4,681,728 normal boundary/
midpoint/sign/exponent cases and44 specials, export explicit LE records, and
replay every result on other targets. GMP/C++ and native CPU instructions are
maintainer-only. The runtime and players need none of them. Source pins and
all six FS/address-size/F3 prefix orders are tested by `tests/approx.rs`.
Run `tools/check_takedawn_approx.sh` for host/ELF32/MIPS32 and PE32 compiler/TLS
qualification. Full CPU/game fidelity, signed launch and platform proof remain
required.

References: [pinned original](https://github.com/bochs-emu/Bochs/blob/b0c88410501f48bbe1a966cab1c0592fb504915a/bochs/cpu/sse_rcp.cc),
[Intel instruction reference](https://cdrdv2-public.intel.com/789581/325383-sdm-vol-2abcd.pdf).
