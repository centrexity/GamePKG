# Reviewed binary32, raw80 and wider-intermediate SoftFloat subsets

This is reviewed host runtime code. It adds no guest crates or file/network
permissions, and contains no game code or assets. Sources are John R. Hauser's
[Berkeley SoftFloat3e](https://www.jhauser.us/arithmetic/SoftFloat.html),2018-01-20.
The [official archive](https://www.jhauser.us/arithmetic/SoftFloat-3e.zip) SHA256 is
`21130ce885d35c1fe73fc1e1bf2244178167e05c6747cad5f450cc991714c746`.
COPYING.txt retains the full BSD3-clause license; every selected source retains
its original notices. upstream-lock.json records78 file hashes and11 adaptation
entries, including two successive entries for the same multiword-shift file.
Rust tests verify all local pins.

The exact dependency selections are sources.txt (20 binary32 files),
ext80-sources.txt (25 additional raw80 files) and wide-sources.txt (26 additional
binary128 memory-arithmetic/conversion files). Together they select71 C sources;
headers and the license account for the remaining pins. All new selections were
checked against the same pinned official archive. Unimplemented declarations in
the upstream headers do not supply additional instruction implementations.

The generic C11 platform uses explicit word/byte order, eight-bit bytes,
thread-local library controls and standard32/64-bit integers. The8086-SSE
specialization specifies NaN behavior. SOFTFLOAT_FAST_INT64, CPU-specific
intrinsics and native128-bit/compiler-floating types are not enabled. Production
binary128 values are multiword integer containers. Host compiler __float128 and
long double occur only in the independent maintainer reference fixture.

Reviewed adaptations retain numerical behavior except where fixing undefined
behavior or an endian error:

- Binary32/raw80 square-root exponent arithmetic uses defined floor division
  instead of negative signed right shifts. Unused declarations/labels are
  removed, and multiword shift preconditions are explicit.
- Binary32 rounding observes destination-precision inexactness before range
  clamping, supporting unmasked SSE flags. Raw80 rounding exposes normalized
  unrounded96-bit inputs for C1 and wrapped exponent responses.
- Wider significand normalization uses an unsigned leading-bit constant. A
  conversion shifts it into bit31; the original signed-int shift is undefined.
- Both wider common-NaN converters access logical payload fields. Treating
  &v0 as the start of128 payload bits reads/writes past the object on big-endian
  hosts, where v64 precedes v0. Guard tests reject the original write on actual
  MIPS32 C under QEMU and pass the corrected converters. Round-trip and adjacent
  memory-independence checks accompany full numerical replay.

No warnings are suppressed. The wrappers preserve private rounding, tininess,
flags and raw80 precision. Guest MXCSR/CW interpretation, stack/tags, metadata,
source DE, deferred exceptions and commit/pop decisions belong to the separate
instruction services. The wider helper is only internal numerical workspace:
it does not add a guest SDK or complete a transcendental instruction. Its raw80
conversion returns numerical bits/IEEE flags, without a guest C1/wrapped-fault
commit contract. Reserved raw80 PC01 follows the measured64-bit-significand
profile; PC00/10/11 select24/53/64 bits. Physical legacy-CPU qualification remains
separate.

The wider helper passes660,646 independent GCC binary128 arithmetic/conversion
cases on Linux64 and ELF32, plus32,000 operations across eight worker threads.
Identical reference records replay on native/MIPS32 and GCC plus
Clang-emulated-TLS/GCC PE32 Wine profiles. Private and host rounding/flag state
are preserved. The39,638,772-byte LE corpus SHA256 is
`69e17da0921d78e2b862ad6b208342c4d3b0b3c140065ac6a6e6f99e68acf183`.
C long-double reference inputs canonicalize x87 pseudo-denormals to their equal
exponent1 representation; production accepts the original raw encodings.

Reproduction uses tools/check_takedawn_f128_portability.sh and
 tools/check_takedawn_f128_targets.sh from the repository root. The target
script requires a GCC multilib compiler for its independent ELF32 reference;
GAMEPKG_GCC32 selects it when it differs from the system default. Windows replay
contains no compiler floating types. Existing SSE/raw80 arithmetic, comparisons,
loads/stores, stack/control, scale/remainder and full runtime qualification are
recorded in the parent READMEs. Players need none of these reference tools or
Python. Component replay is not whole-game/platform or signed-launch evidence.
