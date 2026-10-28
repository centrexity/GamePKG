# Native generator work

Historical experiment: the delivery path now reuses Takedawn’s native runtime
and ports its generated C game functions to Rust. See
[../rust-port/README.md](../rust-port/README.md). This replacement CPU/runtime
is not used by that integration.

Latest storage integration: the owned startup now creates/reopens its real
system-marked `TitleMeta.xbx` file and completes its metadata query. The reviewed
wide-format routine now runs, producing a44-byte metadata write request. Actual
file transfers remain unfinished. See the qualifications at the end of this
document. Complete gameplay remains open.

This host-only Rust tool implements the native frontend, typed integer/SSE
lowering, closed function control flow/calls and bounded streamed C generation.
The ordinary profile currently emits 23,956 functions; the controlled virtual-CPU
profile emits 23,959. Both build on three object targets, but neither replaces
the complete original pipeline or produces a playable GamePKG. Native discovery/recovery, remaining instructions/ABI, repair/patch
passes, production dispatcher and source-kit/build integration remain required. No
dummy game functions or successful-looking placeholders are emitted. Complete
GamePKG integration and measured optimization across all current ports remain
the goal, not just this component.

The executable and its build use Rust plus bundled native C Capstone; no Python
runtime is linked or invoked. Capstone0.14.0/capstone-sys0.18.0 pin C5.0.6, with
only x86/full/std features. This is reviewed host tooling, not a guest crate or
a change to the rule that game source cannot import crates. Original algorithm
attribution is retained in ../Xboxrecomp-LICENSE.txt; dependency notices are in
licenses/. No game executable, generated game C or game assets are included.

## Implemented frontend

- Admission through the same owned VerifiedDisc reader, all716 file hashes
  checked, then default.xbe read in memory and its exact pinned SHA256 checked.
  There is no extracted XBE path or bulk asset folder.
- Borrowed LE/u32 XBE header/section layout, retail/debug entry decoding,
  explicit code/data exceptions, backed virtual-to-file ranges and rejection of
  overflow, truncation, overlaps, bad names and entry/thunk addresses. BSS bytes
  never become bytes from the next raw section. This is the layout needed by
  the frontend, not yet the whole certificate/import/TLS analysis pipeline.
- Native instruction/operand model: registers, immediate normalization, memory
  widths, FS segment overrides, signed displacements, register writes, absolute
  references and call/branch targets.
- Linear decoding/resynchronization, reachable CFG worklists, stop addresses and
  mnemonics, plus basic blocks and successor edges. CFG/resync decode one
  instruction at a time so joins do not repeatedly decode unused tails; initial
  linear scans use bounded batches. None of this executes guest instructions.
- Bounded memory reservations and work:128MiB frontend command budget
  (192MiB for the dependency-graph diagnostic below),4MiB decoder
  context,16KiB maximum function bytes (pinned largest candidate13,416bytes),
  bounded per-function reservations,65,536 decode-work units, and16MiB/32,768
  limits for the explicit maintainer reference input. Count/hex address widths
  are enforced while parsing, not after an unbounded vector has been allocated.
  Input reads are capped on the owned file handle, including a file that grows.

The tool's inspect command reports native_generation_complete=false. Its
reference commands are diagnostic comparisons against private original metadata;
those addresses are not shipped or used as a player-side discovery shortcut.
Rejected extents are distinct report records, not silently skipped functions.

## Native data and stack lowering

`lower::data_instruction` emits bounded C scopes for integer MOV, MOVZX/MOVSX,
LEA, register XCHG, PUSH/POP, LEAVE, BSWAP, CDQ/CWD, CWDE/CBW and operand-free
NOP. Registers come from an explicit typed whitelist; printed disassembly and
synthetic expressions are never copied into C. Unsupported operations, widths,
segments and unqualified forms fail instead of emitting stubs. Exchange forms
route to the separate checked RAM RMW lowerer; string repeats have their own
instruction entry. This component expects the checked Decoder model, not
external metadata.

The emitter uses the pinned runtime's TLS register globals, local `ebp`,
`g_fs_base` and logical `xbox_MmioLoad/Store` interface. Partial registers preserve
unaffected bits, sign extension uses unsigned arithmetic, and effective addresses
wrap at32bits or16bits as encoded. FS is added after effective-address wrapping;
LEA ignores segment bases. PUSH snapshots its input before changing ESP; POP
evaluates an ESP-based destination after advancing the stack. The latter
semantics follow the [Intel instruction reference](https://cdrdv2-public.intel.com/671110/325383-sdm-vol-2abcd.pdf).
LEAVE respects operand size while retaining the32bit Xbox stack-address size.
EFLAGS are unchanged by these operations.

Twenty-one explicit architectural cases execute emitted C with strict warnings
and undefined-behavior checks on Linux64 and actual ELF32; the same synthetic C
also executes as an Intel80386 PE32 program in Wine. These tests use a small
logical little-endian memory service, not the entire Xbox memory subsystem.
They also compile against the unchanged pinned generic `recomp_types.h`
(SHA256 `a2b2d11ebb7851355c821f4029fde2ebe444390a730795b371881e939205f6dd`),
so the declarations and local-register convention are checked against the real
runtime. This is not big-endian engine or physical Windows qualification.
The data-stage gate passed nine Rust tests on Linux and eight on Windows, with
the separately cross-compiled C executable covering C execution there. The
expanded arithmetic suite below supersedes those total test counts.

The `reference-data-coverage` diagnostic walks the entire private reachable CFG
inventory and attempts each instruction, discarding emitted C immediately.
Current counts:375536 accepted data/stack instruction forms,434641 unsupported,
23999 backed functions and1 explicitly rejected upstream extent. Acceptance
measures emitter coverage, not whole-function correctness, reachability proof,
gameplay fidelity or performance. No game-derived C is saved by this diagnostic.
The arithmetic and closed-control-flow stages below extend this component.
Complete call/ABI handling, SSE/MMX/x87, strings/atomics, fault rollback/SEH,
recovery and all original patch/spin-wait passes still need implementation and
integration before native generation can replace the original tool.

## Native arithmetic and flags

`lower::integer_instruction` extends the data emitter with8/16/32bit
ADD/ADC/SUB/SBB/CMP/TEST/AND/OR/XOR/INC/DEC/NEG/NOT. It uses unsigned intermediates,
explicit carry/borrow and parity, and eager arithmetic bits in `g_eflags`.
INC/DEC preserve carry; NOT preserves every flag. Logical AF is architecturally
undefined, so its prior image is preserved and excluded from hardware equality
claims. Memory read/modify/write evaluates its address once and commits flags
after the store returns; LOCK/atomic variants remain rejected. General fault
rollback, runtime scheduling and SEH are still pending. Existing original
generated functions use lazy flags: their call/return transitions must be
bridged before mixing the new eager emitter with those functions.

A synthetic C oracle executes actual x86 instructions and captures hardware
flags. It compares1408000 cases: every pair of8bit binary inputs, all8bit unary
inputs, both carry states, and16/32bit boundary and deterministic random inputs.
Results, defined arithmetic flags, untouched virtual flags, high subregister
bits and source-register preservation are checked. Five additional cases cover
memory read/write counts, an ESP-based destination, FS memory and aliased
registers. The same fixture passes against the unchanged pinned runtime header
on Linux64 with UB checks, actual ELF32 with UB traps, and actual PE32 under
Wine. These are defined scalar-instruction checks, not full PIII emulation or
gameplay qualification. The arithmetic-stage gate passed twelve Rust tests on Linux and ten on Windows;
the separately cross-compiled C oracle supplies Windows hardware comparisons.

`reference-integer-coverage` reports500913 accepted instruction forms and309264
unsupported forms across the same810177 decoded instructions/23999 backed
functions, retaining the1 rejected reference extent. This is125377 additional
accepted forms over the data-only diagnostic. Counts do not establish actual
game reachability, complete functions, speed gains or complete native generation.
Shifts/multiply/divide, calls and complete control-flow/ABI, SIMD/x87, strings, atomics and
the remaining discovery/ABI/patch/runtime integration gates remain open.

Reproduce the hardware oracle (no player Python dependency):

```sh
GAMEPKG_TAKEDAWN_RUNTIME_HEADER=/tmp/gamepkg-xboxrecomp-audit/templates/runtime/recomp_types.h \
GAMEPKG_TAKEDAWN_ALU_FIXTURE=/tmp/gamepkg-takedawn-native-alu.c \
cargo test --offline --release --manifest-path engines/takedawn/native-gen/Cargo.toml --test alu_lower -- --nocapture
clang-18 -m32 -std=c11 -O3 -Wall -Wextra -Werror -fsanitize=undefined \
  -fsanitize-undefined-trap-on-error /tmp/gamepkg-takedawn-native-alu.c -o /tmp/gamepkg-takedawn-native-alu32
/tmp/gamepkg-takedawn-native-alu32
i686-w64-mingw32-gcc -std=c11 -O3 -Wall -Wextra -Werror \
  /tmp/gamepkg-takedawn-native-alu.c -o /tmp/gamepkg-takedawn-native-alu32.exe
WINEPREFIX=/tmp/gamepkg-steam-shortcuts-wine WINEDEBUG=-all \
  wine /tmp/gamepkg-takedawn-native-alu32.exe
```

The restricted maintainer sandbox blocks a32bit TLS system call for this
fixture; the actual ELF32 execution succeeds outside that restriction. The
default system GCC also lacks its32bit libgcc, so the ELF32 command uses the
available Clang18 runtime. Neither limitation adds a player dependency.

## Conditions and complete closed scalar leaves

The scalar emitter supports all16 flag conditions and their architectural
aliases for SETcc and16/32bit CMOVcc. The memory source of CMOVcc is loaded
even when its condition is false. Condition operations preserve EFLAGS.
The new flow::leaf checks decoded instruction extents, overlaps, entry and
all reachable edges before returning successful source. It emits explicit
labels/gotos for direct internal JMP/Jcc, JCXZ/JECXZ and LOOP/LOOPE/LOOPNE,
plus near RET/RET-imm stack cleanup. Address size selects CX/ECX;16bit counters
preserve the upper ECX half. LOOP and zero-counter branches preserve flags.
Operand-size16 IP branches, external/indirect transfers, calls, fallthrough to
undecoded bytes and unsupported instructions fail without a partial function.
This leaf-only API rejects direct external tails; the closed-program stage below
adds them for explicitly bound native targets. Indirect jumps and16bit-IP
transfers remain required.

The leaf follows the original C-call/guest-stack contract: its C caller has
already supplied a guest return-address slot; RET removes that slot and
returns to the C caller rather than dynamically dispatching the guest address.
Local EBP is initialized from and published back to the existing TLS bridge.
This is not the complete call, SEH, nonlocal-return or lazy/eager-flag ABI.
Output/work allocations are capped, charged to the caller's Budget and retained
by the returned source owner; failures release their reservations.

The synthetic C gate runs8192 condition/branch/counter cases plus a real
bounded summation loop, EBP stack frame/RET cleanup, false memory-CMOV load,
SETcc memory-write counts and high-byte destination cases. All64 combinations
of six arithmetic flags are checked against actual SETcc/CMOVcc instructions;
branch outcomes use those hardware results. Loop counters include zero,
one, wrapping and preserved-high-half cases. Linux64 uses explicit expectations
for CX-size loops, which that host mode cannot execute; actual ELF32 and PE32
execute the CX-size hardware oracle too. Hardware loop flags and virtual
flags/registers/guest stack cleanup are checked. The gate passes against the
unchanged pinned generic runtime header on Linux64 with UB checks, actual ELF32
with UB traps, and actual PE32 under Wine. The condition-stage gate passed fifteen
Rust tests on Linux and twelve on Windows, in addition to executing generated C.
No physical Windows, full Xbox-memory fault model or full-game proof is implied.

The native reference-leaf-coverage diagnostic accounts for all24000 private
reference candidates:23999 backed functions and1 rejected extent. It produces
complete closed source for1174 functions (4752912 aggregate C bytes) and rejects
22825 functions needing unsupported transfers/instructions/layout or resources.
Linux64 and the actual PE32 native generator produce byte-identical complete
reports (SHA256 cfda456723324e74a697cd26632cc32608df779e5daea1ed1e920b329f466dc2).
Generated game-derived C is immediately discarded, never saved by the command.
Emission acceptance does not prove original-game semantic equivalence, correct
recovery/patches, complete generation, playable packaging or optimization gains.

Reproduce the condition gate (only synthetic C is saved):

    GAMEPKG_TAKEDAWN_RUNTIME_HEADER=/tmp/gamepkg-xboxrecomp-audit/templates/runtime/recomp_types.h \
    GAMEPKG_TAKEDAWN_FLOW_FIXTURE=/tmp/gamepkg-takedawn-native-flow.c \
    cargo test --offline --release --manifest-path engines/takedawn/native-gen/Cargo.toml --test flow_lower -- --nocapture
    clang-18 -m32 -std=c11 -O3 -Wall -Wextra -Werror -fsanitize=undefined \
      -fsanitize-undefined-trap-on-error /tmp/gamepkg-takedawn-native-flow.c -o /tmp/gamepkg-takedawn-native-flow32
    /tmp/gamepkg-takedawn-native-flow32
    i686-w64-mingw32-gcc -std=c11 -O3 -Wall -Wextra -Werror \
      /tmp/gamepkg-takedawn-native-flow.c -o /tmp/gamepkg-takedawn-native-flow32.exe
    WINEPREFIX=/tmp/gamepkg-steam-shortcuts-wine WINEDEBUG=-all \
      wine /tmp/gamepkg-takedawn-native-flow32.exe

## Closed native programs and calls

Provisional direct-function emission now accepts an explicit set of native
eager-flag entries. CALL writes the actual32bit next-instruction address to the
guest stack, commits ESP after the write returns, publishes local EBP through
the existing TLS bridge, calls the generated C function and reloads EBP.
Callee RET/RET-imm performs guest stack cleanup. Direct external JMP/Jcc/loop
tails transfer the existing guest return slot without pushing another one.
They return to the original C caller after the target returns. No free-form
symbols, indirect dispatch or original manual/kernel/lazy-flag callees are
accepted. Operand-size16 calls/IP transfers remain rejected.

Program::build validates every supplied unit, rejects duplicate/overlapping
ranges and unresolved targets, then emits all declarations before definitions.
A successful program cannot contain a successful-looking stub for an invalid
callee. Source owners and planner/work allocations retain caller budget leases;
output errors are returned and staged-file cleanup remains the build caller's
responsibility. The direct-function API alone is provisional and cannot establish
that all its callees successfully compile or link.

The13-function synthetic program covers reading the actual guest return
address, argument passing/RET8 cleanup, recursive summation, unconditional and
conditional tails, carry across a call, guest callee-save sequences and EBP
changes visible to callers. A simulated failed return-address write leaves ESP,
EBP, registers, flags and original memory unchanged. This is an injected helper
failure, not full exception/SEH rollback qualification. The program executes
against the pinned runtime header on Linux64 with UB checks. Actual ELF32 with
UB traps and PE32 under Wine additionally pass130 comparisons against original
synthetic x86 CALL/RET/RET-imm/recursive/tail sequences running on their native
stack. Linux64 does not run that32bit original-stack oracle. A Windows warning
about an automatic test counter around longjmp was fixed by giving the counter
static storage; no warning suppression was added.

The dependency planner preserves resolved recursive cycles and removes missing
targets and their transitive callers using a bounded iterative reverse worklist.
It caps32768 nodes/262144 edges and retains a budget lease for its returned set.
Tests also reject invalid/missing/duplicate/overlapping units, exhausted budgets
and writer failures, verify lease release and prune a4096-node missing-target
chain without recursion. The direct-call-stage gate passed nineteen Rust tests on Linux and fifteen in actual
PE32 test binaries; strict release clippy and formatting pass.

At the direct-call stage, reference-program-coverage accounted for all24000 private
candidates:23999 backed,1 invalid extent.3851 functions provisionally emit with
6109 dependency edges;20148 fail emission. Removing2451 functions whose callees
cannot be emitted leaves1400 dependency-closed functions (6143207 aggregate
source bytes). No generated game C is saved. This graph result does not check
all cross-unit alias/layout/recovery rules or compile/link the game; both
program_ready and native_generation_complete remain false. Linux64 and actual
PE32 tool reports match byte-for-byte:
SHA2561694fba69e0a8275b5e807adb5f5465018af53edea720b5d74907baa36ca6eab.
The old leaf-only report remained byte-identical after adding direct calls.

The program diagnostic has a separate192MiB host-tool budget, including an8MiB
reservation for bounded metadata and a separately charged closure worklist.
This changes no signed game memory ceiling, generic ROM limit, guest dependency
rule or runtime permission. Native discovery/recovery, all original patches,
remaining integer/SIMD/x87/string/atomic instructions, kernel/manual dispatch,
SEH/nonlocal returns and eager/lazy-flag interoperability, full source-kit/build
integration and measured game optimization/fidelity/platform gates remain open.

Reproduce the closed-program C gate:

    GAMEPKG_TAKEDAWN_RUNTIME_HEADER=/tmp/gamepkg-xboxrecomp-audit/templates/runtime/recomp_types.h \
    GAMEPKG_TAKEDAWN_PROGRAM_FIXTURE=/tmp/gamepkg-takedawn-native-program.c \
    cargo test --offline --release --manifest-path engines/takedawn/native-gen/Cargo.toml --test program_lower -- --nocapture
    clang-18 -m32 -std=c11 -O3 -Wall -Wextra -Werror -fsanitize=undefined \
      -fsanitize-undefined-trap-on-error /tmp/gamepkg-takedawn-native-program.c -o /tmp/gamepkg-takedawn-native-program32
    /tmp/gamepkg-takedawn-native-program32
    i686-w64-mingw32-gcc -std=c11 -O3 -Wall -Wextra -Werror \
      /tmp/gamepkg-takedawn-native-program.c -o /tmp/gamepkg-takedawn-native-program32.exe
    WINEPREFIX=/tmp/gamepkg-steam-shortcuts-wine WINEDEBUG=-all \
      wine /tmp/gamepkg-takedawn-native-program32.exe

## Exact-bit SSE1 data and lane operations

The full instruction dispatcher now supports MOVSS, MOVAPS/MOVUPS,
MOVLPS/MOVHPS, MOVLHPS/MOVHLPS, ANDPS/ORPS/XORPS/ANDNPS, SHUFPS,
UNPCKLPS/UNPCKHPS and MOVMSKPS. It reads/writes the existing RecompXmm TLS
registers through unsigned32bit lanes; it does not convert values to host
floats. All eight Xbox XMM registers use an explicit whitelist. Register
sources and aliased shuffle operands are snapshotted before destination updates.
MOVSS memory loads clear the upper lanes while register copies preserve them.
Integer EFLAGS and host MXCSR remain unchanged by these operations.

Memory goes through the existing logical32bit MMIO load/store interface with
explicit lane order. Register updates occur only after all required loads
complete. Alignment-constrained legacy packed operations, including MOVAPS,
check the final guest linear address for16byte alignment before invoking any
memory callback. Proven constant aligned addresses omit the guard. Dynamic,
FS-relative and16bit effective addresses use the guest-GP service described
below. Unaligned MOVUPS and scalar/
half-vector transfers accept their valid dynamic address forms. FS base and
16bit effective-address wrapping use the existing checked address builder.
Multi-lane store fault rollback, full segment/address-span exceptions and
complete runtime memory/concurrency semantics remain to be qualified; these
logical helper checks do not establish them. SIMD arithmetic/MXCSR exceptions,
MMX/x87, SSE2 forms and streaming-store/fence semantics are not silently
emulated by this component.

The synthetic oracle passes44800 exact128bit/result/memory comparisons against
actual SSE1 instructions. It covers all256 SHUFPS controls with distinct and
aliased registers, signed zero, infinities, quiet/signaling NaN payloads,
subnormal and deterministic random bits, every byte alignment for legal
unaligned transfers, proven-aligned packed accesses and all eight register
names. Memory guard bytes and helper access counts are checked. MXCSR is tested
with clear/set sticky flags, four rounding controls and FTZ states; no floating
arithmetic is executed or qualified. Four injected failed loads preserve every
destination lane and virtual EFLAGS. A complete two-function generated program
passes vector bits across CALL/RET, checks memory output and the actual guest
return address, and preserves ESP/EBP/EFLAGS/MXCSR.

The unchanged pinned generic runtime header is used on Linux64 with UB checks,
actual ELF32 with UB traps and actual PE32 under Wine. All three execute the
hardware oracle; FS and16bit-address tests compare the computed logical linear
address against an ordinary hardware memory operand, not the host FS segment.
This is not big-endian runtime, physical Windows or full game qualification.
Twenty-two Rust tests pass on Linux and seventeen in the PE32 test binaries,
along with strict release lint and formatting. Players acquire no Python
runtime or new guest dependency.

The new full-inventory dependency report has12602 provisional functions,
11397 emission rejections and7586 edges. Pruning2722 functions with unavailable
transitive callees leaves9880 dependency-closed functions (11084345 aggregate
C bytes), up8480 from the direct-call stage. All23999 backed functions plus
the1 invalid extent remain accounted for. Linux64 and actual PE32 reports are
byte-identical, SHA256f31aaaded026c8a6ea4e9ce51dbfd6c26a9739bfe284dd57324d4ab4425d07e0.
No game-derived C is saved. Graph counts do not prove all cross-unit layout,
original recovery/patches, complete compilation/linking, gameplay fidelity,
playable packaging or an optimization gain. Both completion fields remain false.

Reproduce the current vector gate:

    GAMEPKG_TAKEDAWN_RUNTIME_HEADER=/tmp/gamepkg-xboxrecomp-audit/templates/runtime/recomp_types.h \
    GAMEPKG_TAKEDAWN_VECTOR_FIXTURE=/tmp/gamepkg-takedawn-native-vector.c \
    cargo test --offline --release --manifest-path engines/takedawn/native-gen/Cargo.toml --test vector_lower -- --nocapture
    clang-18 -m32 -msse -std=c11 -O3 -Wall -Wextra -Werror -fsanitize=undefined \
      -fsanitize-undefined-trap-on-error /tmp/gamepkg-takedawn-native-vector.c -o /tmp/gamepkg-takedawn-native-vector32
    /tmp/gamepkg-takedawn-native-vector32
    i686-w64-mingw32-gcc -msse -std=c11 -O3 -Wall -Wextra -Werror \
      /tmp/gamepkg-takedawn-native-vector.c -o /tmp/gamepkg-takedawn-native-vector32.exe
    WINEPREFIX=/tmp/gamepkg-steam-shortcuts-wine WINEDEBUG=-all \
      wine /tmp/gamepkg-takedawn-native-vector32.exe

Reproduce the instruction gate and retain only synthetic C for cross-compilation:

```sh
GAMEPKG_TAKEDAWN_RUNTIME_HEADER=/tmp/gamepkg-xboxrecomp-audit/templates/runtime/recomp_types.h \
GAMEPKG_TAKEDAWN_DATA_FIXTURE=/tmp/gamepkg-takedawn-native-data.c \
cargo test --offline --release --manifest-path engines/takedawn/native-gen/Cargo.toml --test data_lower
clang-18 -m32 -std=c11 -O3 -Wall -Wextra -Werror -fsanitize=undefined \
  -fsanitize-undefined-trap-on-error /tmp/gamepkg-takedawn-native-data.c -o /tmp/gamepkg-takedawn-native-data32
/tmp/gamepkg-takedawn-native-data32
i686-w64-mingw32-gcc -std=c11 -O3 -Wall -Wextra -Werror \
  /tmp/gamepkg-takedawn-native-data.c -o /tmp/gamepkg-takedawn-native-data32.exe
# Use an isolated maintainer Wine prefix, never player application storage.
WINEPREFIX=/tmp/gamepkg-steam-shortcuts-wine WINEDEBUG=-all \
  wine /tmp/gamepkg-takedawn-native-data32.exe
```

## Scalar shifts and rotates

The integer emitter now supports8/16/32bit SHL/SAL, SHR, SAR, ROL, ROR, RCL
and RCR with immediate or CL counts. Counts are masked to five bits; byte/word
carry rotates use their9/17bit rings and dword carry rotates use a33bit ring.
Full-width ROL/ROR still update carry when the masked count is nonzero, while
zero effective carry rotations retain their original value/carry. No count can
cause a C shift by or above its storage type's width. Arithmetic right shifts
use explicit unsigned sign filling, not implementation-defined signed shifts.

SHLD/SHRD support all masked dword counts and constant word counts whose masked
value is at most16. Dynamic word counts and oversized word constants remain
rejected until range analysis or the PIII-specific undefined-result behavior
is handled. Architecturally undefined AF/OF/CF values retain their previous
virtual image and are excluded from hardware-equivalence claims. The zero-count
case preserves every flag. Defined flags follow the
[Intel instruction reference](https://cdrdv2-public.intel.com/825743/325462-sdm-vol-1-2abcd-3abcd-4.pdf).
This is not physical-PIII qualification of undefined outcomes.

The destination address is evaluated once, the count is captured before an
aliased register can change, and flags commit after a successful memory store.
Memory RMW still reads and writes when the count masks to zero; it is not
turned into a no-op that bypasses writable-memory checks.

The synthetic C gate passes4205981 actual-x86 result/defined-flag/memory
comparisons. It covers every byte value and CL count, both initial carry and
overflow values, varied preserved flag bits, word/dword edge/random values,
double-shift source preservation and legal constant word counts. Native memory
RMW comparisons also check untouched high bytes and helper access counts.
Immediate encodings, ECX/CL aliasing and high-byte destinations have additional
assertions. Linux64 and actual ELF32 verify that a zero-count hardware memory
shift faults on a read-only page, using a bounded reaped child with core dumps
disabled. This host page-permission check is not a complete guest segment,
page-table or SEH-fault model. Windows runs the arithmetic/RMW oracle but does
not run that Linux-only page-fault gate.

Generated C compiles against the unchanged pinned runtime header on Linux64
with UB checks, actual ELF32 with UB traps and actual PE32 under Wine. All
three pass the4205981 comparisons. Twenty-five Rust tests pass on Linux and
nineteen in the actual PE32 test executables, with strict release lint and
formatting. CI now includes the native generator's synthetic suite without
requiring any game media; local oracle runs also use the pinned real header.

The current private inventory report has13300 provisional functions,10699
emission rejections and8873 edges. Removing2992 functions with unavailable
transitive callees leaves10308 dependency-closed functions/16985656 C bytes,
up428 functions from the SSE1-data stage. All23999 backed candidates and the
one invalid extent remain accounted for. Linux64/actual PE32 reports match
SHA2562bb4048231d2ca01f5d28482039f7536447a82d9cb9393518e23b2621dc3bfa6.
Generated game-derived C is discarded by the diagnostic. These counts do not
compile/link the game or prove alias recovery, original patches, runtime ABI,
full game fidelity or an optimization gain. Complete streamed source-kit
assembly, remaining instructions, original discovery/recovery/patches,
kernel/manual/indirect dispatch, memory/SEH/nonlocal-return and eager/lazy-flag
interoperability, signed GamePKG integration and measured per-port/platform
qualification remain required. Both completion fields remain false.

    GAMEPKG_TAKEDAWN_RUNTIME_HEADER=/tmp/gamepkg-xboxrecomp-audit/templates/runtime/recomp_types.h \
    GAMEPKG_TAKEDAWN_SHIFT_FIXTURE=/tmp/gamepkg-takedawn-native-shift.c \
    cargo test --offline --release --manifest-path engines/takedawn/native-gen/Cargo.toml --test shift_lower -- --nocapture
    clang-18 -m32 -std=c11 -O3 -Wall -Wextra -Werror -fsanitize=undefined \
      -fsanitize-undefined-trap-on-error /tmp/gamepkg-takedawn-native-shift.c -o /tmp/gamepkg-takedawn-native-shift32
    /tmp/gamepkg-takedawn-native-shift32
    i686-w64-mingw32-gcc -std=c11 -O3 -Wall -Wextra -Werror \
      /tmp/gamepkg-takedawn-native-shift.c -o /tmp/gamepkg-takedawn-native-shift32.exe
    WINEPREFIX=/tmp/gamepkg-steam-shortcuts-wine WINEDEBUG=-all \
      wine /tmp/gamepkg-takedawn-native-shift32.exe

## Integer multiplication

Native integer lowering now implements unsigned MUL and signed IMUL, including
implicit8/16/32bit operands/results and word/dword two- and three-operand IMUL.
All inputs are captured before implicit AX/DX/EAX/EDX or explicit destinations
change. Signed operands are normalized into int64 with explicit subtraction;
signed32 products fit int64, and converting a negative product to uint64 is
defined. Result extraction uses unsigned shifts and masks. There are no signed
overflow assumptions, out-of-range signed narrowing or signed right shifts.
Immediate8 values use the decoder's sign-extended value before width masking.

CF/OF describe whether the complete product fits the destination according to
the signed/unsigned instruction form. Undefined arithmetic flags retain their
prior image; this is not a claim about PIII-specific undefined flag values.
Instruction forms and defined flag behavior follow the
[Intel instruction reference](https://cdrdv2-public.intel.com/671110/325383-sdm-vol-2abcd.pdf).
Memory operands are read once even when the multiplier is zero; there is no
guest-memory destination. Load failure precedes every register/flag commit.
LOCK/repeat prefixes, unsupported segments, synthetic expressions and invalid
forms/widths are rejected. DIV/IDIV now use the checked division and integer
fault boundary described below; complete guest SEH/recovery remains pending.

The synthetic C fixture runs1,760,104 actual hardware product/defined-flag
comparisons, plus5 address/alias and10 injected-load-failure checks. It exhausts
byte input pairs and exercises AL/AH and implicit accumulator/high-register
aliases, word/dword boundaries and random inputs, full and truncated products,
positive/negative short/full immediates, prior flag images, preserved upper
halves/source registers, unaligned memory, FS and16bit addresses. Memory reads
and absence of writes are asserted. Faults cover every supported memory form;
this is the host helper's injected failure contract, not full guest SEH proof.
All1,760,119 checks pass on Linux64 with undefined-behavior checks, actual ELF32
with undefined-behavior traps, and actual PE32 under Wine using the pinned
runtime header. This does not qualify physical Windows or complete game play.

```sh
GAMEPKG_TAKEDAWN_RUNTIME_HEADER=/path/to/pinned/templates/runtime/recomp_types.h \
GAMEPKG_TAKEDAWN_MULTIPLY_FIXTURE=/tmp/gamepkg-native-multiply.c \
  cargo test --offline --release --manifest-path engines/takedawn/native-gen/Cargo.toml \
  --test multiply_lower -- --nocapture
clang-18 -m32 -std=c11 -O3 -mno-red-zone -Wall -Wextra -Werror \
  -fsanitize=undefined -fsanitize-undefined-trap-on-error \
  /tmp/gamepkg-native-multiply.c -o /tmp/gamepkg-native-multiply32
/tmp/gamepkg-native-multiply32
i686-w64-mingw32-gcc -std=c11 -O3 -Wall -Wextra -Werror \
  /tmp/gamepkg-native-multiply.c -o /tmp/gamepkg-native-multiply32.exe
wine /tmp/gamepkg-native-multiply32.exe
```

The supported dependency-closed selection increases by175 functions to10,483:
13,539 provisional functions,10,460 emission failures,9,342 dependency edges,
3,056 removed dependent callers and the same1 rejected DSOUND extent.
The partial module includes111 shared-tail entries and19,518,462 body/declaration
bytes. Linux64 and actual Windows32 emit identical source: body SHA256
`8dd594ea93c7174715226b08b149f9529aadc814c645a7d6a7dd73f719318722`,
complete-file SHA256
`cb7596bc307505338125cf77a405f432f9061687f8495fca76a5e99514f39c01`.
The complete partial module compiles at O3/strict warnings into ELF64, ELF32 and
i386 COFF, each with10,483 definitions and no unresolved generated-function
symbols. Runtime helpers remain external and no game functions are executed by
this object gate. The generation report SHA256 is
`2cd549b89e83defe7ecdfbd408b11d74aa156ca76105cdb93650e760218af32b`.
One Linux run including disc verification/planning measured3.11seconds and
16,684KiB peak RSS; no gameplay speedup follows from that measurement. All30
host tests (with pinned header and with default CI declarations),23 Windows-tool
tests, strict clippy and formatting pass. Prior race/save-tested game binary
and actual player library are unchanged; generated game C/objects stay private.
Native discovery/recovery, remaining instruction/ABI/patch/runtime/build work,
full fidelity/platform verification and heavy measured optimization across
every current port remain open.

## Checked division and integer fault boundary

DIV/IDIV now support byte, word and dword register/memory sources. The divisor
and double-width implicit dividend are captured before architectural writes.
All host division is unsigned and guarded against a zero divisor. Signed
division uses unsigned magnitudes and explicit quotient signs/ranges, including
the full64bit minimum and negative-one case, without invoking C signed division
overflow. Quotients and signed remainders commit only after representability
checks; byte results pack AL/AH, and word results preserve upper register bits.
Undefined arithmetic flags retain their virtual image, without claiming PIII
undefined behavior. Defined forms/faults follow the
[Intel instruction reference](https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-vol-2a-manual.pdf).

Zero divisors and oversized quotients call the concrete, non-returning host
service in runtime/gamepkg_x86_fault.c with the faulting guest EIP/current local
EBP. It records vector0 and integer state and exits translated C frames through
the nearest nested thread-local entry boundary. The entry returns a distinct
fault status, never a successful quotient or a return through following guest
instructions. Host integration must enter through gamepkg_x86_run and handle
the fault status. Complete guest SEH dispatch/recovery, memory/FP faults,
floating exception context, nonlocal exits and final player build integration
remain required; see runtime/README.md. This is a working integer fault
transport contract, not a complete Xbox exception emulator.

The synthetic fixture exhausts all65,536 byte dividends against all256 byte
divisors for both signednesses. Additional cases cover word/dword boundary
triples and random values, CL/memory and AL/AH/AX/EAX/DX/EDX aliases, partial
register preservation, signed quotient/remainder rules, flags, source memory
counts, zero divisors and overflow without partial writes. On each of Linux64,
actual ELF32 and actual PE32 Wine it passes12,633,635 normal actual-hardware
result comparisons and21,133,789 checked guest-fault cases. Linux64/ELF32 also
confirm15 real x86 SIGFPE cases in reaped children with core dumps disabled;
Windows does not run that physical-fault oracle. Host/ELF32 C undefined behavior
checks pass. Overflow cases include a positive double-width maximum divided
by one and the signed double-width minimum divided by negative one.

Nested generated CALL/RET tests verify the original guest return slot and ESP,
precise callee fault EIP and propagated local EBP, uncommitted quotient/remainder
and no subsequent callee/caller instructions. Nested host boundaries restore
their outer frame, normal execution after faults succeeds, and8 simultaneous
threads each preserve1,000 independent fault contexts.

```sh
GAMEPKG_TAKEDAWN_RUNTIME_HEADER=/path/to/pinned/templates/runtime/recomp_types.h \
GAMEPKG_TAKEDAWN_DIVISION_FIXTURE=/tmp/gamepkg-native-division.c \
  cargo test --offline --release --manifest-path engines/takedawn/native-gen/Cargo.toml \
  --test division_lower -- --nocapture
clang-18 -m32 -std=c11 -O3 -mno-red-zone -pthread -Wall -Wextra -Werror \
  -fsanitize=undefined -fsanitize-undefined-trap-on-error \
  -Iengines/takedawn/native-gen/runtime /tmp/gamepkg-native-division.c \
  engines/takedawn/native-gen/runtime/gamepkg_x86_fault.c \
  -o /tmp/gamepkg-native-division32
/tmp/gamepkg-native-division32
```

The supported selection increases another146 functions to10,629, including116
validated shared-tail entries:13,688 provisional functions,10,311 emission
failures,9,593 edges,3,059 removed dependent callers and the same1 rejected
DSOUND extent. Full body/declarations total22,248,034bytes, SHA256
`ec0a04181774de16b6ca69442c2190c33788f139e0e7ce15a66d60b329f3949e`;
complete-file SHA256
`a31096adaed54896dca714b5b3e594f153047188786d74f10b030a4cd0cc8b8d`.
Linux64/actual Windows32 source outputs match exactly. O3/strict-warning ELF64,
ELF32 and i386 COFF objects relocatably link the real fault service, each with
10,629 generated definitions and no unresolved generated-function or fault
service symbols. Other helpers remain external; no complete game is linked or
executed by this gate. The private generation report SHA256 is
`a61d647c802cf5aa5a1530beecb4743e0220a57522aa974bfa56ee24a10c1964`.
One Linux verification/planning/generation run measured3.16seconds and16,820KiB
peak RSS, not gameplay performance. All33host tests with actual/default headers,
25Windows-tool tests and strict lint/formatting pass. The earlier race/save-tested
game binary and player library remain unchanged, and private generated C stays
out of public packages. Full game generation/integration, broader fidelity and
platform gates, and heavy measured optimization across every port remain open.

## SSE1 scalar comparisons and guest MXCSR

COMISS/UCOMISS and LDMXCSR/STMXCSR now have typed native lowering and concrete
host services. Guest control state is thread-local, initially0x1f80, restricted
to the Pentium III mask0xffbf. DAZ and AMD's misaligned-SSE control are not
inherited from the host. Invalid reserved bits deliver a general-protection
fault before modifying guest MXCSR; a successful control load does not itself
raise an exception for previously set sticky flags. Logical m32 loads/stores
are used, including unaligned scalar addresses. Unsupported prefixes, widths,
segments, registers and free-form expressions remain rejected.

The comparator uses only integer binary32 classification/order, with exact
signed-zero, infinity, subnormal and quiet/signaling-NaN behavior. COMISS versus
UCOMISS invalid conditions, NaN/denormal precedence, sticky status, mask handling
and defined EFLAGS match hardware. No host float casts, host rounding changes
or host NaN handling enter the implementation. Unmasked conditions update
guest MXCSR then report vector19 without changing EFLAGS or XMM state. The
fault record now captures MXCSR for all transported faults. Full SIMD/x87
context, guest SEH recovery, CR0/CR4 mode selection, remaining arithmetic and
conversion semantics and legacy-helper control-state bridges remain open.
This is the SSE-enabled/OS-SIMD-exceptions-enabled profile, not a full control
register emulator. Architecture references are the
[Intel manual](https://cdrdv2-public.intel.com/843827/253665-sdm-vol-1-dec-24.pdf)
and [AMD manual](https://www.amd.com/content/dam/amd/en/documents/processor-tech-docs/programmer-references/40332.pdf).

Synthetic cases pass on Linux64 with UB checks, actual ELF32 with UB traps and
actual PE32 under Wine:537,088 hardware comparisons across special/random bits,
both comparison types, register/memory sources,4 rounding modes, FTZ on/off and
varied prior flags/status;32,768 valid MXCSR load/store round trips;6,400 mask
combinations;6 explicit unmasked SIMD faults;17 guest-reserved-bit cases;
injected read/write failures and priority; and8threads with1,000 independent
state iterations each. Exact host MXCSR, guest upper XMM lanes/source registers,
untouched flag bits, memory access counts and failed-operation state are checked.

Linux64/ELF32 capture actual kernel signal contexts from6 SIMD faults and15
host-reserved control-bit faults: vector, post-exception MXCSR, unmodified
arithmetic flags for SIMD faults, and unchanged MXCSR for invalid loads. These
children are reaped with core dumps disabled. Windows does not run this physical
fault oracle. The host's FXSAVE mask is0x0002ffff: DAZ and AMD MM load normally
there but are still rejected by the guest. Physical fault selection follows
that advertised host mask instead of incorrectly assuming every host matches
the old Xbox CPU. No guest acceptance rule is loosened to make the oracle pass.

```sh
GAMEPKG_TAKEDAWN_RUNTIME_HEADER=/path/to/pinned/templates/runtime/recomp_types.h \
GAMEPKG_TAKEDAWN_FLOATING_FIXTURE=/tmp/gamepkg-native-floating.c \
  cargo test --offline --release --manifest-path engines/takedawn/native-gen/Cargo.toml \
  --test floating_lower -- --nocapture
clang-18 -m32 -msse2 -std=c11 -O3 -mno-red-zone -pthread -Wall -Wextra -Werror \
  -fsanitize=undefined -fsanitize-undefined-trap-on-error \
  -Iengines/takedawn/native-gen/runtime /tmp/gamepkg-native-floating.c \
  engines/takedawn/native-gen/runtime/gamepkg_x86_fault.c \
  engines/takedawn/native-gen/runtime/gamepkg_x86_sse.c \
  -o /tmp/gamepkg-native-floating32
/tmp/gamepkg-native-floating32
```

The full supported selection is now10,660 functions with117 validated shared
tails:13,735 provisional functions,10,264 emission failures,9,636 edges,3,075
dependency-removed callers and1 rejected DSOUND extent. Body/declarations total
22,508,598bytes, SHA256
`eafb1451d368666a6c3f7608ec662154d5400d30868029d19db97cfcf1dadc5d`;
complete-file SHA256
`6e27b6fe48c3772ce25c4a3161171ea26432cb2856b30999af0b9e905820566f`.
Linux64/actual Windows32 emit identical source. O3/strict-warning ELF64, ELF32
and i386 COFF modules relocatably link the concrete comparison/control/fault
services:10,660 definitions, all8 service symbols and no unresolved generated
or gamepkg_x86 service calls. Other runtime dependencies remain external;
this does not execute or fully link the game. Full report SHA256 is
`214809d2a6d0a1ad172308b8b527490e15eb6d18cc7aadcc288e7f29e467840c`.
One Linux verification/planning/generation run measured3.17seconds and16,896KiB
peak RSS, not gameplay performance. All36host tests with actual/default header
declarations,27Windows-tool tests and strict clippy/formatting pass. Earlier
race/save-tested game binary, owned ISO and actual player library are unchanged.
Private generated C/objects stay private. Complete game generation/integration,
remaining fidelity/platform requirements and heavy measured optimizations
across every current port remain unfinished.

## Portable SSE1 arithmetic

ADDSS/ADDPS, SUBSS/SUBPS, MULSS/MULPS, DIVSS/DIVPS and SQRTSS/SQRTPS now emit
typed calls to a concrete binary32 host service. Scalar destinations retain
upper lanes and packed operations stage all lanes before committing. Register,
same-register, scalar and alignment-checked packed memory forms are
supported. Dynamic packed alignment uses the guest-GP bridge below. Invalid prefixes, widths,
registers and synthetic source expressions do not become C.

The service uses a reviewed20-file SoftFloat3e subset with SSE NaN semantics,
integer arithmetic and C11 TLS, with no native C float or host control changes.
Its BSD3 notice, source pins, generic platform and two adaptations are in
runtime/softfloat/. One adaptation defines negative-exponent floor division;
the other observes unbounded-exponent rounding loss without changing results.
The guest crate ban is unchanged. Packed preconditions aggregate before every
calculation; invalid/NaN and divide-by-zero take precedence over a denormal in
the same lane. Unmasked preconditions stop before postconditions. Masked gradual
underflow/FTZ, exact tiny unmasked results and per-lane overflow/underflow/
precision status match tested hardware. Results do not commit on fault. All
modified library state restores before normal return or fault transport.
The exception staging follows the
[Intel architecture manual](https://cdrdv2-public.intel.com/843827/253665-sdm-vol-1-dec-24.pdf).

Linux64 UB checks, actual ELF32 UB traps and actual PE32 Wine pass718,080 direct
and218,880 generated-instruction comparisons across all opcodes/widths, edge and
random bits, rounding/FTZ settings, register/memory/same-register sources and
upper/source preservation. There are30 emitted unmasked fault cases and a failed
source-load gate. Eight threads each run1,000 hardware comparisons with private
guest/library state. Linux64/ELF32 additionally compare2,640 physical kernel
outcomes:1,648 faults/992 completions across all64 mask combinations at default
rounding and representative masks at other rounding/FTZ settings. Exact
post-exception MXCSR and unchanged destinations are checked. Windows does not
run Linux signal-context capture.

An optional49,008,972-byte explicit LE corpus contains720,720 x86 observations,
including those2,640 physical outcomes. Native64, actual PE32 Wine and a static
ELF32 MSB MIPS executable under QEMU match every result, status and fault.
The MIPS consumer enforces32bit pointers/big-endian byte order, uses no x86/native
float operations, and validates fault EIP/EBP and flag preservation. Corpus SHA:
`a685776988efecf84c09edf6163f4ea8dc172d9d121a03d1a0a6a234e00f7504`.
This is component qualification, not full MIPS gameplay or console equivalence.

```sh
GAMEPKG_TAKEDAWN_RUNTIME_HEADER=/path/to/pinned/templates/runtime/recomp_types.h \
GAMEPKG_TAKEDAWN_ARITHMETIC_FIXTURE=/tmp/gamepkg-native-arithmetic.c \
GAMEPKG_TAKEDAWN_ARITHMETIC_CORPUS=/tmp/gamepkg-native-arithmetic.bin \
  cargo test --offline --release --manifest-path engines/takedawn/native-gen/Cargo.toml \
  --test sse_arithmetic -- --nocapture
GAMEPKG_ARITHMETIC_CORPUS_SOURCE=/tmp/gamepkg-native-arithmetic.bin \
  bash tools/check_takedawn_arithmetic_portability.sh
```

The script can regenerate its own corpus on a maintainer's x86_64 Linux host.
MIPS/QEMU are test dependencies, not player requirements; no Python is used.
CI reuses the ordinary semantic suite's corpus and adds the real big-endian
target. The exact script passes locally using the existing cross compiler and
a checksum-verified Ubuntu QEMU package unpacked privately; no host packages
were installed. Remote CI execution is not claimed.

The supported selection increases5,681 functions to16,341 with146 validated
shared tails:19,554 provisional functions,4,445 emission failures,10,116 edges,
3,213 dependent callers removed and1 rejected DSOUND extent. Body/declarations
total30,204,855bytes, SHA256
`b7461b7873c3ba1e7f4f38e7666d935d5ce87bdff80c33956f78466894d9a2b9`;
complete-file SHA256
`6bc39fc85eed792e4c9f8b449f9148250f08f034b39b79c0c5b394f560eb672c`.
Linux64/actual Windows32 source matches. O3/strict-warning ELF64/ELF32/i386 COFF
modules relocatably link all concrete services/selected SoftFloat functions:
16,341 definitions with no unresolved generated, gamepkg_x86, SoftFloat or
observation symbols. Other helpers remain external; no full game is linked or
executed by this object gate. Full report SHA256:
`e6ffe0ad0c5930d9ca759add43447433fce21788c766dbae176fd452ffc75f3e`.
One Linux verification/planning/generation run measured3.30seconds/16,980KiB peak
RSS, not gameplay speedup. All40host/30Windows-tool tests and strict checks pass.
The earlier race/save-tested game binary, actual library and owned ISO are
unchanged. Game-derived C/objects stay private. Discovery/recovery, remaining
instructions (including other SSE/MMX/x87), full exception/ABI/patch/dispatch/
source-kit integration, broader fidelity/platform checks and heavy measured
optimization across every port remain unfinished. This portable correctness
baseline is not a gameplay performance or all-ports optimization claim.

## Bounded streamed source and compiled partial module

`program::write_streamed` validates all selected functions in a first pass,
retaining only typed entries and input/source hashes, then regenerates and
writes one function at a time. Input bytes and emitted body hashes must agree
between passes. The output quota includes all declarations and bodies. Source
and decoded-function storage do not accumulate with function count. The existing
in-memory `Program` remains available for small nonoverlapping programs.

Nested entry ranges sharing an end can describe real shared tails. The streamed
builder verifies every reachable shared byte and instruction boundary using two
bounded byte maps for the current cluster. Duplicate entries, crossing ranges,
conflicting bytes and entries inside an existing instruction are rejected.
No alias or function is silently discarded. This establishes consistent static
decoding, not original discovery/reachability or complete ABI recovery.

The explicit maintainer diagnostic accepts:

```sh
private_dir=$(mktemp -d)
engines/takedawn/native-gen/target/release/gamepkg-takedawn-native-gen \
  reference-source /path/to/supported.iso /path/to/private/functions.json \
  "$private_dir/supported.c"
```

Output must be a new file in a private temporary subdirectory. On Unix the
directory must deny group/other access. A staged file is synchronized and
published without replacing an existing file only after successful generation;
errors discard the stage. The API's caller owns staged cleanup on I/O failure
or changed input. This command emits proprietary executable-derived code for
local diagnostics only: do not commit or publish the output. The eventual
player path still needs native discovery rather than the reference inventory.

On 2026-10-21 the full supported dependency-closed selection emitted10,308
functions, including111 validated shared-tail entries. Body plus declarations
total17,243,356bytes, SHA256
`5921e7b2273ee1654ca7f83994bd7aba781f3fe224c700aad54533b845451fb9`.
Including the fixed runtime-header include, source SHA256 is
`73c6626cc0183b62224215de784a0ea2aa01558b70fd4bfba0de92546b65b367`.
Linux64 and an actual i386 Windows generator under Wine emit byte-identical
source. Windows uses its actual private Windows Temp directory; an output
outside that temporary root is rejected.

The complete partial module compiles with O3, strict warnings and the unchanged
pinned `recomp_types.h`: Clang18 ELF64 and ELF32, and MinGW GCC i386 COFF. Each
object contains exactly10,308 defined native functions and no unresolved
generated-function symbols. Runtime helpers remain external; this is object
compilation, not a full linked game or an execution/fidelity test. Generated C
and objects stay private. A repeated Linux generation, including716-file disc
verification and all reference planning, measured3.15seconds and16,732KiB peak
RSS with `/usr/bin/time`; this is one tool run, not a gameplay speedup claim.

All27 host tests and21 actual Windows-tool tests pass. Synthetic streamed output
matches the old builder exactly and therefore runs through the existing native
execution fixtures. Additional tests cover256 functions under an8MiB budget,
hash/size accounting, shared-tail conflicts, malformed ranges, changed input,
short/missing loads, quotas, writer failures and lease release. Strict clippy
and formatting pass. CLI negative checks preserve existing source and reject
output at the temporary root itself. Native game generation and readiness
remain explicitly false: the10,699 unsupported functions,2,992 removed dependent
callers, rejected DSOUND extent, discovery/recovery, full ABI/instruction/patch
pipeline and signed GamePKG integration remain unfinished.

## Evidence

All17 sections match the independent pinned original XBE parser. All24,000
reference candidates are accounted for:

| Reference pass | Backed functions | Matching instructions | Matching blocks |
| --- | ---: | ---: | ---: |
| Linear | 23,999 | 903,343 | 178,231 |
| Reachable CFG | 23,999 | 810,177 | 161,530 |

Every instruction/operand field and every block/successor list is compared by
canonical JSON SHA256. Both complete reports are byte-identical on Linux64 and
an actual Intel80386 PE32 tool under a private Wine prefix. This qualifies the
frontend, not Takedawn32-bit gameplay or physical Windows hardware. Six synthetic
tests pass on both widths, covering malformed XBE layout, retail/debug decoding,
BSS/scope rejection, operands/register effects, branches/calls/loops, CFG/block
edges, resynchronization, stop instructions, metadata limits and budget lifetime.
Strict release clippy and formatting pass; the reused media library's15 tests
and strict lint pass too.

One candidate,0x0030002F..0x00300D00, extends past DSOUND's stored bytes into its
zero-filled tail. The native tool rejects the full range. The original raw-byte
helper checks the starting mapping and whole-file extent, which can read later
file bytes as this section's tail. The candidate is marked a tail-jump alias;
its stored called_by list is empty, but that alone does not prove it unreachable.
Its true reachability/boundary must be resolved by native discovery/recovery
before generation can be complete. It is not counted as a generated function.

The maintainer differential uses the original Python oracle only for testing.
Both tools share Capstone, so matching is evidence for the model/CFG port, not
independent proof of Capstone's instruction semantics or original analysis.
Reported execution times include different work and are not gameplay or
all-ports optimization claims. The previously race/save-tested private game
binary is unchanged by this frontend work.

## Reproduce

From the repository root:

```sh
cargo test --offline --release --manifest-path engines/takedawn/native-gen/Cargo.toml
cargo clippy --offline --release --all-targets --manifest-path engines/takedawn/native-gen/Cargo.toml -- -D warnings
engines/takedawn/native-gen/target/release/gamepkg-takedawn-native-gen inspect /path/to/supported.iso
```

For a maintainer with the pinned original toolkit and full private reference
inventory from the original generator:

```sh
python3 tools/check_takedawn_native_decode.py \
  --native "$PWD/engines/takedawn/native-gen/target/release/gamepkg-takedawn-native-gen" \
  --disc /path/to/supported.iso --toolkit /path/to/pinned/xboxrecomp \
  --functions /path/to/private/build/xr/disasm/functions.json --mode linear
```

Repeat with --mode cfg. The native executable itself has no Python dependency;
this explicit maintainer-only oracle is not part of the eventual player build.
Private comparison reports and reference metadata stay out of public packages.

## Dynamic SSE memory alignment

MOVAPS loads/stores, packed logic, shuffle/unpack and packed arithmetic now
accept dynamic addresses. The emitter evaluates the guest address once, applies
32bit wrapping,16bit effective-address wrapping and FS addition, then checks
the final linear address before any load/store or arithmetic callback. A
misalignment delivers vector13 with the instruction address and the live local
EBP through the existing mandatory host fault boundary. XMM destinations,
memory, EFLAGS and guest MXCSR remain unchanged. Constant aligned addresses
retain their guard-free path; MOVUPS/scalar/half-vector unaligned access remains
supported. This does not implement page/segment-span validation or rollback of
a multi-lane store that faults after an earlier lane was committed.

The asset-free `sse_alignment` fixture checks14 operations across six address
forms and all16 low address bits:1,344 generated-code fault/result checks.
Aligned results match physical SSE instructions. Every misaligned case checks
zero memory callbacks and unchanged state. Linux64 and actual ELF32 also
inspect420 physical x86 faults, including missing-page addresses, and require
GP vector13/error0 instead of a page fault. The same fixture passes as PE32
under Wine (the physical fault oracle is Linux-only). The complete generator
suites pass42 host tests and31 Windows-tool tests; strict release lint and
formatting pass.

The private owned-media selection increases302 functions to16,643, with150
validated shared tails:19,998 provisional functions,4,001 emission failures,
11,100 dependency edges,3,355 dependency-removed callers and1 rejected extent.
The35,559,610-byte body/declaration stream has SHA256
`3d3b9625b59c5f5354cd8934d95929d6e4a9c3dcf19f92ac887655f5db625d87`;
complete private source SHA256
`8662262b48053dbc4252f00c0097755f98e41ae8977bd690bac3ce9f227a444e`.
It compiles at O3 with strict warnings against the pinned generic runtime header
on ELF64, ELF32 and i386 COFF. Relocatable links on each target contain16,643
native function definitions and resolve the generated, fault, SSE and SoftFloat
services. Kernel/MMIO/host integration remains external. Game-derived source
stays private; original game code/assets are not published. These counts do
not prove final executable linking, game fidelity, full native generation or
optimization gains. Both readiness fields remain false.

To export only the synthetic fixture for32bit compiler checks:

```sh
GAMEPKG_TAKEDAWN_ALIGNMENT_FIXTURE=/tmp/gamepkg-alignment.c \
  cargo test --locked --offline --release \
  --manifest-path engines/takedawn/native-gen/Cargo.toml --test sse_alignment
```

Compile the exported fixture with runtime/gamepkg_x86_fault.c,
runtime/gamepkg_x86_arithmetic.c and the20 pinned SoftFloat sources, using the
same include paths and C11/O3 settings as the arithmetic fixture above. Use
Clang -m32 with UB traps for ELF32, or i686-w64-mingw32-gcc for PE32 and run it
under an isolated maintainer Wine prefix. Players acquire no Python dependency.

## Scalar signed32/binary32 conversions

CVTSI2SS, CVTSS2SI and CVTTSS2SI now use a concrete integer-only host service
in runtime/gamepkg_x86_sse.c. The pinned game inventory contains981,4 and372
instances respectively. Typed lowering admits only their SSE1 F3 prefix,
32bit GPR/memory and approved XMM forms. Integer source values are unsigned
bit patterns; no host signed overflow, out-of-range float cast, native float
rounding mode or additional runtime dependency is involved.

The service implements all four guest rounding directions, ties to even,
truncation independent of rounding control, signed32 minimum, masked invalid
results and IE/PE status. These conversion instructions do not signal a denormal
operand exception; subnormals are converted with the ordinary rounding rule,
and FTZ does not alter an integer result. The distinction is documented in
[Intel's SIMD exception reference](https://cdrdv2-public.intel.com/819710/252046-sdm-change-document.pdf),
section11.5.2.2, and checked independently against actual instructions. The
service raises any unmasked condition before returning, so generated code
commits only a completed result. CVTSI2SS preserves the upper three XMM lanes;
GPR destinations include the generated function's live local EBP and ESP.
Sources are captured before destination assignment; failed memory reads precede
conversion exceptions. EFLAGS remain unchanged.

The standalone synthetic fixture compares1,771,152 masked hardware cases across
all exponent fields, sign choices, boundary mantissas, deterministic random
bits, integer limits, four rounding modes, FTZ and pre-existing sticky flags.
Its36,960 emitted-code checks include26,880 cases across all64 exception masks,
six register/memory/FS/wrapped-address forms and the local EBP/ESP convention.
Eight threads add8,000 hardware comparisons with independent guest MXCSR.
Linux64 and actual ELF32 each inspect20 physical SIGFPE/vector19 contexts,
checking exact status and unchanged destinations. Actual PE32 under Wine passes
the same result/emission/thread gates; the physical context oracle is Linux-only.

The supported dependency-closed selection increases96 functions to16,739 with
153 validated shared tails:20,180 provisional functions,3,819 emission failures,
11,544 edges,3,441 dependency-removed callers and1 rejected extent. Body plus
declarations total37,434,145bytes, SHA256
`cf3f0e62cc353c13e4b66e0ae38dd0410bc1cab1fe9c716b592789fa980f67c4`;
complete private source SHA256
`fc66b4be3cffd35f78cbbd55302f8da5594f301b9e7ae5846fdd01f720b47198`.
Full generator suites pass45 host and33 Windows-tool tests; strict release
lint and formatting pass. Strict O3 compilation against the pinned generic
runtime header succeeds on ELF64, ELF32 and i386 COFF. Each relocatable link
contains16,739 native definitions and resolves the generated, fault, SSE and
SoftFloat services, including the new conversion service. Other kernel/MMIO/
host symbols remain external. This does not establish a complete game link;
remaining x87/MMX/SSE instructions, discovery/recovery/patches, indirect/kernel
and exception/ABI integration, signed GamePKG/runtime integration and measured
heavy optimization across all ports still need completion. Readiness fields
remain false; no game-derived code is published.

```sh
GAMEPKG_TAKEDAWN_CONVERSION_FIXTURE=/tmp/gamepkg-conversion.c \
  cargo test --release --locked --offline \
  --manifest-path engines/takedawn/native-gen/Cargo.toml --test sse_conversion
clang-18 -m32 -msse2 -std=c11 -O3 -mno-red-zone -pthread \
  -Wall -Wextra -Werror -fsanitize=undefined -fsanitize-undefined-trap-on-error \
  -Iengines/takedawn/native-gen/runtime /tmp/gamepkg-conversion.c \
  engines/takedawn/native-gen/runtime/gamepkg_x86_fault.c \
  engines/takedawn/native-gen/runtime/gamepkg_x86_sse.c \
  -o /tmp/gamepkg-conversion32
/tmp/gamepkg-conversion32
```

For Windows, compile the same synthetic source and services with
`i686-w64-mingw32-gcc -msse2 -std=c11 -O3 -pthread -Wall -Wextra -Werror`
and execute under an isolated maintainer Wine prefix. Players gain no Python
requirement. These defined-instruction checks do not qualify physical Xbox
hardware, full Windows gameplay, guest SEH recovery or game-level speed gains.

## Shared x87/MMX state and initial instructions

The new host service runtime/gamepkg_x86_x87.c stores eight physical80bit
registers as explicit64bit significands and16bit sign/exponents, plus x87
control, status and full tag words. MMX aliases those physical significands;
it does not use the upstream runtime's independent double/MMX shadow files.
MMX destinations set their sign/exponent toFFFF, validate every tag and set
TOP to0. Read-only MMX operations validate tags and set TOP without overwriting
the source's80bit representation. EMMS empties all tags without erasing data.
Integer lanes use unsigned shifts/masks rather than host union byte order.
This follows the register transition rules in
[Intel's architecture manual](https://cdrdv2-public.intel.com/835781/325462-sdm-vol-1-2abcd-3abcd-4.pdf).

Typed lowering now handles MOVD/MOVQ, PAND/PANDN/POR/PXOR, wrapping
PADDB/W/D and PSUBB/W/D, PCMPEQB/W/D and signed PCMPGTB/W/D, plus EMMS.
The x87 control foundation handles FNINIT, FNCLEX, FNSTCW, FNSTSW AX/m16
and FWAIT. FNINIT resets the environment without erasing physical register
bits. FNCLEX clears exception/summary/busy bits and preserves condition codes
and TOP. Non-waiting stores and clear/init work without first delivering a
pending exception. Floating loads/stores, arithmetic and FLDCW are still
rejected; this is the shared state needed to implement them accurately.

MMX and FWAIT check unmasked x87 exception flags before register/memory access
and use the concrete vector16 fault service with exact EIP/local EBP. The
check uses status flags and control masks, not just ES: physical FXRSTOR/FWAIT
experiments show that restored summary bits alone do not determine delivery.
Sources and results are staged; failed loads leave register bits/tags/TOP
unchanged. Multi-lane stores still require memory-span validation and partial
store rollback. Full CR0/CR4 mode selection, complete fault-context capture,
SEH/resumption and old/new kernel/manual ABI integration remain open.

The asset-free oracle executes393,336 generated/hardware cases across all16
binary operations, register/aliased/memory operands, all eight initial TOP
positions, MOVD/MOVQ and each physical register. It compares all eight80bit
register images, control/status and nonempty tags using actual FXSAVE images.
It also checks65,536 hardware FNCLEX status cases,288 pending-exception cases,
load-fault rollback, and 8,000 threaded MMX/EMMS transitions with independent
state. Linux64 and actual ELF32 run the288 physical pending exception cases;
PE32 Wine runs the same software mask/summary-state cases without the Linux
signal oracle. All three targets pass. The generated fixture is synthetic and
contains no game bytes. Full suites pass48 host/35 Windows-tool tests and
strict release lint/formatting.

Private owned-media generation now selects16,776 functions (up37), with154
validated shared tails:20,208 provisional functions,3,791 emission failures,
11,578 dependency edges,3,432 dependency-removed callers and1 rejected extent.
The38,242,234-byte body/declaration stream SHA256 is
`0344f9ec125aa30ac1992d891b91ef1b6abac5b4434be18d67e7627862fbcd13`;
complete-file SHA256 is
`3a3694a33873030c1d6ffd68c7dee4d231df9a946ed879aa613407561f6a5f3c`.
Generated code remains private. Readiness fields remain false; full x87
arithmetic, remaining MMX/SSE operations, discovery/recovery/patches and
indirect dispatch, signed GamePKG integration, complete game/platform fidelity
and measured heavy optimization of every port remain required. Instruction
coverage does not establish a speed gain or a playable package.

```sh
GAMEPKG_TAKEDAWN_MMX_FIXTURE=/tmp/gamepkg-mmx.c \
  cargo test --release --locked --offline \
  --manifest-path engines/takedawn/native-gen/Cargo.toml --test mmx_x87
clang-18 -m32 -msse2 -mfpmath=sse -std=c11 -O3 -pthread \
  -Wall -Wextra -Werror -fsanitize=undefined -fsanitize-undefined-trap-on-error \
  -Iengines/takedawn/native-gen/runtime /tmp/gamepkg-mmx.c \
  engines/takedawn/native-gen/runtime/gamepkg_x86_fault.c \
  engines/takedawn/native-gen/runtime/gamepkg_x86_x87.c -o /tmp/gamepkg-mmx32
/tmp/gamepkg-mmx32
```

Use i686-w64-mingw32-gcc for the equivalent PE32 fixture and a private
maintainer Wine prefix. This is reviewed host tooling, not a guest dependency;
players acquire no Python requirement. Host C long double/double arithmetic is
not used as a substitute for guest extended precision.

The16,776-function module also passes strict O3 compilation against the
unchanged pinned generic runtime header on ELF64, ELF32 and i386 COFF.
Relocatable links on each target resolve every generated/fault/SSE/SoftFloat/
x87/MMX service reference, including the new vector16 handler and shared-state
service. Kernel/MMIO/other host symbols remain external; this is not a complete
game executable or an old/new runtime ABI qualification. The original private
playable reference binary remains unchanged.

## Extended-precision arithmetic backend

runtime/gamepkg_x86_ext80.c now provides add, subtract, multiply, divide and
square root on raw80bit values through25 additional pinned SoftFloat3e C
sources. It uses the same8086-SSE specialization, portable multiword primitives,
explicit byte order and C11 thread-local state. There is no host double/long
double conversion, native FP environment change, native128bit requirement or
player Python dependency. Original archives and copied file hashes are checked;
recorded adaptations enforce primitive preconditions, remove unused code and
replace a negative-exponent signed shift with defined floor division.

The non-committing API accepts PC00/10/11 (24/53/64 significand bits) and all
four rounding modes, rejects reserved PC01 and invalid host arguments without
writing, and returns the default masked numerical result plus newly raised
IE/DE/ZE/OE/UE/PE. It handles unsupported80bit encodings, quiet/signaling NaNs,
signed zeros, pseudo-denormals and denormals with the observed priority rules.
Private library rounding, tininess, precision and flags are restored after every
calculation. The original binary32 source list remains unchanged; extended math
uses its own dependency list, ext80-sources.txt.

This is the numerical foundation, not complete x87 instruction support. The
shared architectural state still needs stack/tag checks, C1, wrapped unmasked
overflow/underflow results, deferred status/delivery, FIP/FDP/opcode metadata,
pop/commit ordering and memory/register instruction integration. Existing
unsupported x87 instructions remain rejected; the supported16,776-function
selection and readiness=false fields are unchanged. Transcendentals, compares,
loads/stores/conversions/environment operations, remaining MMX/SSE and complete
runtime/dispatch/signed GamePKG integration also remain required.

The independent oracle installs raw registers with FXRSTOR, executes actual
x87 opcodes and captures FXSAVE results; it does not narrow operands through
host floating types or add FLD rounding.1,023,600 raw result/exception cases
cover all supported precision/rounding pairs, extreme exponents, integer-bit
encodings, NaN payload/sign combinations, subnormals and deterministic random
inputs.8,000 threaded cases check independent state and exact private-state
restoration. These pass on Linux64 with UB checks, actual ELF32 with UB traps,
and actual PE32 under Wine. They do not prove physical Xbox/Windows gameplay.

The explicit-LE synthetic corpus has1,023,600 rows and SHA256
`ef26d1b8c2a509699e2e6c898217ca030880e43a2772de57c07021ddf30498e9`.
All cases also replay exactly on a static big-endian MIPS32 executable under
QEMU, enforcing32bit pointers and undefined-behavior traps. The new maintainer
script verifies both native and MIPS replays; CI generates the corpus during
its existing asset-free full suite and adds extended replay beside the SSE
gate. These are local verified checks; no remote CI result is claimed.
Full suites pass50 host/36 Windows-tool tests and strict release lint/formatting.

```sh
GAMEPKG_TAKEDAWN_EXT80_FIXTURE=/tmp/gamepkg-ext80.c \
GAMEPKG_TAKEDAWN_EXT80_CORPUS=/tmp/gamepkg-ext80.bin \
  cargo test --release --locked --offline \
  --manifest-path engines/takedawn/native-gen/Cargo.toml --test ext80_math
GAMEPKG_EXT80_CORPUS_SOURCE=/tmp/gamepkg-ext80.bin \
  bash tools/check_takedawn_ext80_portability.sh
```

The script accepts GAMEPKG_HOST_CC, GAMEPKG_MIPS_CC and GAMEPKG_QEMU_MIPS
for locally unpacked maintainer tools. For ELF32/PE32, compile the exported
fixture, runtime/gamepkg_x86_ext80.c and both pinned source lists with their
include paths, C11/O3 and strict warnings. Use Clang -m32 with UB traps or
MinGW i686 GCC and an isolated Wine prefix. No game code/assets or private
reference addresses are consumed by these tests; no speed gain is claimed.

## Architectural x87 register arithmetic

The typed emitter now connects FADD/FMUL/FSUB/FSUBR/FDIV/FDIVR, their pop
variants and FSQRT register forms to runtime/gamepkg_x86_x87_arithmetic.c.
It uses the shared physical80bit state and logical TOP-relative operands,
snapshots both inputs, applies reversal, classifies result tags and commits
register results/pop only when the instruction's exception response permits.
This includes masked empty-stack indefinite results, unmasked stack faults,
pre-computation exceptions that suppress destination/pop changes, and post-
computation exceptions that return rounded or wrapped values before deferring
delivery. The following waiting operation delivers vector16 with its own EIP/
live EBP while preserving the original floating instruction's FIP/opcode/state.
Register instructions preserve FDP. Complete selectors/context/SEH integration
still needs qualification. PC00/10/11 are the supported host profile; reserved
PC01 is still unsupported. FLDCW and memory arithmetic are not enabled here.

The SoftFloat round-pack routine now records its normalized unrounded96bit
input through a thread-local observer, without changing its numerical output.
Rerounding that small snapshot supplies C1 and the exponent-adjusted result
for unmasked overflow/underflow, including exact tiny results and precision
loss before clamping/denormalization. Observation and all private math state
are restored. The recorded vendor adaptation and local hash cover the hook.
The numerical oracle now checks C1 on all1,023,600 cases and100,920 unmasked
wrapped-range responses, plus normal/min-normal boundary non-wrap responses.
Eight threads add8,000 cases. These pass on Linux64, actual ELF32 and PE32 Wine.

The generated-code architectural oracle covers55 representative encodings:
register, reversed, aliased and popping forms, ST0/ST1/ST7, all eight TOP
positions, all64 exception masks, all supported precision/rounding modes and
empty operands.1,351,680 hardware/generated comparisons check every80bit
register, nonempty tags, defined status bits, exact guest FIP/opcode/FDP and
unchanged GPR/EFLAGS. Inputs include denormals, NaNs, infinities, unsupported
encodings representative of MMX register contents and pseudo-denormals.
C0/C2/C3 are architecturally undefined for these arithmetic operations; their
prior virtual image is preserved and excluded from equivalence claims.
585,174 queued-fault checks use a distinct next instruction address/EBP and
verify unchanged x87 state after delivery. The same fixture passes actual
ELF32/PE32; this is not physical Windows/Xbox or complete guest SEH qualification.

The v2 explicit-LE corpus includes C1 and wrapped-range metadata. All1,023,600
rows replay on big-endian MIPS32/QEMU with UB traps; SHA256
`f965f9802b895d94ab61ec10a2c43460b8e1aaedc2d12b75a5564c9b12ddd948`.
Full suites pass53 host/38 Windows-tool tests and strict release lint/formatting.
The previous v1 corpus remains historical evidence and is rejected by the v2
consumer. The existing asset-free CI path regenerates the current corpus.

A new reference-instruction-coverage diagnostic calls the full typed emitter,
rather than only its integer subset. On the private pinned inventory it reports
655,559 lowered/154,618 unsupported instruction forms across810,177 instructions,
including1,238 newly accepted x87 arithmetic sites. Counts include branches/
calls/returns rejected by this single-instruction API even when the separate
function emitter handles them; they are not a completion percentage.
Linux64 and actual PE32 reports are byte-identical. The largest remaining x87
barriers are3,819 FLD,3,288 FSTP,1,945 memory FMUL,855 memory FADD and617 FILD
sites. This identifies memory loading/storing/conversion as the next integration
work, alongside remaining MMX/SSE, comparisons, controls and transcendentals.

Whole-function selection is still16,776 functions/154 shared tails and its
complete private C SHA256 is unchanged:
`3a3694a33873030c1d6ffd68c7dee4d231df9a946ed879aa613407561f6a5f3c`.
Every newly admitted register site is in a function that still has another
unsupported operation or dependency. Byte equality allowed reuse of the
previously compiled ELF64/ELF32/i386 COFF game objects. Fresh runtime/extended-
math objects link with them on each target, retain16,776 native definitions and
resolve all generated/fault/SSE/SoftFloat/x87/MMX services, including the new
arithmetic function. Kernel/MMIO/other host symbols remain external; unchanged
selection is not proof of a complete executable or old/new ABI readiness.
Both readiness fields remain false and the original private game binary is
unchanged. Full discovery/recovery/patches/dispatch, memory/exception/thread/
kernel ABI, signed GamePKG launch integration and game/platform fidelity remain
required. No speed gain or fully playable package is claimed.

```sh
GAMEPKG_TAKEDAWN_X87_FIXTURE=/tmp/gamepkg-x87.c \
  cargo test --release --locked --offline \
  --manifest-path engines/takedawn/native-gen/Cargo.toml --test x87_arithmetic
```

For32bit fixtures, compile that generated synthetic C with the fault, x87,
x87_arithmetic and ext80 services plus both pinned SoftFloat source lists,
C11/O3/strict warnings, pthread and SSE-based host C math. Use Clang -m32 with
UB traps for ELF32, or MinGW i686 GCC and an isolated Wine prefix for PE32.
The actual guest math still uses integer software primitives. These tests
consume no game assets and introduce no player Python dependency.


## Exact x87 loads

Typed FLD binary32/binary64/raw 80-bit memory and ST(i), signed FILD 16/32/64,
and the seven FLD constants now call gamepkg_x86_x87_load.c. Conversion uses
integer bits throughout, including exact subnormal normalization, integer
minima, NaN payloads and signed zero. Raw 80-bit memory/register copies preserve
unsupported encodings and signaling NaNs without inventing conversion faults.
Constant rounding follows RC independently of PC, without precision exceptions.

The service updates the shared physical stack, full tags, TOP, defined C1 and
instruction/opcode/data metadata. Masked stack faults push indefinite; newly
unmasked invalid faults suppress the push. A newly unmasked denormal load
still pushes its exact extended value and queues the exception. Existing
pending faults precede every memory read. Complete 2/4/8/10-byte operands are
staged before any architectural commit; failures preserve the entire state.
Register/constant loads preserve FDP. Addresses support the existing typed
FS/address16/wrapped-address machinery without accepting free expressions.

The asset-free fixture passes 1,736,704 hardware/generated cases, including all
65,536 signed 16-bit inputs, all 64 exception masks, four PC and RC settings, all TOP
positions, empty/full stacks, raw 80-bit special encodings, register aliases and
six memory-address forms.310,177 queued cases verify both immediate and
separate next-wait delivery with exact EIP/local EBP, preserved original FIP
and no reads or state changes.8,000 threaded cases check independent state.
All pass Linux64 with UB checks, actual ELF32 with UB traps and PE32 under
Wine. Linux targets additionally pass18 real page-fault/pending-exception
priority and saved-context cases. Saved FIP is compared to the actual FXSAVE
baseline: this AMD host can report zero there while FNSTENV reports the
preceding instruction. The virtual-state checks separately require exact,
nonzero guest metadata; the OS-context gate does not establish legacy FIP
preservation where that context does not expose it.

The initial load stage selected16,791 functions/154 shared tails. Its private
38,320,641-byte body SHA256 is
`5c18eebc21384cc8f804c8b3e78f85532f62c551e5d3cc9be2f8e3915df4a74d`;
complete-file SHA256 is
`61b6e2c16a9fda65f1afeb2f7b57743e8ab8d710a6405e0456dd8f0ccb0a7d73`.
Fresh strict O3 ELF64/ELF32/i386 COFF objects link all generated/instruction
services, with other host/kernel/MMIO symbols still external. Loads admit
4,648 additional instruction sites. This does not include stores, memory
arithmetic, BCD, full environment/control, comparison or transcendental support.

```sh
GAMEPKG_TAKEDAWN_X87_LOAD_FIXTURE=/tmp/gamepkg-x87-load.c \
  cargo test --release --locked --offline \
  --manifest-path engines/takedawn/native-gen/Cargo.toml --test x87_load
```

For32bit checks, compile the exported synthetic fixture with x87_load, x87 and
fault services, C11/O3/strict warnings and SSE-based host C math; ELF32 uses
Clang -m32, pthread and UB traps, while PE32 uses MinGW i686 GCC and the private
Wine prefix. These fixtures contain no game data and need no player Python.

## Saturating, multiply, pack and unpack MMX

Typed lowering additionally handles PADDSB/W, PADDUSB/W, PSUBSB/W, PSUBUSB/W,
PMULLW, PMULHW, PMULHUW, PMADDWD, PACKSSWB/DW, PACKUSWB and low/high
PUNPCK byte/word/dword forms. This adds21 operations to the prior16 binary
operations. Pure integer helpers use explicit masks and signed 64-bit intermediates;
there are no signed shifts, narrowing assumptions, host unions or floating
conversions. PMADDWD's four-minimum-word overflow wraps to80000000. Pack
sources are captured before destination writes, including aliases. Low unpack
memory operands read exactly 32 bits; high unpack and the other binary memory
forms read 64 bits. Pending-fault checks and staged commit use the existing
shared x87/MMX physical state.

The expanded fixture passes 1,433,720 hardware/generated full-register-state
cases, including every byte pair for all four saturating byte operations with
register and memory sources, boundary/random word/dword lanes, pack limits,
multiply overflow, aliases and all initial TOP positions. All37 binary forms
check pending-fault state preservation and each possible failed read. Existing
65,536 hardware clear-status,288 pending-exception and 8,000 thread gates remain.
Actual Linux64/ELF32 tests also execute each low unpack immediately before a
protected page, proving no extra high-word read. Linux64 UB checks, actual
ELF32 UB traps and PE32 Wine pass. Full suites pass56 host/40 Windows-tool
tests, strict release lint and formatting. These are local runs, not a remote
CI or physical Windows gameplay qualification.

Linux64 and actual PE32 generators produce byte-identical complete private C,
source reports and full-instruction diagnostics. Combined load/MMX selection
is16,793 functions/154 shared tails:20,239 provisional,3,760 emission rejects,
11,627 dependency edges,3,446 dependency-removed callers and1 rejected extent.
The38,366,142-byte body SHA256 is
`1ebe01953bd0c5fe1d7aec256080b3fa8e55dc0ab4e3a29ca8f1b096f2065d9e`;
complete-file SHA256 is
`ce239898645f1c5f35b1ba8252083989f4d9709ca13f3560c0b2b916e56d14ce`;
source-report SHA256 is
`9cc611cb13c47265dac75cc58df4e8d25da18f031095c1c5af868ccd4147d5d2`.
Fresh strict O3 game objects and current load/MMX services link on ELF64,
ELF32 and i386 COFF, retaining16,793 native definitions and resolving all
generated/fault/SSE/SoftFloat/x87/MMX references. Other host/kernel/MMIO symbols
remain external, so these are partial-module checks rather than an executable.

Instruction diagnostics admit 661,019 forms and reject 149,158 among 810,177
decoded instructions;812 additional sites use the new MMX operations. Branch/
call/return rejections in this diagnostic are not function-emitter flow gaps.
Remaining MMX barriers include shifts, PSHUFW, PAVGB, PMINSW, PINSRW,
PMOVMSKB and streaming stores/fences. x87 stores, memory arithmetic,
conversion/compare/transcendental/environment instructions and reserved PC01
arithmetic remain unfinished. Full native discovery/recovery/patch/dispatch,
SEH/memory/kernel/thread and old/new ABI, signed GamePKG launch and complete
game/platform fidelity also remain required. Both readiness fields stay false,
the original private playable prototype is unchanged, and no assets or
game-derived code/inventory are published. No FPS gain or full playability is
claimed by these instruction/component gates.


## MMX shifts with full counts

Typed PSLLW/D/Q, PSRLW/D/Q and PSRAW/D now call the production
`gamepkg_x86_mmx_shift` service for immediate, register and memory counts.
The count is the entire unsigned 64-bit operand. It is never truncated to a
host integer or masked to five bits. Oversized logical counts zero each lane;
arithmetic right shifts sign-fill each word or dword. Zero-count instructions
still read their source, validate MMX tags, set TOP to zero and mark the
destination's exponent FFFF. The helper uses unsigned shifts and explicit
fill masks, with zero/full-width cases handled before any C shift.

The emitter checks instruction bytes against the typed mnemonic, destination,
count encoding and immediate value. It rejects XMM/unsupported prefixes,
wrong memory widths, invalid registers, injected expressions and conflicting
metadata. Register counts are captured before destination commits, including
self-aliases. Memory counts read both dwords after the old pending-fault check;
a failure in either read preserves the entire shared x87/MMX state. MMX
instructions preserve the virtual floating instruction/data/opcode metadata.
The shared host header now explicitly rejects non-eight-bit byte platforms.

The synthetic fixture exercises 2,368 generated forms: all eight operations,
all MMX registers, register/self-alias/unaligned/FS/address16 count sources and
every immediate byte. It passes 1,343,488 actual hardware/generated full-state
comparisons across every TOP position, sparse/empty/full tags, all 0..255
variable counts, upper count bits, large random counts and boundary/random
data. All forms check pending-fault suppression; 384 staged-read failure cases
include zero counts. Eight threads perform 8,000 further hardware comparisons
with independent state. Linux64 and actual ELF32 additionally pass 16 real
page-fault/pending-exception priority cases, including a zero low count dword
followed by an inaccessible high dword. PE32 Wine runs the value/state/thread
and generated-fault checks, with zero Linux signal checks.

Linux64 UB checks, actual ELF32 UB traps and PE32 Wine pass. All three emit the
same explicit little-endian hardware corpus: 1,343,488 records, each 32 bytes
plus a 12-byte header, SHA256
`6cdc3c64380ca8421204daeb07d78a3a1980f2f1dc9014f4f2db8fd2eec84889`.
The bounded streaming replay calls the same production service without x86
instructions; native and actual big-endian MIPS32/QEMU replay every row exactly
with UB traps. The MIPS build enforces 32-bit pointers and big-endian order.
CI now generates and replays this synthetic corpus alongside the earlier SSE
and extended arithmetic gates. The exact script passed locally; a remote CI
run or full game portability is not claimed. Full generator suites pass 59
host / 42 Windows-tool tests and strict release lint/formatting.

```sh
GAMEPKG_TAKEDAWN_MMX_SHIFT_FIXTURE=/tmp/gamepkg-mmx-shift.c \
GAMEPKG_TAKEDAWN_MMX_SHIFT_CORPUS=/tmp/gamepkg-mmx-shift.bin \
  cargo test --release --locked --offline \
  --manifest-path engines/takedawn/native-gen/Cargo.toml --test mmx_shift
GAMEPKG_MMX_SHIFT_CORPUS_SOURCE=/tmp/gamepkg-mmx-shift.bin \
  bash tools/check_takedawn_mmx_shift_portability.sh
```

The portability script requires a maintainer MIPS cross-compiler and QEMU; it
can generate its own synthetic corpus when no path is supplied. For ELF32 or
PE32, compile the exported fixture with the x87 and fault services using
C11/O3/strict warnings and SSE-based host C math, as for the earlier MMX
fixture. These are maintainer tools and introduce no player Python dependency.

All 455 shift sites in the current reference instruction inventory now emit.
The diagnostic admits 661,474 forms / rejects 148,703 of 810,177 decoded
instructions. As before, its branch/call/return rejects are not equivalent to
whole-function flow failures. Selection grows by 21 functions to 16,814, with
156 validated shared tails: 20,255 provisional functions, 3,744 emission
rejects, 11,628 dependency edges, 3,441 dependency-removed callers and one
rejected extent. Linux64 and actual PE32 generators emit byte-identical private
C, source reports and full-instruction diagnostics. The 39,578,172-byte body
SHA256 is
`fc6ec182d7bdf1d66d289db5855bf999df9fa74b37862b3dcca9bbaaba98bebb`;
complete-file SHA256 is
`b0732d91e2530a278c3da07aa7431318c1109203af3b83dc1ab462bb1f95f3ec`;
report SHA256 is
`85345c2734bcb50c3fb5522766c29c52d54a77e10ff1deeb3f58948ebc6bac1e`.
Fresh strict O3 game objects and current MMX/load runtime services relink on
ELF64, ELF32 and i386 COFF, retaining 16,814 native definitions and resolving
all generated/fault/SSE/SoftFloat/x87/MMX service references, including the
shift helper. Kernel/MMIO/other host symbols remain external.

Remaining MMX work includes shuffle, average/min/max, insert/extract/mask,
streaming stores/fences and the mixed MMX/SSE conversions. x87 stores, memory
arithmetic and the remaining conversion/compare/transcendental/control and
environment set, native discovery/recovery/patch/dispatch, complete memory/
SEH/kernel/thread/old-new ABI and signed GamePKG launch/game/platform fidelity
remain required. Both readiness fields remain false; the original private
prototype is unchanged. No game assets, game-derived C or reference inventory
are published, and instruction gates do not establish FPS gains or full game
playability.


## Remaining SSE-era MMX lanes and transfers

Typed PAVGB/W, PMINUB/PMAXUB, PMINSW/PMAXSW, PSADBW, PSHUFW, PINSRW,
PEXTRW and PMOVMSKB now use `gamepkg_x86_mmx_lanes.c` alongside the shared
physical x87/MMX state and fault service. The pure integer helper computes
round-up unsigned averages, signed-word or unsigned-byte min/max, horizontal
absolute byte differences, word permutations, word insertion/extraction and
byte sign masks without host floating point or endian-dependent unions.

The emitter validates opcode/register/immediate metadata, rejects unapproved
prefixes/widths/free expressions and checks old pending x87 faults first.
Sources stage before commits, including aliases. PINSRW reads exactly a word
from memory or the low word of r32; all upper immediate selector bits are
ignored as encoded by the instruction. PEXTRW and PMOVMSKB zero-extend their
r32 output. Those read-only MMX operations validate tags and set TOP to zero
without overwriting any physical register's significand/exponent. MMX-result
forms also set the destination exponent FFFF. Floating instruction/data/opcode
metadata and guest EFLAGS remain unchanged. Generated EBP reads/writes use the
function's local EBP rather than the fault bridge's global slot.

The asset-free fixture passes 1,848,320 actual hardware/generated full-state
comparisons across 4,200 forms and all eight TOP positions, with sparse/empty/
full tags and boundary/random raw register data. It covers every shuffle and
insert/extract immediate byte; additional cases vary all destination/source
fields independently of the selected word. Every byte pair is exhausted for
byte average/min/max/absolute differences with register and memory inputs.
Every word pattern is tested for word average and signed min/max against two
boundary/complement pairs. All eight GPRs and MMX registers are exercised,
including local EBP and virtual ESP, self-aliases, zero-extension and preserved
source register images. Each form verifies pending-fault state suppression;
976 staged-read failure cases preserve all floating state. Eight threads run
8,000 further hardware comparisons with independent state.

Linux64 UB checks, actual ELF32 UB traps and PE32 Wine pass. Linux targets add
six physical memory/pending-fault priority cases: packed/shuffle reads must
access the second dword, and PINSRW must succeed when only its two source bytes
precede a protected page. An old pending exception wins before those accesses.
Windows runs zero Linux signal cases. All three targets export identical
explicit-LE hardware corpora: 1,848,320 records of 32 bytes plus a 12-byte header,
SHA256 `77a05c8a98256293ff540bd8e3e8c13dce2779a2b9f4fd6f58ec4d202bd96b8b`.
The bounded replay invokes the same production helper without x86 instructions
and matches every row on native and big-endian MIPS32/QEMU with UB traps and
compile-time endian/pointer checks. CI now includes generation/replay for these
lanes as well as shifts; the exact script is verified locally, without claiming
a remote CI run or full game/platform qualification. Full generator suites
pass 62 host / 44 Windows-tool tests and strict release lint/formatting.

```sh
GAMEPKG_TAKEDAWN_MMX_LANES_FIXTURE=/tmp/gamepkg-mmx-lanes.c \
GAMEPKG_TAKEDAWN_MMX_LANES_CORPUS=/tmp/gamepkg-mmx-lanes.bin \
  cargo test --release --locked --offline \
  --manifest-path engines/takedawn/native-gen/Cargo.toml --test mmx_lanes
GAMEPKG_MMX_LANES_CORPUS_SOURCE=/tmp/gamepkg-mmx-lanes.bin \
  bash tools/check_takedawn_mmx_lanes_portability.sh
```

ELF32/PE32 fixture builds link the lanes, x87 and fault services with the same
C11/O3/strict-warning/SSE-based host C math settings as the shift fixture.
The portability script uses a maintainer MIPS cross-compiler and QEMU, and can
generate its own synthetic corpus. No player Python or extra guest dependency
is introduced. The helper is reviewed host runtime code, not a new guest crate.

All 82 current inventory sites for these operations now emit: 19 PAVGB,
12 PMINSW, 42 PSHUFW, three PINSRW and six PMOVMSKB. Other newly implemented
forms are hardware-qualified even though this inventory contains none. Full
instruction diagnostics admit 661,556 forms and reject 148,621 of 810,177;
single-instruction branch/call/return rejects remain distinct from function
flow support. The dependency-closed selection grows by ten to 16,824 functions
with 156 shared tails: 20,265 provisional functions, 3,734 emission rejects,
11,629 edges, 3,441 dependency-removed callers and one rejected extent. Linux64
and actual PE32 generators produce byte-identical private C, source reports
and instruction diagnostics. The 40,445,601-byte body SHA256 is
`9b6545f6a567724d1743b6293ba2f77ac11f8efa99bcfc634309805210d8ac10`;
complete-file SHA256 is
`e79e38a62b03d51b00a3e2d77d86301e4231ddd21391c6a788d2892faf7393ab`;
report SHA256 is
`d19e160896ea516308c658218c6ccf3717ba39edd6f4bdb4c4bc58698c01bc63`.
Fresh strict O3 game objects and current runtime services relink on ELF64,
ELF32 and i386 COFF, retaining 16,824 native definitions and resolving every
generated/fault/SSE/SoftFloat/x87/MMX service, including the new lanes helper.
Kernel/MMIO/other host references remain external; this is a partial module.

Streaming/masked stores and fences, prefetch hints and mixed MMX/SSE numeric
conversions remain. The current inventory includes 59 MOVNTQ, eight CVTPI2PS
and 48 CVTPS2PI sites; absence of other forms does not establish complete MMX
support. x87 memory arithmetic/stores and the remaining conversion/compare/
transcendental/control/environment set, discovery/recovery/patch/dispatch,
complete memory/SEH/kernel/thread/old-new ABI, signed GamePKG launch and full
game/platform fidelity remain required. Both readiness fields stay false and
the original private prototype is unchanged. No game assets, derived C or
reference inventory are published; component gates do not claim FPS gains or
full GamePKG playability.


## Mixed MMX/SSE conversions

Typed CVTPI2PS, CVTPS2PI and CVTTPS2PI now call
`gamepkg_x86_convert_mmx` in `gamepkg_x86_mmx_convert.c`. The existing
scalar converter is split into a non-committing integer-only primitive,
`gamepkg_x86_convert_bits`, and its original status-applying wrapper. Packed
conversion evaluates both lanes and applies invalid before precision, so an
unmasked invalid result suppresses new precision from either lane. Rounding
uses MXCSR; truncation ignores RC. Nonzero subnormals raise precision rather
than denormal-operand, and FTZ does not alter these integer results. Private
conversion performs no host float casts or rounding-environment changes.

Real hardware establishes operand-dependent architectural ordering:

- CVTPI2PS from an MMX register checks old pending x87 exceptions, then enters
  MMX mode. Its memory form neither checks that old exception nor changes
  x87 tags/TOP; it reads both dwords and only updates the low two XMM lanes.
- Both float-to-MMX operations check an old x87 exception before reads. After
  complete reads, MMX entry validates tags and sets TOP to zero before a SIMD
  fault can occur. A SIMD fault suppresses the numerical destination while
  retaining that transition. Success also sets the MMX destination exponent
  FFFF. Other physical register images remain unchanged.
- Failed memory reads precede MMX entry and all result/status writes. XMM upper
  lanes and other registers, guest EFLAGS and virtual floating metadata remain
  unchanged. Unmasked invalid precedes precision; old MXCSR sticky flags alone
  never trigger a new exception.

The CVTPI2PS memory distinction differs from a literal reading of the Intel
instruction entry. Our native checks reproduce the behavior also reported in
[QEMU issue 4394](https://gitlab.com/qemu-project/qemu/-/work_items/4394) and its
[author's proposed fix](https://www.mail-archive.com/qemu-devel@nongnu.org/msg1221738.html).
The implementation follows observed hardware, without treating a potentially
incorrect x86 emulator as an oracle. Physical Xbox/Pentium III qualification
remains required; modern CPU checks are component evidence.

The asset-free generated fixture covers 96 register/memory/FS/address16 forms,
all eight MMX/XMM registers and TOP positions, sparse/empty/full tags, NaNs,
infinities, signed minima/maxima, binary32 integer boundaries, subnormals and
random values. It passes 417,792 comparisons against actual masked hardware
results/state, with generated checks across all 64 MXCSR exception-mask
settings, all four RC modes, FTZ, sticky bits and per-lane exception priority.
Expected fault destinations use a real hardware MMX-entry-only snapshot.

Linux64 and actual ELF32 independently execute 3,072 real success/#MF/#XM cases
across both operand forms, old x87 pending state, invalid/precision masks,
rounding, FTZ, lane order and TOP. Their kernel signal contexts are compared
against generated state, including every MMX/XMM image and MXCSR. Six real
page-fault/pending-exception cases prove complete memory reads before entry,
old x87 priority for MMX forms and its absence for memory-only CVTPI2PS.
All 96 forms also check pending-state behavior; 144 injected read failures
preserve state, and eight threads run 8,000 independent hardware comparisons.
PE32 Wine runs the value/generated-fault/read/pending/thread checks, with zero
Linux signal cases. Both Linux targets use UB checks/traps.

All three targets export byte-identical explicit-LE numeric corpora: 417,792
records of 32 bytes plus a 12-byte header, SHA256
`ca5277692f074d09325deec00c34915cf2de3b5f4e476b54d6d4f4b8da0763d8`.
The bounded replay matches both result lanes and aggregate IE/PE on native and
big-endian MIPS32/QEMU with UB traps, compile-time endian/pointer checks and
an explicitly different TLS rounding setting to verify primitive isolation.
This replay executes MIPS C, not QEMU's x86 conversion implementation. CI now
generates/replays the corpus alongside earlier arithmetic/MMX gates. The exact
script passed locally; remote CI, full Windows gameplay and full game endian
portability are not claimed.

Scalar regression checks retain 1,771,152 hardware value/status cases,
36,960 generated cases and 26,880 mask cases on Linux64/ELF32/PE32 Wine, plus
20 physical Linux SIMD faults and 8,000 thread comparisons. Full generator
suites pass 65 host / 46 Windows-tool tests and strict release lint/formatting.
The former unsupported no-prefix conversion test now rejects its unsupported
SSE2 double-precision counterpart, preserving the negative test's purpose.

```sh
GAMEPKG_TAKEDAWN_MMX_CONVERSION_FIXTURE=/tmp/gamepkg-mmx-convert.c \
GAMEPKG_TAKEDAWN_MMX_CONVERSION_CORPUS=/tmp/gamepkg-mmx-convert.bin \
  cargo test --release --locked --offline \
  --manifest-path engines/takedawn/native-gen/Cargo.toml --test mmx_conversion
GAMEPKG_MMX_CONVERSION_CORPUS_SOURCE=/tmp/gamepkg-mmx-convert.bin \
  bash tools/check_takedawn_mmx_conversion_portability.sh
```

ELF32/PE32 fixture builds link the mixed-conversion, SSE, x87 and fault services
with C11/O3/strict warnings and SSE-based host C math. The script's MIPS
cross-compiler and QEMU are maintainer tools; players acquire no Python or new
guest crate dependency. The new primitive and mixed service are reviewed host
runtime code under the same boundary as the earlier instruction services.

All 56 current mixed-conversion sites now emit: eight CVTPI2PS and 48 CVTPS2PI.
CVTTPS2PI is qualified by synthetic hardware cases despite having no current
site. Full instruction diagnostics admit 661,612 / reject 148,565 of 810,177
forms. Linux64 and actual PE32 source/reports/diagnostics are identical. Full
function selection stays 16,824 functions / 156 tails, because every newly
admitted site still has another unsupported operation or dependency. The
complete private C remains byte-identical to the lane stage:
`e79e38a62b03d51b00a3e2d77d86301e4231ddd21391c6a788d2892faf7393ab`.
Body and source-report hashes/counts are unchanged. Fresh strict O3 ELF64/
ELF32/i386 COFF game/runtime objects relink all generated/instruction services,
including the new primitive, scalar wrapper and mixed converter. Other kernel/
MMIO/host references remain external. The unchanged game body does not itself
exercise new conversions; the dedicated emitted fixtures do.

Streaming/masked stores, fences and prefetch remain, together with x87 memory
arithmetic/stores and the remaining conversion/compare/transcendental/control/
environment set. Complete discovery/recovery/patch/dispatch, memory/SEH/kernel/
thread/old-new ABI, signed GamePKG launch and original game/platform fidelity
remain required. Readiness stays false, the earlier private prototype is
unchanged, and no game assets, derived C or reference inventory are published.
No FPS gain or full GamePKG playability is claimed by these component gates.


## Checked streaming stores, fences and prefetch

Typed MOVNTQ, MOVNTPS, MASKMOVQ, SFENCE and all four SSE1 PREFETCH hints now
use a concrete checked RAM transaction service in `gamepkg_x86_memory.c`.
MOVNTQ preserves source register images and enters MMX only after a successful
store. MASKMOVQ enters before address/store faults, including a zero mask;
its selected bytes are captured before the transition. MOVNTPS requires the
final linear address to be 16-byte aligned and leaves floating state alone.
Old pending x87 exceptions precede MOVNTQ/MASKMOVQ, while MOVNTPS, fences and
prefetch do not wait for them. Flat32 logical operand bounds precede RAM
admission, with address16 start wrapping and separately applied FS base.

The RAM adapter admits the complete span before changing any byte, so failure
cannot leave a partial store. Selected bytes use explicit little-endian shifts
without reading old target data or rewriting unselected bytes. For the measured
MASKMOVQ profile the whole eight-byte write span is admitted even at mask zero;
this matches native fault checks for every mask but is implementation-dependent
in the architecture. All register and guest EFLAGS/virtual FP metadata remain
unchanged except the specified MMX tags/TOP transition. Page-fault transport
now includes the first denied linear address and error code, in addition to
EIP/local EBP/GPR/MXCSR. Other fault types clear those extra fields.

The host-only API creates an immutable view of at most 64 nonoverlapping RAM
regions with resolved read/write permissions and a supplied PF U/S bit. It
checks map bounds, pointer-range overflow and address wrapping. A binding holds
a reference per worker; owner release is safe while workers remain bound.
Workers must explicitly unbind, and the borrowed backing arena must outlive all
bindings. Accesses share a lock, fault exits unlock before delivery, and reads
stage their complete output. Actual host access to the arena must use the same
synchronization domain. Permission tables describe resolved capabilities; this
is not a CR0/CR4/page-table walker or a complete segmentation implementation.

Missing RAM binding is a host integration error and aborts rather than silently
bypassing checks. Device/MMIO memory cannot be registered as ordinary RAM.
The engine memory map, device transactions and all guest thread entry/exit
bindings must be connected before a complete game launch. Existing ordinary
wide stores and legacy memory callbacks still require migration/coordination;
this component is not a blanket all-memory protection or full ABI claim.

SFENCE uses a real C11 sequentially consistent fence. Transactions publish
stores before unlocking, providing stronger ordering than weak write-combining
permits. Non-temporal cache hints are best-effort: Clang can retain supported
host store hints and other paths use ordinary RAM stores. Prefetch issues
compiler-supported hints only for mapped readable RAM, and ignores inaccessible
addresses without guest reads or faults. Exact cache/bus behavior and a game
performance gain are not claimed; those hints are architecturally advisory.

Intel specifies SFENCE's low ModR/M bits as ignored. Capstone5 rejects seven
valid aliases, so a bounded decoder fallback admits exactly 0F AE F8..FF plus
the qualified ignored FS/address prefixes, retaining original bytes. Linear
and CFG tests check identical instruction boundaries and original byte images.
LOCK/unsupported prefixes, wrong widths/registers/metadata and injected source
expressions are rejected. No generic invalid-byte resynchronization is added.

The [Intel instruction reference](https://cdrdv2-public.intel.com/671110/325383-sdm-vol-2abcd.pdf) specifies the ignored SFENCE r/m bits; its [MASKMOVQ entry](https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-vol-2b-manual.pdf) permits implementation-dependent zero-mask faults.

The synthetic 320-form fixture passes 425,984 real hardware/generated complete
memory/register-state comparisons across all MMX/XMM registers and TOP values,
sparse/empty/full tags, source/mask aliases, every mask byte, unaligned/address16/
FS/absolute/local-EBP/scaled-index addresses, all fence aliases and all prefetch
hints. Failed transactions preserve every target byte. Linux64 compares 550
real fault/priority contexts; actual ELF32 compares 570, including flat32
segment-limit cases. The checks cover every masked-store pattern across an
inaccessible page and read-only mappings, zero masks, old x87 exceptions,
packed-store alignment and page-fault address/error bits. Read-only pages are
prefaulted so the expected present-PTE bit is controlled rather than dependent
on lazy host paging. PE32 Wine runs the value/state/software-fault tests with
zero Linux signal cases. Linux64/ELF32 use UB checks/traps.

The separate concrete RAM suite passes 41,280 byte/mask/read/write cases,
invalid/overlapping maps, fault rollback/unlock, first denied address/error,
linear32 wrapping, owner/binding lifetime and 16,000 threaded transactions.
A further 20,000 writes crossing a 64-byte boundary with a concurrent reader check for torn
transactions. Linux64 ASan/UBSan/leak checks, actual ELF32 UB traps, PE32 Wine
and actual big-endian MIPS32/QEMU with UB traps pass. The endian build enforces
32-bit pointers; the RAM suite contains no x86 instructions or game data.
The exact portability script passed locally and CI adds it; no remote CI or
full Android/Windows/game qualification is claimed. Full generator suites pass
70 host / 49 Windows-tool tests and strict release lint/formatting.

```sh
GAMEPKG_TAKEDAWN_STREAMING_FIXTURE=/tmp/gamepkg-streaming.c \
  cargo test --release --locked --offline \
  --manifest-path engines/takedawn/native-gen/Cargo.toml --test streaming
bash tools/check_takedawn_memory_portability.sh
GAMEPKG_STREAMING_FIXTURE_SOURCE=/tmp/gamepkg-streaming.c \
  bash tools/check_takedawn_windows_tls.sh
```

The tools require maintainer cross-compilers/QEMU/Wine, and introduce no player
Python or guest crate dependency. Compile the exported x86 fixture with the
memory, x87 and fault services using C11/O3/strict warnings and SSE-based host
C math. The RAM suite links only memory/fault services and is CPU-independent.

### Windows compiler/TLS profile

Clang for MinGW defaults to native Windows TLS, while GCC MinGW uses emulated
TLS. Mixing those defaults produced unresolved GPR TLS control symbols in the
executable check; a partial object link alone would not prove compatibility.
Clang translation units sharing this GCC runtime must use `-femulated-tls`
with `--target=i686-w64-windows-gnu` and the MinGW sysroot. The reusable Windows
check compiles emitted code with that profile, links GCC runtime helpers, and
executes all 425,984 streaming comparisons plus 8,000 cross-compiler threaded
page-fault contexts and parent isolation under an isolated Wine prefix. It
passes locally. Its explicit link-only mode also passes and is added to CI;
that CI step does not execute Wine. A provided GAMEPKG_WINEPREFIX can reuse a
maintainer's isolated test prefix; otherwise the script creates a private one.

The MinGW GCC whole-game compilation was terminated before producing an object.
The retry uses Clang18 with emulated TLS for the game object and GCC for the
current runtime helpers, retaining strict O3 settings and the full selection.
The mixed executable test above verifies the compiler boundary; this does not
qualify a complete game executable or an MSVC/mixed-native-TLS configuration.

### Expanded private module

All 252 current sites now emit: 59 MOVNTQ, 48 MOVNTPS, 111 PREFETCHNTA,
31 PREFETCHT0 and three SFENCE. MASKMOVQ/T1/T2 are qualified synthetically.
Instruction diagnostics admit 661,864 / reject 148,313 of 810,177 forms;
single-instruction flow rejects remain distinct from function flow support.
Selection grows by 28 to 16,852 functions / 158 shared tails: 20,286 provisional,
3,713 emission rejects, 11,639 dependency edges, 3,434 removed callers and one
rejected extent. Linux64/actual PE32 generators produce byte-identical private
C, source reports and instruction diagnostics. The 42,636,807-byte body SHA256
is `135e33ce3f59dc45bc37238f5353cf3780e2f0c3e8beb9fb064496f3e7287653`;
complete-file SHA256 is
`ffc8a32fe9eb47b67db40cfdd5943ea0bdfb0a1d7560eabc92a59c3bfb81abd8`;
report SHA256 is
`e486e3e41e5f53c52ce9db38580ea527e2f48967608a646c83bc26153b23f830`.
Fresh ELF64/ELF32 and emulated-TLS i386 COFF game objects relink all current
instruction services, including memory transactions and page faults, with
16,852 native definitions and no unresolved generated/instruction services.
Fault consumers are rebuilt for the appended address/error fields. Other
kernel/MMIO/host references and actual engine binding remain external/open.

x87 memory arithmetic/stores and the remaining conversion/compare/transcendental/
control/environment set remain required, together with full discovery/recovery/
patch/dispatch, device memory/ordinary-wide-store migration, complete segmentation/
paging/SEH/kernel/thread/old-new ABI, signed GamePKG launch and original game/
platform fidelity. Readiness stays false and the original private prototype
is unchanged. No game assets, derived C or reference inventory are published,
and this stage does not claim full GamePKG playability or an FPS gain.

## x87 floating and integer memory arithmetic

Typed FADD/FMUL/FSUB/FSUBR/FDIV/FDIVR now admit binary32 and binary64
memory operands; FIADD/FIMUL/FISUB/FISUBR/FIDIV/FIDIVR admit signed16 and
signed32 operands. Encoding, mnemonic, operand width, allowed segment and
typed address must agree. Operand-size overrides, unqualified segments, LOCK,
unsupported forms and synthetic expressions remain rejected. Qualified FS and
address16 forms work alongside local EBP, absolute and scaled/wrapping addresses.
These are the memory formats in the
[Intel instruction reference](https://cdrdv2-public.intel.com/782156/325383-sdm-vol-2abcd.pdf).

The emitter waits on old pending x87 errors, checks the logical flat32 span,
then admits the complete operand through the checked RAM service. Failed reads
leave all x87 bits and metadata unchanged. Complete reads precede new stack
underflow or numerical exceptions. This uses existing immutable RAM capabilities;
it does not add device/MMIO support, alignment checking, segment descriptors or
guest page tables. The host must bind a RAM view before execution. Missing
bindings remain host integration errors, without an unchecked callback fallback.

A shared exact bit decoder preserves IEEE values and signed integer minima
without host floating-point casts. FLD quiets signaling NaNs at its load boundary;
arithmetic retains their payload until binary NaN selection. IEEE subnormals
normalize into raw80 while a separate marker retains source DE with invalid/
NaN/zero-divide priority. Register and memory forms share the existing commit
path, extended precision, PC00/10/11 and all rounding modes, deferred exceptions,
wrapped unmasked overflow/underflow, C1, stack tags and result suppression.
Memory forms update ST0 without popping, record FIP/opcode/data address after
successful reads and preserve unrelated physical register bits. Reserved PC01
remains unsupported; this does not enable FLDCW or the entire x87 set.

The synthetic fixture covers **144 encodings**: 24 operations/formats across
six address forms. Linux64 and actual ELF32 compare **2,211,840** result/state
cases against hardware and **923,538** queued exceptions at a separate waiting
instruction, plus **624** denied-span/logical-bound/pending priority cases and
**8,000** independent worker cases. Linux also checks **72** real page-fault/
pending-x87 kernel contexts, including an inaccessible second page and empty
ST0 with unmasked invalid. Defined status, all physical register bits, full
tags, GPR preservation and virtual metadata are checked; undefined C0/C2/C3
are excluded from physical comparisons.

PE32 Wine passes the numerical, generated-fault and worker tests with zero
Linux signal contexts. Both all-GCC and Clang-generated/GCC-helper executables
pass; Clang uses `-femulated-tls` at this compiler boundary. Linux64/ELF32/both
Windows profiles export byte-identical explicit-LE corpora: **737,280** records,
64 bytes each plus a 12-byte header, SHA256
`af25f76793004151a73c9eac704dabe08206becfad86142ea2d0a2c5d40fc79a`.
Native and actual big-endian MIPS32/QEMU replay match saved numerical/state
fields and **277,104** deferred faults, checking unchanged unrelated registers
and metadata. Replay runs portable C, not emulated x86 floating-point code.
Linux64/MIPS replay uses UB checks; ELF32 uses UB traps. Wine does not qualify
physical Windows, Xbox/PentiumIII or full platform behavior.

Reproduce without game media:

~~~sh
cargo test --locked --release --manifest-path engines/takedawn/native-gen/Cargo.toml --test x87_memory -- --nocapture
bash tools/check_takedawn_x87_memory_portability.sh
GAMEPKG_CLANG=clang-18 bash tools/check_takedawn_x87_memory_targets.sh
~~~

The portability script requires the MIPS C compiler and QEMU; the target script
requires multilib Clang, MinGW GCC and Wine for execution. It accepts an existing
synthetic fixture/corpus via `GAMEPKG_X87_MEMORY_FIXTURE_SOURCE` and
`GAMEPKG_X87_MEMORY_CORPUS_SOURCE`, and a private Wine prefix through
`GAMEPKG_WINEPREFIX`. Set `GAMEPKG_X87_MEMORY_LINK_ONLY=1` to compile/link both
Windows compiler profiles without Wine or Linux multilib. CI now generates the
corpus/fixture, replays MIPS and links Windows profiles. Execution and link-only
scripts pass locally; no remote CI run is claimed. Generator suites pass
**73 host / 51 Windows-tool tests**, including register arithmetic, FLD and all
existing MMX regressions; strict release lint passes.

The private owned-disc diagnostic now admits **665,253 / 810,177** instruction
sites (144,924 rejected), adding **3,389** memory arithmetic sites. The closed
module grows by six to **16,858 functions / 158 shared tails**: 20,321 provisional
functions, 3,678 emission rejects, 11,698 dependency edges, 3,463 removed callers
and one rejected extent. Body42,686,708 bytes SHA256
`720a73032dd21dd1974713942f8f037a5324fb5b3c89271661131f542ddcac20`;
complete-file SHA256
`aa67ed4f857478ea9ba819fe05c92c94bd03a7d6794e60a6132d5247afdba220`;
report SHA256
`bb373c0b9b61aa3d13303b0a2e23c7b38f3cf1dd178a9184a1f134205ba7689e`.
Linux64/PE32 generation, reports and complete diagnostics agree byte-for-byte.
Fresh strict O3 ELF64/ELF32/i386 COFF game objects link current load/arithmetic/
RAM/fault and existing math/MMX services without unresolved generated/instruction
helpers. Windows game code uses Clang emulated TLS with GCC helpers. The
original private prototype is unchanged; other host/kernel/MMIO links remain.

This is a larger qualified partial module, not a playable GamePKG release.
x87 stores/conversions/compares/transcendentals/environment/control, ordinary
wide-store rollback and legacy callback migration, real engine RAM/device and
worker bindings, full segmentation/paging/FP-fault/SEH/kernel/old-new ABI,
recovery/patch/dispatch/source-kit/build, signed launch and full game/platform
fidelity remain. Readiness stays false. No game assets, derived C, private
inventory or player Python are published or added; no FPS gain is claimed.

## x87 ordered, unordered and integer comparisons

The emitter now admits FCOM/FCOMP/FCOMPP, FUCOM/FUCOMP/FUCOMPP,
FCOMI/FCOMIP/FUCOMI/FUCOMIP, FICOM/FICOMP and FTST. Capstone spells the
popping EFLAGS forms `fcompi`/`fucompi`; typed operand metadata includes only
the explicit ST(i), with ST0 implicit. Qualified binary32/binary64 and signed
integer16/32 memory forms use the same whole-span checked RAM reads as memory
arithmetic. Encoding/mnemonic/width/register/segment mismatches, unsafe text,
unqualified prefixes and unsupported aliases remain errors. The fixture has
**115 encodings**: 48 memory forms across six address modes, 64 register forms,
two double-pop forms and FTST.

`gamepkg_x86_x87_compare.c` is an integer-only raw80 comparison service; it
requires no host floating casts or SoftFloat arithmetic. Signed zeros compare
equal; subnormal/pseudo-denormal magnitude handling, NaN/unsupported priority,
quiet versus signaling invalid, and source IEEE DE after exact normalization
are explicit. No numerical register bits are overwritten, even for masked
invalid. Comparisons publish C0/C2/C3 or CF/PF/ZF and clear OF/SF/AF for EFLAGS
forms, retain sticky exceptions/SF, suppress popping for newly unmasked IE/DE,
and otherwise pop zero/one/two physical tags with TOP wrap. Old pending errors
precede memory/metadata access; complete reads precede new stack faults.

Hardware checks exposed details that differ from a literal reading of the
[Intel SDM comparison entries](https://cdrdv2-public.intel.com/782156/325383-sdm-vol-2abcd.pdf).
The EFLAGS family preserves C1 on ordinary comparisons; stack underflow clears
it. Other comparison forms clear C1. Independent modern Intel/AMD measurements
are reported in the [QEMU correction](https://www.mail-archive.com/qemu-devel%40nongnu.org/msg1225151.html)
and [issue4378 update](https://gitlab.com/qemu-project/qemu/-/work_items/4378).
Our measured unmasked IE/DE cases publish comparison flags while suppressing
popping; the [Bochs comparison implementation](https://github.com/bochs-emu/Bochs/blob/master/bochs/cpu/fpu/fpu_compare.cc)
also updates flags before exception-driven pop suppression. This remains an
explicit measured profile: SDM wording says flags are not set on unmasked IA,
so physical Xbox/PentiumIII qualification is still required. No current physical
condition bits are masked away to make these tests pass; C1, C0/C2/C3 and all six
arithmetic EFLAGS bits are compared, including clear/set starting patterns.
QEMU x86 emulation is not the numerical oracle for this qualification.

Linux64 and actual ELF32 pass **7,247,872** hardware/generated state cases,
**2,693,572** separately delivered next-wait exceptions, **323** whole-read/
logical-bound/pending priority cases and **8,000** isolated worker cases. The
matrix covers all64 exception masks, all four PC settings (comparison is
independent of arithmetic precision), all rounding modes/TOP, empty/aliased
operands, special-class pairs, sticky masked exceptions/SF and all64 starting
arithmetic flag patterns. **369** real Linux kernel contexts comprise24 memory
page-fault/old-error priority cases and345 deferred MF cases after empty-stack,
signaling-NaN and denormal comparisons. Those context checks verify the live
physical FPU snapshot at the subsequent waiting instruction. Virtual GPR/flags/
metadata preservation is checked separately at the generated fault boundary.

All-GCC PE32 and Clang-generated/GCC-helper PE32 executables under Wine pass
the same numerical/generated-fault/worker cases, with zero Linux signal cases.
Clang uses emulated TLS at the GCC boundary. Linux64/ELF32/both Windows profiles
export identical explicit-LE hardware-qualified corpora: **423,104** records,
80 bytes each plus a12-byte header (33,848,332 bytes), SHA256
`b8d60b3995937eef9365df6db01f22fb85b4ca404969cb3d4ab28c7ea764f665`.
The sample includes every flag pattern for the EFLAGS encodings, ordinary and
signaling operands, every IM/DM combination, plus the complete control/mask/
rounding/form matrix sampled across TOP/emptiness. Native and big-endian MIPS32
replay match all state/flags and **151,494** deferred faults, including unchanged
unrelated raw registers. MIPS executes portable C, not x86 floating emulation.
UB checks/traps and strict O3 warnings pass. This does not qualify physical
Windows/Xbox/PentiumIII, or imply the complete game is endian-independent.

Reproduce without game media:

~~~sh
cargo test --locked --release --manifest-path engines/takedawn/native-gen/Cargo.toml --test x87_compare -- --nocapture
bash tools/check_takedawn_x87_compare_portability.sh
GAMEPKG_CLANG=clang-18 bash tools/check_takedawn_x87_compare_targets.sh
~~~

The scripts mirror the memory-arithmetic checks: optional
`GAMEPKG_X87_COMPARE_FIXTURE_SOURCE`, `GAMEPKG_X87_COMPARE_CORPUS_SOURCE`,
`GAMEPKG_WINEPREFIX` and `GAMEPKG_X87_COMPARE_LINK_ONLY=1`. Execution and
Windows link-only modes pass locally. CI generates/replays the corpus and links
both Windows compiler profiles; no remote CI result is claimed. Full suites
pass **76 host / 53 Windows-tool tests**, with strict release lint and existing
MMX/load/arithmetic regressions. Old rejection fixtures were updated because
FCOM/FICOM memory operands are now implemented, retaining unsupported store
forms as negative cases.

The private owned-disc diagnostic admits **665,739 / 810,177** sites (144,438
rejected), adding486 comparison sites. The module grows by four to **16,862
functions / 158 shared tails**: 20,327 provisional, 3,672 emission rejects,
11,700 dependency edges, 3,465 removed callers and one rejected extent.
Dependency-closed source42,304,480 bytes; body42,726,030 bytes SHA256
`036d89320700febd6c09f68618828358fb1ba813783f671cc0591ed0d5e9fd6f`;
complete-file SHA256
`7d19a8ca95200921c01e3e453486cd760403af225d5ad203e03142ccfb2bb490`;
report SHA256
`c5a1d9883049e2bada7bf2f1d0055d3ba79144ef8a6900bdc101ddce9a22c749`.
Linux64/PE32 generated C/reports/complete diagnostics match byte-for-byte.
Fresh strict O3 ELF64/ELF32/i386 COFF modules resolve all generated/instruction
helpers, including comparisons, checked RAM and current fault services. Other
host/kernel/MMIO references remain. Generated Windows game C uses Clang
emulated TLS with GCC helpers. The original private prototype hash is unchanged.

Remaining x87 stores/conversions, stack/control/environment/transcendentals,
real engine RAM/device/all-worker binding, legacy callbacks/ordinary-wide-store
migration, full segmentation/paging/FP-fault/SEH/kernel/old-new ABI, discovery/
recovery/repair/dispatch/source-kit/build and signed package/full-game/platform
fidelity remain required. Readiness stays false. No owned assets, executable-
derived source or private reference inventory are published; no player Python,
guest crates, game launch or FPS improvement is claimed by this component.

## x87 stores and integer conversions

Typed FST/FSTP now support binary32, binary64, raw80 (popping only) and ST(i)
destinations. FIST/FISTP support signed16/signed32 and popping signed64.
Encoding/mnemonic/width/register/address/segment checks reject mismatched metadata,
unqualified prefixes, synthetic expressions and unsupported forms such as SSE3
FISTTP. FS, address16, absolute/local EBP and scaled/wrapping address forms are
qualified. The 76-form fixture comprises ten memory instructions across six
addresses and sixteen register instructions. Store forms that were negative
fixtures are now tested positively; negative fixtures retain unimplemented
environment/control forms.

`gamepkg_x86_x87_store.c` uses original integer-only raw80 conversion, with no
host FP casts, native128, additional SoftFloat sources or player dependency.
Guarded shifts handle zero and counts at/beyond64; exact signed minima and all
RC modes work independently of PC. IEEE stores preserve zero/NaN identity,
quiet signaling NaNs with IE, and report unsupported encodings as invalid.
Raw80/register stores copy their bits unchanged except masked empty-source
indefinites. Destination classification, overwrite of occupied registers,
self-copy, empty destinations and popping TOP wrap are explicit.

The implementation follows the memory-destination exception distinction in
[Intel volume 1, sections 8.5.4–8.5.6](https://cdrdv2-public.intel.com/671436/253665-sdm-vol-1.pdf)
and the [FST/FIST instruction entries](https://cdrdv2-public.intel.com/782156/325383-sdm-vol-2abcd.pdf).
Unmasked IE/OE/UE suppress memory/register writes and pops. Unmasked PE still
stores the rounded result and pops when requested, then queues a later MF.
Unmasked floating range errors suppress new PE and clear C1. Masked tiny exact
stores avoid UE; unmasked tiny stores raise UE. Tininess and overflow use the
rounded unbounded-exponent result. Store conversion does not raise source DE.
Sticky exceptions/SF survive; C1 is checked explicitly, while undefined
C0/C2/C3 are retained virtually and excluded from numerical hardware comparisons.

### Admission before conversion

The RAM service now accepts ten-byte spans and provides
`gamepkg_x86_memory_check(address,bytes,write,eip,ebp)`. This admits a complete
read/write span without accessing its data. Old pending exceptions and logical
flat32 bounds precede admission; permission/absent-byte faults precede new
stack/numerical errors, even when those errors would suppress the store.
Architectural changes remain private until the re-admitted complete write
succeeds. This relies on immutable maps/permissions and a live thread binding;
there is no callback/binding change between admission and write. It is not a
transferable lock/token or a way to admit devices as RAM. Complete ten-byte
stores use the existing serialized transaction, without partial publication.

The portable RAM suite now has **74,112** byte/mask/access cases, including all
1,024 ten-byte masks across 32 unaligned offsets, no-data admission checks,
permission/first-bad-byte faults, lock release, unchanged bytes after rejection,
and eight/ten-byte linear32 wrap across two maps. Owner/binding lifetime,
16,000 worker transactions and the existing 20,000 concurrent cross-cache-line
writes/reader checks remain. Linux64 ASan/UBSan/leak checks, actual ELF32 UB
traps, PE32 Wine and big-endian MIPS32/QEMU pass. This remains a resolved RAM
capability view, not guest page tables, full segmentation or device/MMIO support.

### Store qualification

Linux64 and actual ELF32 pass **4,487,168** hardware/generated state and full
16-byte target-buffer comparisons, **1,799,975** next-wait faults, **576**
read-only/missing-span/logical-bound/old-pending cases and **8,000** workers.
Tests cover 64 masks, four PC/four RC settings, every TOP, empty/occupied/aliased
registers, sticky masked flags, raw special encodings, random inputs and exact
rounding/overflow/underflow/integer-range boundaries. All physical raw-register
bits/nonempty masks, virtual destination tags/metadata and all virtual GPRs/flags/
FS base are checked. **412** real Linux fault contexts comprise 120 page-fault/
old-error priority cases and 292 newly deferred exceptions, including stores and
pops already committed before an unmasked precision exception. The context
checks compare live FPU control/status/nonempty bits/raw registers/FIP at the
fault; virtual metadata and the distinct waiting EIP are checked separately.

The hardware harness explicitly declares all XMM registers clobbered by
FXRSTOR. Restoring the CPU also overwrites vectorized C byte snapshots; omitting
these compiler clobbers produced a false buffer mismatch, fixed in the harness
rather than hidden with weaker comparisons.

Both all-GCC and Clang-generated/GCC-helper PE32 Wine executables pass the
numerical/generated-fault/worker cases with zero Linux signal contexts.
Clang uses emulated TLS. Linux64/ELF32/both Windows profiles export identical
explicit-LE hardware-qualified corpora: **383,264** records, 96 bytes each plus
12-byte header (36,793,356 bytes), SHA256
`4d584d38fe60e13bb3e3727847412f56db6f8fadfc73cc386802f35959901db1`.
Native and big-endian MIPS32 replay match every saved numerical/state/memory
field and **155,631** deferred faults, including unchanged unrelated registers.
Replay executes portable integer C and the concrete RAM service, not x86 FP
emulation. Physical Windows/Xbox/PentiumIII and complete-game portability remain
separate qualification requirements.

~~~sh
cargo test --locked --release --manifest-path engines/takedawn/native-gen/Cargo.toml --test x87_store -- --nocapture
bash tools/check_takedawn_x87_store_portability.sh
GAMEPKG_CLANG=clang-18 bash tools/check_takedawn_x87_store_targets.sh
~~~

The scripts accept `GAMEPKG_X87_STORE_FIXTURE_SOURCE`,
`GAMEPKG_X87_STORE_CORPUS_SOURCE`, `GAMEPKG_WINEPREFIX` and
`GAMEPKG_X87_STORE_LINK_ONLY=1`, mirroring the comparison checks. The target
script also executes the portable RAM suite on ELF32/PE32 and links it in the
Windows CI profile. Execution and link-only modes pass locally. CI adds corpus/
fixture generation, MIPS replay and Windows compiler-profile linking; remote CI
was not run. Full suites pass **79 host / 55 Windows-tool tests**, strict release
lint/format and existing arithmetic/load/comparison/MMX/streaming regressions.

The owned-disc diagnostic admits **669,438 / 810,177** sites (140,739 rejected),
adding 3,699 stores. The closed module grows by 194 to **17,056 functions / 161
shared tails**: 20,613 provisional functions, 3,386 emission rejects, 12,359 edges,
3,557 removed callers and one rejected extent. Closed source 47,560,381 bytes;
body 47,986,781 bytes SHA256
`2cb921ef022c98565592745ad6c9f3099d917fbc475fa69f8e9c11e6b3eb66df`;
complete-file SHA256
`aa9bf7c91cdd8b9814e535f727b936d48c30fd1ac6e4d5c219e2347a14db6da4`;
report SHA256
`a07fb6eb0e493b3c704f4ccc4406c9f21bcd88a947d25a1ade0184e6686823e8`.
Linux64/PE32 generated C/reports/complete diagnostics match byte-for-byte.
Fresh strict O3 ELF64/ELF32/i386 COFF modules resolve all generated/instruction
helpers, including stores and admission-only RAM checks. Other host/kernel/MMIO
references remain external. Windows game code uses Clang emulated TLS and GCC
helpers. The original private prototype hash remains unchanged; private output
directories/source retain restrictive permissions.

This advances the qualified partial module, not a complete playable package.
Remaining x87 stack/control/environment/transcendentals and other unsupported
operations, real engine RAM/device/all-worker binding, legacy callbacks/ordinary
wide-store migration, full segmentation/paging/FP-context/SEH/kernel/old-new ABI,
discovery/recovery/repair/dispatch/source-kit/build and signed launch/full game/
platform fidelity remain required. Readiness stays false. No game assets,
executable-derived C or private reference inventory are published. No guest
crates, player Python, full-game launch or measured FPS improvement are claimed.

## x87 stack motion, conditional moves, classification and rounding

The generator now admits FABS/FCHS, FXCH, FFREE, FINCSTP/FDECSTP, FXAM, FNOP,
FRNDINT, FXTRACT and all eight FCMOV conditions. The 88 encodings comprise eight
fixed operations, eight exchanges, eight frees and 64 conditional moves.
Capstone represents FXCH and FCMOV with explicit ST0/ST(i) operands; both must
match the encoded registers. Prefixes, synthetic expressions and mismatched
operand kinds/registers/mnemonics remain rejected. The former negative FCMOV
fixture now uses still-unsupported FLDCW instead.

`gamepkg_x86_x87_stack.c` operates on shared physical raw80 state using integer
bits only, without host FP/native128 or additional SoftFloat sources. Old pending
errors precede metadata changes. FABS/FCHS retain raw payloads, including signaling
NaNs and unsupported formats. FXCH swaps snapshots and substitutes masked empty
operands independently. FFREE changes only its selected tag; TOP rotations
preserve data/tags. FXAM classifies raw values without new exceptions and takes
its sign even from an empty slot. Ordinary conditional moves preserve C1, check
both operands even on an untaken condition, and use masked indefinite/IE/SF
responses for empty operands. Guest EFLAGS/GPRs/FS base remain unchanged.

FRNDINT performs exact signed magnitude rounding in every RC mode, independent
of PC, including signed zero and the unsigned 2^63 rounding boundary. It preserves
infinities, quiets signaling NaNs with IE, and suppresses a new precision result
on an unmasked denormal. FXTRACT normalizes the significand and produces an exact
floating exponent, including signed-zero/negative-infinity, infinity and NaN
responses. It checks source underflow before destination overflow, distinguishes
their C1 responses and suppresses both writes/push on unmasked pre-errors.
Masked faults, occupied push destinations, TOP wrap and aliased slots are explicit.

The [Intel instruction reference](https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-vol-2a-manual.pdf)
describes these forms and their stack/condition behavior. The retained ordinary
FCMOV C1 and untaken-condition empty-operand response are explicitly measured in
our fixture; they are not replaced by integer CMOV behavior. Undefined C0/C2/C3
(and all condition bits for FFREE/FNOP) are retained virtually and excluded from
physical comparisons. FXAM checks all condition bits. C1 is independently seeded
clear/set for every encoding, as are old masked exceptions and SF, and checked
where defined. The exact original Xbox/PentiumIII profile remains a separate
qualification requirement from modern-host instruction measurements.

Linux64 and actual ELF32 pass **6,447,104** hardware/generated state cases,
**1,994,542** separately delivered next-wait faults, all 88 old-pending gates and
**8,000** independent worker cases. The matrix covers all 64 masks, all PC/RC/TOP,
empty/occupied/aliased/sparse stacks, special/raw/random operands, integer-rounding
boundaries and exponent extraction. An independent conditional-move sweep covers
all 64 arithmetic-flag patterns, every condition/register, all IM/DM combinations
and eight emptiness patterns, without coupling branch truth to exception masks.
All physical raw-register bits/nonempty masks, virtual tags/metadata and all
virtual GPR/flags/FS values are checked.

**243** real Linux FPU contexts check old MF delivery for every encoding and
newly deferred empty/overflow/signaling-NaN/denormal/zero-divide/precision errors.
They include untaken conditional moves with an empty source and the already
committed rounded result of unmasked precision. Context checks compare live FPU
control/status/nonempty bits/raw registers/FIP at the waiting instruction;
virtual metadata and the distinct waiting EIP are checked independently.
FXRSTOR declares the full host XMM clobber set, preserving compiler-managed
snapshots correctly.

All-GCC and Clang-generated/GCC-helper PE32 Wine executables pass equivalent
numerical/generated-fault/worker cases, with zero Linux signal contexts. Clang
uses emulated TLS at that boundary. Linux64/ELF32/both Windows profiles export
identical explicit-LE hardware-qualified corpora: **349,184** records, 252 bytes
each plus 12-byte header (87,994,380 bytes), SHA256
`42a71c4e83389fb52017dbf9ebdec01178f5b23b497e4c89e3c86803193a3346`.
Each record contains complete before/after physical-register and metadata state.
Native and big-endian MIPS32 replay match every field and **102,690** deferred
faults. The supplied condition flags are deliberately different from the virtual
EFLAGS global during replay to prove the API uses its explicit input and leaves
unrelated state alone. Replay executes integer C, not emulated x86 FP. These are
component results, not full Windows/Xbox/Android/endian-game qualification.

~~~sh
cargo test --locked --release --manifest-path engines/takedawn/native-gen/Cargo.toml --test x87_stack -- --nocapture
bash tools/check_takedawn_x87_stack_portability.sh
GAMEPKG_CLANG=clang-18 bash tools/check_takedawn_x87_stack_targets.sh
~~~

The reusable scripts accept `GAMEPKG_X87_STACK_FIXTURE_SOURCE`,
`GAMEPKG_X87_STACK_CORPUS_SOURCE`, `GAMEPKG_WINEPREFIX` and
`GAMEPKG_X87_STACK_LINK_ONLY=1`, mirroring previous checks. Local execution and
Windows link-only modes pass, with strict warnings and UB checks/traps. CI now
adds corpus/fixture generation, MIPS replay and both Windows compiler-profile
links; no remote CI run is claimed. Full suites pass **82 host / 57 Windows-tool
tests**, strict release lint/format and prior arithmetic/load/comparison/store/
MMX/streaming/RAM regressions. A test-only redundant string conversion was fixed
after lint identified it; no runtime behavior was changed to satisfy lint.

The private owned-disc diagnostic admits **670,096 / 810,177** sites (140,081
rejected), adding 658 stack/rounding sites. The closed module grows by 105 to
**17,161 functions / 164 shared tails**: 20,741 provisional, 3,258 emission
rejects, 12,536 edges, 3,580 removed callers and one rejected extent.
Closed source 51,220,824 bytes; body 51,649,849 bytes SHA256
`006d28a814f8e00cb5a288356089a21911be422db71fed8ee188b86c5afe4b68`;
complete-file SHA256
`f13e4b21fe0a4721037d570b149b0a4a5d59a8abf95b030c6dc7e4301652a282`;
report SHA256
`60e1676fb62551993a79528a9675488a32a9edbb5501db46905d9dbef64edd69`.
Linux64/PE32 generated C/reports/complete instruction diagnostics are identical.
Fresh strict O3 ELF64/ELF32/i386 COFF partial modules resolve all generated/
instruction helpers, including stack operations. Windows game C uses Clang
emulated TLS with GCC helpers. Other host/kernel/MMIO references remain external;
the original private prototype hash is unchanged and derived sources stay private.

Remaining x87 control/environment, scale/remainder/transcendentals and other
unsupported operations, actual engine RAM/device/all-worker binding, legacy
callbacks/ordinary-wide-store migration, full segmentation/paging/FP-context/SEH/
kernel/old-new ABI, discovery/recovery/repair/dispatch/source-kit/build and signed
launch/full game/platform fidelity remain required. Readiness stays false.
No assets, executable-derived C or private inventory are published. No guest
crates, player Python, complete game launch or measured FPS gain are claimed.

### Protected-mode x87 environment and control

`FLDCW`, `FLDENV`, `FRSTOR`, `FNSTENV` and `FNSAVE` now use a checked runtime
service. Operand-size prefixes select explicit little-endian14/28-byte
protected-mode environments or94/108-byte complete images. Capstone labels
these operands with fixed widths (28 for environments,4 for full images);
validation checks those labels, while the opcode/prefix determines the actual
transaction span. A host struct is never a guest save image.

Loads wait on old pending exceptions before logical bounds or RAM access.
No-wait saves preserve pending flags in their image. Stores mask exceptions or
reset the FPU only after the whole write succeeds; failed loads/saves preserve
state and destination. Full images store logical ST0..ST7 payloads while tags
remain physical. Nonempty tags are classified from raw register bits; empty
slots retain their payloads. Control changes canonicalize reserved bits and
recompute deferred ES/B. All65,536 control words match this host's hardware.
Reserved PC01 follows its measured64-bit-significand profile, rather than
aborting; qualification on the original Xbox/PentiumIII remains required.

The numerical/save-image oracle passes286,720 hardware cases,33,690 deferred
next-wait faults, old-pending/failing-span rollback and explicit legacy selector
layouts. Linux64/ELF32 and GCC plus Clang-emulated-TLS/GCC PE32 Wine produce
identical286,720-record LE data (436-byte records plus12-byte header), SHA256
`63f0cb042b80b1f5d0697f20603b3ba0424de8a70d6b984c24238f748a3073d8`.
Native and big-endian MIPS32 replay agree. Modern hardware deprecates some
legacy FIP/FDP/selectors/FOP fields: the numerical oracle excludes those fields,
while independent layout/metadata tests check the intended legacy profile.
All four hardware profiles additionally pass8,000 thread-binding checks.

Reviewed host workers must bind their CS/DS/SS/FS selectors. Generated memory
operations select DS/SS/FS and record logical FDP offsets, subtracting FS base
modulo32 instead of saving the translated linear address. This supplies FPU
metadata; it does not implement segment descriptors, paging or engine binding.
The bounded RAM image API admits1..512-byte transactions under the existing
shared lock.526,336 additional tests exercise every failing byte, permission
response, complete rollback and32-bit linear wrap, on native64, ELF32, PE32
Wine and MIPS32 C. Generated logical32-bound tests verify old pending MF before
GP for waiting loads and GP for no-wait saves, with complete state preservation.
Existing74,112 access cases and concurrency/lifetime tests still pass.

Reproduction uses `tools/check_takedawn_x87_environment_targets.sh` and
`tools/check_takedawn_x87_environment_portability.sh`; optional fixture/corpus
inputs follow the other x87 scripts. CI generates synthetic data and replays
it across endian targets, and compiles both Windows TLS profiles. These local
checks do not establish a remote CI result or physical Windows qualification.

PC01-inclusive raw80 tests now pass1,364,800 hardware cases and133,586 wrapped
range responses, with native/MIPS32 replay SHA256
`6f08124195a5a1564b3213ce84ad497e6abd957fc80febffe256355ef45c4e5e`.
Register arithmetic passes1,802,240 state cases/775,321 deferred faults; memory
arithmetic passes2,801,664 cases/1,177,278 deferred faults, including ELF32 and
both PE32 Wine compiler profiles. The complete generator suites pass85 host
and59 Windows-tool tests; strict release lint and formatting pass.

The private module now has17,168 closed functions/164 tails (seven additional
functions),20,748 provisional functions,3,251 emission rejects,12,541 edges,
3,580 removed callers and one rejected extent. Instruction diagnostics admit
670,110 sites and reject140,067. Closed source is51,666,197 bytes; body52,095,397
bytes SHA256 `2664e86413976e2bb8e604eb790e0b97c4d9802d8d797cbf3f304abf798550c8`;
complete52,095,423-byte source SHA256
`6d1ae8cb2ad010366e4fa40be5e1a9291c33e2046081219dacd51a79e2f3f007`;
report SHA256 `2b32b71d4f14ef167c170f91a17e9f9090886e161acaaf498450ea45a4f41ee2`.
Linux/Windows generation and reports are byte-identical. Fresh strict O3
ELF64/ELF32/i386 COFF partial modules resolve all instruction helpers, including
the new environment service. All shared-state services and the changed ext80
wrapper are freshly compiled; only unchanged pure SoftFloat objects are reused.

This supersedes the older control/environment and PC01-pending notes above.
Real/V86 and FXSAVE/SSE contexts, scale/remainder/transcendentals, full guest
fault/SEH/CPU context, actual engine RAM/device/worker/old-new ABI integration,
discovery/repair/dispatch/source-kit/build and signed playable launch remain
open. The existing private game prototype is unchanged; no assets, derived C,
private inventories, player Python, guest crates or FPS claims are added.

### Exact x87 exponent scaling

`FSCALE` now emits a fixed ST0/ST1 operation through the shared arithmetic
service. The integer-only backend truncates ST1 toward zero independently of RC,
ignores PC, normalizes raw80 subnormals, and rounds only at the final extended
range. It uses the existing reviewed SoftFloat rounder; no host floating-point
casts, native128, new third-party sources or player dependencies are added.
NaN/unsupported/infinite/zero cases, source DE and stack-underflow priority use
the same precise deferred-commit protocol as other x87 arithmetic.

A single exponent wrap by24,576 can still leave the result out of range. Hardware
then commits signed infinity or zero with PE, independently of RC; it does not
perform another directed-rounding/denormalization step. This differs from the
ordinary masked range result, which honors RC. Tests check both responses and
C1. Finite scale magnitudes of65,536 or larger cannot be rescued by one wrap;
using that bound internally preserves every final response without overflowing
host integer conversions. Explicit infinity operands follow their own rules.

The generated-instruction oracle passes2,089,999 hardware/full-state cases and
756,350 deferred next-wait faults, an old-pending gate, and8,000 independent
workers. It covers all64 masks, four PC/RC bit patterns, all TOP/empty-source
combinations, sticky C1/SF/status, special and random raw80 values, plus every
integer scale from−65,537 through+65,537 against five range-boundary inputs.
All registers, control, tags, defined status flags and legacy metadata are
checked; undefined C0/C2/C3 retain initial bits in the virtual state. Numerical
hardware snapshots declare every XMM register overwritten by FXRSTOR.

Linux64, real ELF32 and GCC plus Clang-emulated-TLS/GCC PE32 Wine profiles have
identical explicit-LE full-state data:2,089,999 records of256 bytes plus12-byte
header,535,039,756 bytes, SHA256
`9f566bea797b977d11ed02614fedd4f4abefa56f76d5ced2ed8e9f4ef0f6e224`.
Native and big-endian MIPS32 replay match all state and756,350 queued faults,
with unrelated GPR/flags/FS/DF preserved. Reproduction uses
`tools/check_takedawn_x87_scale_targets.sh` and
`tools/check_takedawn_x87_scale_portability.sh`. Local full generator suites
pass88 host/61 Windows-tool tests, strict release lint and formatting. CI adds
synthetic generation/endian replay/Windows linking; no remote run is claimed.
Physical legacy Xbox/PentiumIII and physical Windows qualification stay open.

All13 current FSCALE sites now emit. Diagnostics admit670,123 instructions and
reject140,054. Two more provisional functions become available (20,750 total),
but their remaining dependencies are unsupported: the closed selection stays
17,168 functions/164 tails and its source is byte-identical to the preceding
control/environment stage. There are3,249 emission rejects,12,549 edges,3,582
removed callers and one rejected extent. Report SHA256 is
`81d1cddab48ad5d150d0365856217c0c30ab52a5fdd7a6730091dd95e8072f1a`;
Linux/Windows source, report and complete instruction diagnostics match.
All shared-state helpers and the ext80 wrapper are freshly compiled and linked
on ELF64/ELF32/i386 COFF; unchanged verified game objects and pure SoftFloat
objects are reused. All generated/instruction services resolve. The unchanged
closed game body does not exercise FSCALE; the dedicated emitted oracle does.

Remainders/transcendentals and the previously listed engine, CPU-context/SEH,
ABI, discovery/repair/dispatch/source-kit/build, signed launch and full-game
requirements remain open. The private original prototype is unchanged and
readiness remains false. No assets, derived code or private inventories are
published, and no complete-playability or performance gain is claimed.

### Exact x87 partial and nearest remainders

`FPREM` and `FPREM1` now lower through fixed ST0/ST1 arithmetic operations6/7.
An integer-only raw80 backend performs bounded unsigned64 shift/subtract division,
tracks quotient magnitude modulo8, and normalizes an exact remainder. FPREM
truncates the quotient; FPREM1 rounds it to nearest-even, independently of PC/RC.
The sign of zero remains the dividend's sign; nonzero nearest remainders can
change sign. No new SoftFloat sources, native128, host floating casts or player
runtime dependencies are introduced.

Large exponent differences use a partial step of32+(D modulo32) bits. Intel
permits an implementation-dependent32..63-bit step, and this profile matches the
measured host. C2 indicates incomplete reduction and repeated steps converge;
completed C0/C3/C1 contain quotient-magnitude bits2/1/0. Starting an instruction
clears C2 even for invalid or denormal faults. Invalid/NaN/pre-denormal responses
retain C0/C3 and clear C1; an old pending fault precedes all state/metadata changes.
Tiny exact results can queue unmasked UE with a wrapped exponent, including a
tiny dividend modulo infinity. No new PE is raised. Stack/DE/IE priority, masked
indefinite values, signed zeros and committed versus suppressed results are
checked rather than inferred from host library remainder functions.

The emitted-code suite passes4,136,989 hardware/full-state cases,1,183,892 queued
next-wait faults, an old-pending gate and8,000 workers. Coverage includes both
operations, all64 masks/four PC/four RC/TOP/empty combinations, sticky flags,
special/random raw80 pairs, signed small-integer quotient/tie cases, odd tiny
divisors across127 exponent boundaries with DE/UE priority, and256 repeated
reduction chains. All condition bits, physical registers/tags/control and legacy
selectors/pointers match the measured profile. SoftFloat/private math state and
unrelated GPR/EFLAGS/FS/DF remain unchanged.

Linux64, real ELF32 and GCC plus Clang-emulated-TLS/GCC PE32 Wine produce identical
LE data:4,136,989 records of260 bytes (operation plus two128-byte states), plus a
12-byte header;1,075,617,152 bytes, SHA256
`c98dfc8a79fc1f1b006230bd281c4bbb2114630f35954952a515927203c7d1a6`.
Native and big-endian MIPS32 replay agree on every state and1,183,892 deferred
faults. Reproduce with `tools/check_takedawn_x87_remainder_targets.sh` and
`tools/check_takedawn_x87_remainder_portability.sh`. Full suites pass91 host and
63 Windows-tool tests; strict lint/format, shell checks and CI YAML parsing pass.
CI adds synthetic corpus generation, endian replay and Windows profile linking;
remote CI and physical Xbox/PentiumIII/Windows qualification are not claimed.

The hardware-test compiler contract is also corrected at37 older FXRSTOR sites
across10 x87/MMX/streaming oracle files. Every overwritten XMM register is now
listed, preventing compiler-held C snapshots from being silently overwritten.
The full host suite passes, and regenerated prior raw80/memory-arithmetic/
comparison corpora retain their exact previously qualified hashes:
`6f08124195a5a1564b3213ce84ad497e6abd957fc80febffe256355ef45c4e5e`,
`c3449a7c73c8525091d8106b377944ffed0b15db6daa0fc1db4212519de8457b`,
`b8d60b3995937eef9365df6db01f22fb85b4ca404969cb3d4ab28c7ea764f665`.
This corrects the test harness; it does not claim a previously observed runtime
numerical failure or change the pinned SoftFloat sources.

The current backed game inventory contains no FPREM/FPREM1 sites. Source,
instruction coverage and graph report remain byte-identical to the scaling
stage (17,168 closed functions/164 tails;670,123 admitted/140,054 rejected sites).
Linux/Windows generation remains identical. All shared-state services and the
changed ext80 wrapper are freshly compiled/relinked on ELF64/ELF32/i386 COFF,
reusing only verified byte-identical game objects and pure SoftFloat objects.
Every instruction/numerical reference resolves. The dedicated generated oracle
exercises remainders; the unchanged closed game body does not.

This completes the scoped remainder service, superseding earlier remainder-
pending notes. Transcendentals/other x87 formats, full CPU-context/SEH/ABI,
actual engine RAM/device/worker integration, discovery/repair/dispatch/source-kit/
build, signed playable launch and original full-game/platform requirements stay
open. The original private prototype is unchanged and readiness remains false.
No assets, derived sources or private inventory are published, and no complete
playability or performance gain is claimed.

### Portable wider intermediates for transcendental work

The reviewed math selection now includes26 additional original SoftFloat3e
multiword binary128 files (71 C sources/78 pinned source-header-license files
total), with the same BSD3 notices and archive hash. The internal
`gamepkg_x86_f128` helper supplies add/subtract/multiply/divide, unsigned32
constants and raw80 conversions through logical little-endian32-bit words.
Production uses no native128/compiler floating types or new guest dependencies.
Private library state is restored; host rounding and flags are preserved.
Source DE, x87 stack/metadata, C1/wrapped-fault handling and instruction commits
remain the caller's responsibility. This is a precision foundation for the
remaining transcendentals; none is newly admitted by this helper alone.

Testing found and fixed an unsigned leading-bit shift and two big-endian
common-NaN conversions in the original sources. The original shift invokes
undefined behavior at distance15. The NaN routines assumed &v0 starts the full
payload; on big-endian systems that accesses beyond the object. Converters now
use logical fields. An actual MIPS32 guard regression fails with the original
write routine (exit134, adjacent canary overwritten) and passes the adaptation;
round-trip and adjacent-memory-independence checks also pass. Every adaptation
and resulting hash is recorded; existing selected source files are unchanged.

660,646 independent GCC binary128 arithmetic/conversion reference cases pass
Linux64 GCC13 and real ELF32 GCC11 with UB checks, covering four rounding modes,
zeros, infinities, normals/subnormals, range/precision/invalid flags, special NaN
conversions and random bit patterns. Eight workers add32,000 operations. Invalid
host parameters preserve outputs, and unsupported raw80 values yield the
specified canonical invalid result. C long-double reference inputs canonicalize
pseudo-denormals to their equal exponent1 value; production receives original
raw bits. Independent reference operations occur only in the maintainer fixture.

A byte-identical LE corpus has660,646 records of60 bytes plus12-byte header,
39,638,772 bytes, SHA256
`69e17da0921d78e2b862ad6b208342c4d3b0b3c140065ac6a6e6f99e68acf183`.
Native and big-endian MIPS32 replay match every value/flag, as do integer-only
GCC and Clang-emulated-TLS/GCC Windows32 Wine replay profiles. Windows replay does
not require compiler binary128 support. Reproduce using
`tools/check_takedawn_f128_portability.sh` and
`tools/check_takedawn_f128_targets.sh`; GAMEPKG_GCC32 selects the multilib GCC
reference compiler. CI generates/replays this synthetic data and links both
Windows profiles. No remote CI or physical Windows/gameplay result is claimed.
Full suites pass93 host/64 Windows-tool tests; updated78-file pin gates and
final wider/guard cases are rerun after adaptations, with strict lint/format,
shell and CI YAML parsing passing.

The source frontend is unchanged:17,168 closed functions/164 tails and all
source/report/instruction coverage remain the same. The new private module uses
verified prior generated source/game objects, with all shared-state wrappers and
new wider sources freshly compiled and linked on ELF64/ELF32/i386 COFF. Every
numerical helper resolves; there are no compiler binary128 arithmetic/conversion
references in the production module. The new helper is not yet called from a
generated game instruction. Transcendental handlers/evaluation, other x87 forms,
full engine/CPU/SEH/ABI/discovery/repair/dispatch/build/signed-launch and original
full-game/platform requirements remain open. Readiness is false; the original
private prototype and media remain unchanged. No game assets/derived sources/
private inventory are published and no complete-playability/FPS claim is made.

### F2XM1 evaluation and shared runtime integration

D9F0 now lowers to operation0 of gamepkg_x86_x87_trans, with exact encoding and
operand checks. The service waits for old pending conditions before metadata
changes, uses the shared raw80 physical stack/tags and legacy FIP/FOP/selectors,
and preserves unrelated registers and private math state. Empty/unsupported/
NaN/zero/infinite inputs, DE/stack priority, masked results and unmasked wrapped
underflow/deferred precision responses are handled before publication. PC is
ignored for the 64-bit-significand result. Finite inputs outside [-1,1] follow
the measured retain-input/PE profile; the architecture leaves them undefined.
The +1 endpoint follows measured RC/C1 behavior separately from the reference.

The numerical evaluator adapts pinned Bochs revision
b0c88410501f48bbe1a966cab1c0592fb504915a through portable multiword arithmetic.
It returns a binary128 intermediate and an exact exponent adjustment. A new
raw80 final-rounding helper supplies C1 and wrapped exponent observations;
its unscaled values/flags also match the independent GCC conversion reference
at every PC in the existing660,646-case wide arithmetic fixture. No host float,
compiler128, player C++, MPFR, guest dependency or Python is introduced.
The isolated evaluator is LGPL-2.1-or-later, with original notices,41 original
source pins, full license and corresponding source/relink notes under
runtime/transcendentals. The original C++ implementation is built only as an
independent maintainer numerical reference.

Approximate transcendental last bits vary between CPU implementations. For
nonzero normal inputs strictly inside (-1,1), tests demand exact agreement
with the independent pinned profile; physical status/exception/stack responses
and unrelated register bits remain independently checked. Modern hardware has
5,937 numerical/C1 differences in this146,304-state corpus. MPFR512 checks
strict Intel bounds (<1 ulp nearest, <1.5 ulp other RC);279,808 adjacent pairs
also pass monotonicity, including table, quarter and tiny-path transitions.
This is not bit-for-bit qualification on the original Pentium III/Xbox.
All64 masks/four PC/four RC, every TOP, absent operands, special values, gradual
and wrapped underflow, sticky state, old-pending suppression and8,000 worker
cases pass.44,064 deferred next-wait faults are checked. Windows physical
instruction behavior is not inferred from replay.

The explicit-LE corpus contains146,304 before/after state pairs,256 bytes each
plus12-byte header:37,453,836 bytes, SHA256
be701cbde79d28f3ce12e6bba430426edbc1d1e7dd750f9713b7b1ed27a6c48c.
Every full state and queued fault replays on Linux64, actual ELF32,
GCC and Clang-emulated-TLS/GCC PE32 under Wine, and big-endian MIPS32 under QEMU.
Reproduce via tools/check_takedawn_x87_f2xm1_targets.sh and
 tools/check_takedawn_x87_f2xm1_portability.sh; GAMEPKG_GCC32 selects a multilib
compiler and GAMEPKG_X87_F2XM1_CORPUS_SOURCE reuses the qualified data.
The host oracle needs MPFR/GMP development files only for maintainers.
CI generates the corpus, replays it on big-endian32 and links both Windows
profiles. Full host/Windows-tool suites pass97/67 tests; strict lint passes.
No remote CI, physical Windows or original target CPU qualification is claimed.

All7 current F2XM1 sites now emit, yielding670,130 admitted/140,047 rejected
instructions. Provisional functions rise to20,755, emission rejects fall to3,244,
and dependency closure now admits17,173 functions (+5) with164 tails and12,550
edges;3,582 callers and1 extent remain rejected. Linux/Windows source and report
bytes agree. Closed source bytes51,707,785; streamed body52,137,110 bytes SHA256
e7d9505cb9a892330ade5cf04c3b8f20d1c00ca579a70aa00115c7f560bfccb0;
complete source52,137,136 bytes SHA256
7062940fcefe68edad3f72d78c4cafd59bc23ae379d33d6b6bc28f2b298437b2;
report SHA256 dc00bd5d57e8f42ffb0230153d3be708c38b0128c88a0772a359afe437b878cf.
The entire changed generated body is freshly compiled at O3 and partially linked
with current services on ELF64/ELF32/i386 COFF; all numerical/instruction services
resolve, without compiler binary128 arithmetic references. This is not a full
executable link. Other transcendental/BCD instructions, full CPU/SEH/ABI and
engine/device/all-worker integration, discovery/repair/dispatch/source-kit/build,
signed GamePKG launch and complete game/platform fidelity remain required.
Readiness remains false; original private prototype/media are unchanged.
No game assets, derived sources or private inventory are published.

### FYL2X logarithm/product and stack integration

D9F1 now emits shared transcendental op1. It waits before metadata, reads ST0
and ST1, writes ST1 and pops only when the instruction commits. The service
handles both empty operands, unsupported encodings/NaNs, signed zeros,
infinities, negative logarithm arguments, source denormals and IE/DE/ZE
priority. Unmasked preconditions suppress the write/pop; range results use
wrapped exponents and post-computation precision delivery. GPR/EFLAGS/FS,
unrelated FP registers and private numerical controls remain unchanged.

The isolated LGPL evaluator adapts the same pinned Bochs revision's FYL2X
reduction tables and 67/64-bit intermediate operations. Normalization keeps
both source mantissas in range; y's exponent is applied only at final raw80
rounding. Exact powers of two use an exact signed integer log2 before the
product. They compare bit-for-bit against physical FYL2X, rather than the
approximate reduction model. A separate192-case power probe found44 original
model numeric differences versus hardware; the primary full-state fixture
also checks exact-power inputs at exponent boundaries. No host floating/compiler128 types or new
player dependencies enter production.46 original files are hash-pinned.

1,585,260 state cases and650,192 deferred next-wait faults pass the fixed
profile and exact physical stack/status/exception comparisons.8,000 workers
also pass. All64 exception masks, four PC/RC, all TOPs/empty operands,
special-value pairs and random full-range products are checked. Reduction-table
and near-one polynomial boundaries are tested on both sides, including tiny
and large source exponents.133,116 adjacent log2 pairs pass monotonicity.
General finite non-power results match the independently compiled original
numerical profile.32,287 corpus rows differ numerically/C1 from modern hardware;
this does not relax defined status/exception/commit comparisons. MPFR512
checks the strict <1ulp nearest/<1.5ulp directed bounds. Masked mathematical
overflow clamps are checked by state/reference comparisons rather than an
in-range approximation bound; wrapped exponent rescaling uses newly unmasked
range bits, independently of old masked sticky flags. Original Pentium III/
Xbox bit-for-bit approximation qualification remains unverified.

Full-state explicit-LE replay passes Linux64/actual ELF32, GCC and
Clang-emulated-TLS/GCC PE32 Wine, and MIPS32 big-endian QEMU, including all
650,192 queued faults. Corpus1,585,260x256+12=405,826,572bytes SHA256
3394e9e75117542df1f7e84781420fd43db753c5817381baf7d262418aa5be3d.
Reproduce using tools/check_takedawn_x87_fyl2x_targets.sh and
 tools/check_takedawn_x87_fyl2x_portability.sh; the corresponding
GAMEPKG_X87_FYL2X_CORPUS_SOURCE variable reuses this asset-free data. CI adds
corpus generation, big-endian replay and both Windows link gates. Existing
F2XM1 tests remain unchanged numerically. Old negative admission fixtures now
select still-unsupported FPTAN instead of newly supported FYL2X.

All30 actual FYL2X sites emit, with670,160 admitted/140,017 rejected instruction
forms. Closed selection grows by18 to17,191 functions/164 tails; provisional
20,769, emission rejects3,230, dependency edges12,572, removed callers3,578 and
one rejected extent. Closed source51,906,117 bytes; body52,335,892 SHA256
7a000ba3a56a8b0e65fd2c723defb3ea151097baa4f2e2765fca462fce4c0591;
complete52,335,918 SHA256
c902554eb35f5d689941131442e18016e580d13a127f394d4a2539dcf722ffb1;
report8196ef3626907ae6df39f7ba65ed2b17a26340d32666c830be96b2240c14d7fb.
Linux/Windows generation/report bytes agree. Fresh O3/strict-warning compilation
of the entire changed body and partial links on ELF64/ELF32/i386 COFF resolve
all numerical/instruction services. This is not full game executable linking.
FYL2XP1/other transcendentals/BCD, full fault-context/SEH/CPU/ABI and engine/
device/all-worker integration, discovery/repair/dispatch/source-kit/build and
signed playable GamePKG launch remain open. Readiness false; private original
prototype/media unchanged. No assets, game-derived C or private inventory are
published; no complete-playability or performance claim is made.

### FPATAN quadrant and tiny-ratio integration

D9F3 now validates exact encoding and lowers to shared transcendental op2.
Binary operations both use ST1 as the destination, with a pop only after
commit. FPATAN handles signed zero/infinity quadrants and the pinned legacy
pi fractions, NaN/unsupported operands, DE priority, masked/unmasked stack
faults and wrapped-underflow/precision responses. Old pending delivery precedes
metadata changes; unrelated FP/GPR/flags and private numerical state persist.

The isolated portable evaluator adapts the same pinned Bochs revision's
33-entry atan table, 67/64-bit polynomial and quadrant reconstruction. Exact
bounded unsigned reduction computes32v=k*u+r, preserving the65th carry bit;
k is bounded0..32 without estimates or native128. Magnitudes are normalized
before evaluating their ratio. Fifty original files, including the independent
reference remainder source and constants, are pinned without changes. LGPL
notices and corresponding source/relink notes remain with this component.
No player C++/MPFR/compiler floating types, guest dependencies or Python are
introduced.

Qualification found a real tiny-ratio problem in the original profile: an
exact dyadic q can be returned unchanged even though atan(q) is infinitesimally
smaller in magnitude. For x=1,y=minimum-normal and RC down/toward-zero, this
misses both predecessor rounding and the wrapped underflow response. The C
adaptation keeps a nonzero lower tail for an exact tiny ratio, and normalizes
all tiny divisions before evaluating them so intermediate binary128 gradual
underflow cannot reduce precision. The strict physical oracle covers result
bytes in this corrected dyadic domain; an independent MPFR ratio comparison
checks C1, because modern hosts can report different approximation flags for
the same tiny result. General finite results still match the
independently compiled original profile. Approximation differences do not
replace defined physical status/exception/commit checks. Original source
files are unchanged; source and tests document the correction.

Hosted Intel CPUs expose both C1 and underflow differences for x=1,
y=minimum-normal. At CW0x40 (case85), numerical bytes agree but C1 can differ.
At CW0x440 (case87701), Intel returns minimum normal without UE, whereas the
AMD physical result and the mathematically corrected implementation wrap the
underflow result. For exact dyadic q with at most64 significant bits and
|q|<2^-40, 0<|q|-|atan(q)|<|q|^3/3 cannot cross a distinct raw80 rounding
boundary. The independent MPFR oracle therefore supplies a strict lower tail,
rounds at unbounded 64-bit precision to determine UE and then applies subnormal
rounding or wrapping. It derives result bytes, C1, UE and deferred-error flags;
all other physical state checks remain. Unsupported operands retain physical
invalid-operation assertions. MPFR512 accuracy and quadrant monotonicity remain.

Reversed-host-C1 and captured-Intel-underflow variants pass the complete matrix.
Corrupt implementation C1, UE and result-bit probes still fail at case85 or
case87701. The 616,852,492-byte AMD replay corpus remains byte-for-byte unchanged
(SHA-256 fa7632c0b879c45eb541a066d20f6f47d0be7f5272fcbb1124b90e3af4d2dabd).
Variants cannot overwrite the canonical corpus. No runtime, source-kit or
current native-port code changes are involved. Actual Intel CI confirmation of
the new oracle remains separate from these local results.

2,409,580 state cases/1,085,424 deferred next-wait faults pass, plus8,000 worker
cases. All64 masks/four PC/four RC/TOP/empty/special pairs, full-range ratios,
all quadrants and every half-table breakpoint with adjacent operands are
covered.264,192 adjacent-input quadrant monotonicity pairs pass. MPFR512
checks strict <1ulp nearest/<1.5ulp directed accuracy, with gradual/wrapped
underflow normalization.43,842 corpus rows differ numerically/C1 from modern
hardware in the general approximation domain; original Pentium III/Xbox
bit-for-bit approximation qualification remains unverified.

Complete explicit-LE before/after state replay passes Linux64/actual ELF32,
GCC and Clang-emulated-TLS/GCC PE32 Wine, and big-endian MIPS32 QEMU, including
all queued faults. Corpus2,409,580x256+12=616,852,492bytes SHA256
fa7632c0b879c45eb541a066d20f6f47d0be7f5272fcbb1124b90e3af4d2dabd.
Use tools/check_takedawn_x87_fpatan_targets.sh and
 tools/check_takedawn_x87_fpatan_portability.sh, optionally with
GAMEPKG_X87_FPATAN_CORPUS_SOURCE. CI adds corpus generation, big-endian replay
and both Windows link gates. Full103 host/71 Windows-tool tests and strict
lint pass. Existing F2XM1/FYL2X semantic tests retain their results. No remote
CI, physical Windows or actual-game endian qualification is claimed.

All57 game FPATAN sites emit;670,217 forms are admitted/139,960 rejected.
Provisional functions20,781/emission rejects3,218/dependency edges12,606/
removed callers3,582/rejected extent1. Closure grows by8 to17,199 functions
with164 tails and52,213,134 closed-source bytes. Linux/Windows source and
reports agree. Body52,643,109 bytes SHA256
ec20525fd3907a134888397622f7d40782826a53032f44a367969b3f320acab6;
complete52,643,135 SHA256
d5f820f5600de1d9f192d1caf1e90c1b7c0e3258ef5fc56aae8a4a89c35e9fee;
report b2e7d265ef84bb6100929b6f027795299b9b5bc21137256c370800fc5ddbeac9.
Fresh O3/strict-warning compilation of the entire changed body and partial
links on ELF64/ELF32/i386 COFF resolve all numerical/instruction services,
without compiler binary128 arithmetic references. This is not a complete
executable link. Remaining trig/FYL2XP1/BCD, full CPU/fault-context/SEH/ABI,
engine/device/all-worker bindings, discovery/repair/dispatch/source-kit/build
and signed playable launch remain open. Readiness false; original private
prototype/media unchanged. No assets, derived C or private inventory published;
no complete-playability or performance claim is made.

### Standalone FSIN and FCOS

D9FE/D9FF now validate exact encoding/no operands and lower to shared trans
ops3/4. Both are unary ST0 operations. Only binary ops1/2 write ST1/pop; this
classification is explicit. Old pending delivery precedes metadata, source/
stack exceptions follow the established priority, and unsupported/NaN/infinite
inputs and signed zeros follow physical results. C2 is a defined range result:
finite |x|>=2^63 leaves the operand unchanged and sets C2 without computation;
accepted arguments clear it. PC does not reduce final result precision.

The isolated C component gamepkg_fsincos implements standalone polynomial/table
paths from the same pinned Bochs revision. Its integer192-bit restoring
reduction preserves the129th carry, uses a fixed66-bit pi/2 and keeps n modulo4.
Explicit logical64/32-bit conversions avoid host floating/native128, host byte
order or C++ runtime dependencies. Fifty-two original files are pinned. The
complete original C++ file also contains other variants, but this adapter and
frontend implement standalone FSIN/FCOS only; combined FSINCOS and FPTAN remain
unsupported. LGPL notices/source-relink obligations remain with the component.

Tiny nonzero sine/cosine need a lower tail even when a numerical approximation
would simply return x or1. At x=minimum-normal, downward/toward-zero sine must
return its predecessor and take the wrapped underflow path when unmasked;
cosine rounds toward the predecessor of1. The adapter preserves that tail
before final raw80 rounding. Corrected tiny inputs compare exactly against
physical hardware. General inputs match an independently compiled original
standalone profile. Closing C1 includes the sign of an absorbed nonzero tail.
No generic game permission, dependency, resource limit or player tooling changes.

1,077,056 state cases/447,552 deferred next-wait faults and8,000 workers pass.
All64 masks/four PC/four RC, every TOP/empty combination, special values, tiny
and exponent/range boundaries and random full-range inputs are covered. MPFR512
independently selects neighbors of fixed-pi fold boundaries at large exponents;
quarter/table and2^63 cutoffs are also tested.8,704 adjacent-input local
monotonic pairs pass.2,260 numerical/C1 differences from modern hardware are
tracked in the general fixed-profile domain; defined physical stack/status/
exception/commit comparisons are unchanged. Original Xbox/Pentium III
bit-for-bit approximation qualification remains unverified.

Accuracy checks distinguish two effects. They require strict <1ulp nearest/
<1.5ulp directed error for sin/cos of the exact legacy-reduced argument. They
also compare with true sin/cos using the independently computed phase-drift
bound n*|true_pi/2-legacy_pi/2| plus the local approximation bound (sin/cos are
1-Lipschitz). This does not claim true sin/cos stays within one ulp at large
arguments. For extreme tiny values, MPFR512 can absorb a curvature term;
tests recover the mathematical binade below the limiting power of two so the
ulp check does not double its allowance. The analogous FPATAN accuracy helper
is corrected too, with both hardware/reference suites rerun successfully.

The final explicit-LE corpus adds a4-byte operation selector to each128-byte
before/after state pair:1,077,056x260+12=280,034,572bytes SHA256
f01fb9f32e03936420ef18495663352f998a0b20a4df387b20cf2c5638f87778.
Tightened accuracy checks regenerate identical bytes. Full states and all
447,552 faults replay on Linux64/actual ELF32, both GCC and
Clang-emulated-TLS/GCC PE32 Wine, and MIPS32 big-endian QEMU. Reproduce via
 tools/check_takedawn_x87_sincos_targets.sh / portability.sh and
GAMEPKG_X87_SINCOS_CORPUS_SOURCE. CI adds generation/endian replay/Windows
links; full106 host/73 Windows-tool tests and strict lint pass, with final
FPATAN/standalone-oracle reruns after accuracy changes. No remote CI,
physical Windows or whole-game endian qualification is claimed.

All65 FSIN and63 FCOS sites emit:670,345 admitted/139,832 rejected forms.
Provisional functions20,809/emission rejects3,190/edges12,648/removed callers
3,578/rejected extent1. Closure adds32 functions to17,231/164 tails, with
53,086,352 closed-source bytes. Linux/Windows source/report bytes agree.
Body53,517,127 SHA256
204a3ee347cde420ec692cbb94477a830f47abd96970c9fd9d34252e273937b8;
complete53,517,153 SHA256
ccbd9a49978c2c9a804446e10130336dc8b2b3368a3c012081503c0aeed49e78;
report1d692f33cce8db45f30457d06aac88fdd0cf18e8356179f1705048c30d98ecbd.
The changed entire body is freshly compiled O3/strictwarnings and partially
linked with current services on ELF64/ELF32/i386 COFF; every numerical/instruction
service resolves without compiler binary128 math references. This is not a
complete executable. Combined FSINCOS/FPTAN/FYL2XP1/BCD and full CPU/faultcontext/
SEH/ABI/engine/device/all-worker/discovery/repair/dispatch/sourcekit/build/signed
playable integration remain open. Readiness false; original private prototype/
media unchanged. No assets, derived C/private inventory or playability/FPS
claims are published.

### Combined FSINCOS and FPTAN stack results

D9FB/D9F2 now validate exact encoding/no operands and emit shared trans ops5/6.
They write the sine/tangent to the old ST0, decrement TOP and write cosine/one
to the new ST0. Empty source takes stack-underflow priority; an occupied push
slot gives overflow/C1. Masked stack faults publish two indefinites, unmasked
faults publish neither. Stack checks precede range/NaN computation. Finite
|x|>=2^63 leaves TOP/registers unchanged and sets C2; old pending faults precede
metadata. QNaN and masked invalid produce two NaNs, including FPTAN's extra
slot. DE suppresses both writes/push when unmasked. Post-computation range/PE
responses commit their specified wrapped values before deferred delivery.

The numerical component now includes the original separate combined head/tail
and tangent reconstruction layouts. It does not substitute standalone FSIN/FCOS
results for either instruction. FSINCOS C1 describes the sine lane, including
its wrapped underflow result. The independent original C++ helper retains the
cosine C1, so its test bridge independently observes sine magnitude versus a
truncation-mode reevaluation, without changing original sources. FPTAN closing
division retains the wider rounding direction only when narrowing has no
fraction of its own. These details are covered by exact state comparisons.

FSINCOS tiny results use the already-qualified lower tails for sine/cosine.
FPTAN instead retains the pinned P5/P6 tiny numerical profile (x, with required
precision/source/range flags). Modern hardware's tiny tangent output/C1 varies
with the mantissa; attempts to infer one universal modern tiny-tail rule fail.
The fixture therefore checks this selected profile bit-for-bit and independently
checks both its mathematical error and observed modern results. It preserves
exact hardware exception/underflow/stack/commit comparisons; profile-dependent
C1 is checked against the selected numerical profile. No approximation mismatch
is allowed to hide changed exception bits or stack effects. Original Pentium
III/Xbox bit-for-bit numerical qualification remains unverified.

1,077,056 state cases/452,576 deferred next-wait faults and8,000 workers pass.
All64 masks/four PC/four RC/TOP/source-empty/push-occupied combinations,
special values, tiny/range/table/fold boundaries and random accepted inputs
are included.13,056 local monotonic pairs exercise combined sine/cosine and
tangent cores. Invalid numerical API arguments, null/aliased pair outputs and
unsupported domains preserve sentinel outputs.51,892 numerical/C1 differences
from modern hardware are counted, including30,343 tiny-tangent cases. MPFR512
checks strict local <1ulp nearest/<1.5ulp directed bounds. The full true-function
phase-error bound for tangent uses
|sin(A-B)|/|cos(A)cos(B)| <= |A-B|/|cos(A)cos(B)|, accounting for pole
amplification separately from the local approximation bound. Combined sine/
cosine retain their independently checked legacy-pi drift bounds. No true
full-range one-ulp trigonometric accuracy claim is made.

The explicit-LE corpus is1,077,056x260+12=280,034,572bytes SHA256
092847e5d9f253e889b52c47d1f7ffc3ec7b33849e4af5d72c550096af4f133e.
Full before/after state, operation selector and all452,576 queued faults replay
on Linux64/actual ELF32, GCC and Clang-emulated-TLS/GCC PE32 Wine, and MIPS32
big-endian QEMU. Reproduce with tools/check_takedawn_x87_trig_push_targets.sh
and tools/check_takedawn_x87_trig_push_portability.sh, optionally supplying
GAMEPKG_X87_TRIG_PUSH_CORPUS_SOURCE. CI adds generation/endian replay/Windows
link gates. Full109 host/75 Windows-tool tests and strict lint pass; formatting,
shell and CI YAML checks pass. No remote CI, physical Windows or whole-game
endian qualification is claimed. Existing standalone/other math suites pass.

All3 FSINCOS/6 FPTAN sites emit, giving670,354 admitted/139,823 rejected forms.
Closure adds2 to17,233 functions/164 tails; provisional20,815/emission rejects
3,184/edges12,667/removed callers3,582/rejected extent1. Closed source53,162,728
bytes. Linux/Windows source/report bytes agree. Body53,593,553 SHA256
f9767f9c8da77e1975c37647ff0941ff4fcce13b4769eb1f31c044d2597e7956;
complete53,593,579 SHA256
9fd75cdd7f71882666956d14bbca3b748c2c7dec2fa2bb298da17e16d9a27b54;
report73dc32327b78014bf5c8f5ce906d0e46ceccc3106c74669c2825a57af92a6fc3.
The changed entire body is freshly compiled O3/strictwarnings and partially
linked with current services on ELF64/ELF32/i386 COFF; all numerical/instruction
services resolve, without compiler binary128 arithmetic references. This is
not complete executable linking. FYL2XP1/BCD and full CPU/faultcontext/SEH/ABI/
engine/device/all-worker/discovery/repair/dispatch/sourcekit/build/signed
playable integration remain open. Readiness false; original private prototype/
media unchanged. No assets/derived C/private inventory published; no full
playability or performance claim is made.

### FYL2XP1 and retained closing-round direction

D9F9 now validates exact encoding/no operands and emits trans op7: ST1 receives
y*log2(1+x), then pops only on commit. Binary classification includes1/2/7;
trigonometric C2/range/push handling stays restricted to3..6. Signed zeros,
infinities/NaNs/unsupported operands, DE/stack priority and masked/wrapped/
deferred responses are qualified. The numerical path retains x separately
near zero, uses normalized mantissas and delays both exponents to closing, so
1+x does not erase tiny x and a tiny intermediate can still yield a normal
product with large y. Exact representable1+x powers use integer logarithms,
avoiding table residuals that can change product range behavior.

The original FYL2X table/polynomial constants and pinned reference already
contain this operation; no new upstream source, guest dependency or player
tooling is added. The architectural accuracy domain is
|x|<=1-sqrt(2)/2, as specified by the [Intel instruction reference](https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-vol-2a-manual.pdf).
Broader finite inputs have explicitly tested deterministic legacy behavior;
no mathematical accuracy guarantee is claimed outside the specified interval.
Independent MPFR512 checks use log1p before dividing by ln2, preserving tiny
arguments. Both signs and immediate neighbors of the irrational domain
boundary are constructed independently at512 bits. Exact-log special cases
are compared directly with hardware instead of an uncorrected table residual.

Algebraically constructed multiply operands also exposed a closing C1 loss in
FYL2XP1, existing FYL2X and the F2XM1 tiny path. A113-bit multiply can round onto
an exactly representable64-bit result, leaving no narrowing fraction to record
its preceding rounding direction. All three original-profile regressions fail
before the correction, with unchanged result bits and a missing C1. The shared
adapter now observes truncation-mode reevaluation only for such exact-wide
results and retains its rounding direction if final narrowing has no fraction;
wrapped exponent results retain the hint independently. F2XM1 applies this to
its tiny multiply only: its general reconstruction deliberately discards the
wider add's hint. No ordinary narrowing decision is replaced. Constructed
cases span all PC/RC/masks/signs and FYL2XP1 wrapped-underflow responses.

FYL2XP1 passes1,912,940 states/780,384 deferred next-wait faults,297,984 local
monotonic pairs and8,000 workers. This includes special pairs, all masks/PC/RC/
TOP/empty states, full-range products, polynomial/table/tiny thresholds,
domain-boundary neighbors and invalid numerical API argument preservation.
42,154 numerical/C1 differences from modern hardware are tracked against the
fixed approximation profile; exception/stack/commit checks remain physical.
Original Pentium III/Xbox bit-for-bit numerical qualification remains open.
Expanded F2XM1 passes154,496 states/48,160 queued faults (14,129 profile
numerical/C1 differences), with279,808 monotonic pairs; FYL2X passes1,593,452
states/654,288 faults (32,287 differences), with133,116 monotonic pairs. Each
adds8,000 workers. The8192 new prefix records in each earlier corpus leave
every older record byte-identical, independently compared after skipping the
new prefix and header count.

Explicit-LE full-state corpora, each with256-byte records and12-byte header:

- FYL2XP1:489,712,652 bytes SHA256
  0db5e68e1b3544c6d022da5498174015c6b5108b4586c671c4be5a886713e575.
- F2XM1 expanded:39,550,988 bytes SHA256
  1a00ad873c6047417daf773c7f0b89e644bd950f506717a261305fdc6012dc01.
- FYL2X expanded:407,923,724 bytes SHA256
  a30f5543ed4148cd66465b20a300879e9ec209e4dd67e7f4513e240fcf8d2830.

All states/faults replay on Linux64/actual ELF32, both GCC and
Clang-emulated-TLS/GCC PE32 Wine, and MIPS32 big-endian QEMU. The new
 tools/check_takedawn_x87_fyl2xp1_targets.sh / portability.sh accept
GAMEPKG_X87_FYL2XP1_CORPUS_SOURCE; existing F2XM1/FYL2X tools replay their
expanded data. CI generates/replays the new corpus and links both Windows
profiles. Full112 host/77 Windows-tool tests and strict lint pass. Negative
fixtures now select still-unsupported FBLD instead of newly admitted FYL2XP1.
No remote CI, physical Windows or whole-game endian qualification is claimed.

There are no FYL2XP1 sites in the backed game inventory. Coverage, closure and
all source/report bytes remain identical to the previous record:17,233 closed
functions/164 tails,670,354 admitted/139,823 rejected forms,53,593,579 complete
source bytes SHA9fd75cdd7f71882666956d14bbca3b748c2c7dec2fa2bb298da17e16d9a27b54;
body SHAf9767f9c8da77e1975c37647ff0941ff4fcce13b4769eb1f31c044d2597e7956;
report73dc32327b78014bf5c8f5ce906d0e46ceccc3106c74669c2825a57af92a6fc3.
Fresh Linux/Windows generation agrees. Verified identical game objects are
reused, while current state/math wrappers are freshly compiled and partial
links on ELF64/ELF32/i386 COFF resolve all numerical/instruction services,
without compiler binary128 references. Generated synthetic code exercises the
new operation; the unchanged game body does not. This is not full executable
linking. Packed-decimal formats, full CPU/faultcontext/SEH/ABI/engine/device/
all-worker/discovery/repair/dispatch/sourcekit/build/signed playable integration
and original full-game/platform requirements remain open. Readiness false;
private original prototype/media unchanged, with no assets/derived C/private
inventory or complete-playability/FPS claims published.

## Floating-register fault capture

The host boundary now offers `gamepkg_x86_run_with_fp` and the typed
`gamepkg_x86_run_pinned` adapter in `runtime/gamepkg_x86_pinned_context.h`.
The latter is included after the reviewed `recomp_types.h` and binds its actual
TLS XMM lane arrays plus the shared raw x87/MMX state. The old lightweight
`gamepkg_x86_run` API and 64-byte core fault record retain their contract and
can still link without the floating-register implementation.

The optional record preserves eight physical raw80 payloads, x87 status/TOP,
control/tags, FIP/FDP/opcode/selectors, all32 raw XMM lanes and the core MXCSR.
It performs no floating conversion or register-state rollback. Explicit bindings
are trusted borrowed host storage, validated for nulls, alignment, span overflow
and register/output overlap before entry. Their descriptor is copied before
execution; capture reads the current register values. Host padding is zeroed
instead of copied. This is a host record, not a guest FXSAVE image or serialized
format. Invalid inputs leave output untouched; normal completion clears it.

`tests/fp_context.rs` emits real DIV, WAIT, MOVD and EMMS instructions and links
both fault and shared x87 services. It exercises vectors0/13/14/16/19, local EBP,
all core registers, raw special/noncanonical payloads, MMX alias/EMMS behavior,
deferred x87 FIP versus fault EIP, SIMD sticky status, nested lightweight/full
boundaries, descriptor mutation, independent explicit bindings, rejection inside
a live boundary and8000 worker iterations. The pinned generic runtime header
SHAa2b2d11ebb7851355c821f4029fde2ebe444390a730795b371881e939205f6dd
and default asset-free declarations both pass. The fixture passes sanitized
Linux64/actual ELF32, big-endian MIPS32 QEMU and GCC/Clang-emulated-TLS mixed
PE32 under Wine. `tools/check_takedawn_fp_context.sh` and CI retain width/endian
and Windows-link gates. Full native-generator suites:114 host and78 Windows-tool
tests pass, with strict lint/format checks. Wine is not physical Windows proof.

Current services also freshly compile and partial-link with the unchanged
17,233-function generated game body on ELF64/ELF32/i386 COFF; verified identical
game objects are reused. This is component integration, not a full executable or
playable-package qualification. Guest context restoration, SEH/resumption,
remaining CPU modes/state, legacy engine/register ABI migration, all-worker and
device bindings, discovery/repair/dispatch, signed source/build/launch integration
and full-game/platform fidelity remain open. Original private prototype/media
are unchanged. No guest dependencies, player Python or assets are introduced.

## Fault-register restoration and retry foundation

`gamepkg_x86_restore_fault_registers` now loads a captured register file through
explicit trusted writable bindings; `gamepkg_x86_restore_pinned` binds the actual
current-worker TLS registers. The unchanged core fault ABI carries GPRs,
eager EFLAGS, direction, FS base and MXCSR; physical raw x87/MMX state and all
XMM lanes are restored without float conversion or tag reclassification.
Input remains unchanged and host padding is zeroed. Validation rejects nulls,
misalignment, span overflow, target/input/descriptor overlap, overlapping banks,
reserved MXCSR bits and nonboolean direction before any register changes.
No heap allocation, unchecked EIP/native pointer conversion or native jump occurs.

The expanded `fp_context` fixture and `fp_restore_fixture.c` overwrite all
registers between capture and restore, verify exact state across all five fault
vectors, repeat deferred x87 and new SIMD delivery after restore, and preserve
the active outer boundary through nested capture/restore. A fixture-only map
accepts the exact known generated DIV entry, rejects an interior address before
mutation, restores a corrected dividend/divisor and actually executes DIV to
obtain99/3=33. This is recovery at that known entry, not arbitrary interior
instruction resumption. Explicit FP targets receive the saved bits while
unrelated TLS FP banks remain unchanged. Rejections check the full live register
record for partial mutation, including late invalid targets. The fixture also
checks all65536 low-word MXCSR values and8192 combinations of x87 masks, PC,
RC and TOP, preserving raw signed zero, denormal/pseudodenormal, infinity,
quiet/signaling NaN and unsupported payloads. Each target runs8000 worker
iterations of capture/restore/rejection/retry cases.

Default and checksum-verified pinned header declarations pass. Host ASan/UBSan,
actual ELF32 UB traps, big-endian MIPS32 QEMU and both GCC and Clang-emulated-TLS/
GCC PE32 Wine profiles pass. Existing CI fixture gates include restoration.
All114 host and78 Windows-tool native-generator tests, strict lint and formatting
pass. Updated services freshly compile and partial-link against verified
unchanged17,233-function game objects on ELF64/ELF32/i386 COFF; all instruction
and numerical service references resolve. This is not a complete executable.

Restoration does not reconstruct C frames/local EBP, RAM, segment/device
bindings or the old lazy-flag ABI, and does not select handlers or dispatch EIP.
Guest SEH/resumption, remaining CPU modes/instructions, all-worker/engine/device
integration, native discovery/repair/dispatch, signed source/build/launch and
full game/platform fidelity remain open. No physical Windows or whole-game
big-endian proof is implied. Readiness remains false; original private prototype
and media are unchanged, with no assets/private inventory/game-derived C,
guest crates, player Python or complete-playability/FPS claim introduced.

## SSE1 MIN/MAX and backed-game closure

After floating-state capture/restoration, an inventory check found no backed
packed-BCD instructions but769 unsupported MIN/MAX forms that block actual
functions. The new selection lowerer and integer-only shared SSE service
implement MINSS/MINPS/MAXSS/MAXPS, preserving exact second-source NaN/zero bits,
scalar upper lanes and aggregate packed exception behavior. The emitter checks
opcode/prefix/mnemonic agreement, operand counts and trusted typed expressions;
unsupported prefixes/registers/widths and synthetic inputs fail explicitly.
There is no native floating arithmetic, compiler128 math or new dependency.

`tests/sse_selection.rs` compares287,232 masked hardware cases and145,920
emitted register/alias/memory forms, including FS wrapping and16-bit addressing.
It checks in-place output aliases, all30 failed memory-read positions across the
supported memory forms, packed alignment before reads, unmasked exception
suppression and preserved source/GPR/virtual flags/host MXCSR.1,952 physical
Linux signal contexts cover828 faults and1,124 normal outcomes, including old
unmasked sticky flags that must not trigger a new exception.8000 worker cases
run per hardware fixture. Defined outputs/status are compared exactly; there
is no approximation-profile allowance.

The explicit LE corpus has289,184 records of68 bytes plus12-byte header,
19,664,524 bytes, SHAd7443965a8a9ca2fcfc884a23549b48a0ac39a3bff94d47be730102296806384.
All result/status/fault records replay on Linux32 with UB traps, big-endian
MIPS32 QEMU and PE32 GCC and Clang-emulated-TLS/GCC under Wine. Both Windows
profiles additionally execute the complete masked/generated/thread oracle;
physical signal contexts are recorded on Linux. The pinned runtime header and
asset-free default declarations both pass. `tools/check_takedawn_selection.sh`
and CI retain the corpus endian/width and Windows compile/link gates. All117
host and80 Windows-tool native-generator tests, strict lint, formatting, shell
and CI YAML checks pass. No physical Windows/remote CI/full-game endian proof
is implied.

All769 actual MIN/MAX forms now emit:330 MINSS,61 MINPS,331 MAXSS,47 MAXPS.
Instruction coverage rises to671,123 admitted/139,054 rejected of810,177.
Provisional functions20,984; emission rejects3,015; edges13,285; dependency-removed
callers3,624; one rejected raw-to-BSS extent. The closed set rises by127 to17,360
functions with168 shared tails and57,713,621 closed-source bytes. Body58,147,621
bytes SHAdf5cc9b1ef1b08e00a1190a752cf9b2449bac182bcf1f2d218ee0b6d5b42b0fa;
complete58,147,647 SHA37bcf1cbbb6f7867e41cb4ceaaacebcf95cf5b7df143f9717f7515a481a277aa;
report734e2e8cfba83a13fdae99f7b8426c780c71a26020dde607f515dff30920e392.
Linux/Windows generation agrees byte-for-byte. The entire changed game body is
freshly compiled at O3 with strict warnings for ELF64/ELF32/i386 COFF, and current
services partial-link with all instruction/numerical references resolved. This
is not a full executable, reachability proof, performance result or playable
package. Remaining comparison/reciprocal/string/atomic/system instructions,
CPU/SEH/legacy ABI/engine/device/worker integration, discovery/repair/dispatch,
signed source/build/launch and full game/platform fidelity remain open. Readiness
stays false and the private prototype/media are unchanged; no assets, derived
C/private inventory, guest crates or player Python are introduced.

## Eight SSE1 comparison predicates

CMPSS/CMPPS now lower through the integer-only shared predicate service. It
supports EQ/LT/LE/UNORD/NEQ/NLT/NLE/ORD with exact mask results, signed-zero
equality and predicate-specific quiet/signaling NaN rules. Packed exception
conditions aggregate before any destination write; same-lane NaNs suppress DE.
The typed lowerer checks opcode/prefix/mnemonic/predicate agreement, accepts
canonical aliases or correctly matched explicit immediate metadata, and
rejects reserved values8..255 rather than introducing AVX semantics. No host
float, compiler128, added numerical source/dependency or guest capability is used.

`tests/sse_predicate.rs` checks1,148,928 masked hardware cases and583,680
emitted register/alias/memory cases. Coverage includes all eight predicates,
scalar upper-bit preservation, both in-place source aliases, FS wrapping and
16-bit addresses, all120 failed-read positions,16 packed misalignment gates,
quiet/SNaN/denormal priority, RC/FTZ/sticky flags, source/GPR/virtual EFLAGS and
host MXCSR preservation.7,808 physical Linux contexts contain3,128 faults and
4,680 normal outcomes; old unmasked sticky flags do not themselves fault.
Eight workers run1000 hardware comparisons each. The frontend additionally
checks all496 reserved scalar/packed immediate encodings and alias/generic
metadata equivalence/rejection. All defined result/status bits are exact.

The LE corpus has1,156,736 records of68 bytes plus12-byte header,78,658,060
bytes, SHA848782001a6709e8e3d58cc03f137ae37a297b3f1b082ef39816c9af2db9cada.
Full results/status/faults replay with ELF32 UB traps, big-endian MIPS32 QEMU
and PE32 GCC and Clang-emulated-TLS/GCC under Wine. Both Windows profiles also
execute the complete masked/generated/thread hardware fixture; physical signal
contexts are Linux-only. Default and pinned generic runtime declarations pass.
`tools/check_takedawn_predicate.sh` and CI retain width/endian/Windows-link
qualification. All121 host and83 Windows-tool generator tests pass, as do
strict lint, formatting, shell and YAML checks. No physical Windows, remote CI
or actual whole-game endian proof is claimed.

All12 backed forms now emit:11 CMPLTSS and1 CMPNEQPS. Instruction coverage is
671,135 admitted/139,042 rejected of810,177. The closed set grows by one to17,361
functions/168 shared tails with57,720,370 closed-source bytes. Provisional20,985,
emission rejects3,014, dependency edges13,285, removed callers3,624, one rejected
raw-to-BSS extent. Body58,154,395 bytes SHA19ac3f1146bf5ca8de6646fb4a2ca219a96a29fc285d1fdf8e584d273dfb1bd3;
complete58,154,421 SHA03d9b5ad7fa2569ffbdcf4ef1918017db746e67b5f9bcc7208bf4dcf9be1db29;
reportc7905d124cdc5382c030f2e54f4dca9305c62f1aa5594def125832bef23b7574.
Linux/Windows source and reports are byte-identical. Entire changed game bodies
are freshly O3/strict-warning compiled for ELF64/ELF32/i386 COFF and current
services partial-link with all instruction/numerical references resolved,
without compiler128 helpers. Host register/kernel/MMIO/dispatch integration is
still required; this is not full executable linking or a playable-package test.

Reciprocal/string/atomic/system instructions, packed-BCD formats, remaining CPU
modes/context/SEH/legacy ABI, engine/devices/workers, native discovery/repair/
dispatch, signed source/build/launch and full-game/platform fidelity remain
open. Readiness is false and private prototype/media remain unchanged. No
assets, game-derived C/private inventory, guest crates, player Python or
complete-playability/FPS claims are introduced.

## Data-stack fault commit repair

While preparing flag-image/direction integration, the older data emitter was
found to commit ESP too early on PUSH stores, POP memory-destination stores and
LEAVE reads. Fault capture would consequently record a pointer from an
unfinished instruction. Separate retained old-emission fixtures fail for each
case. The data emitter now commits the stack/base register changes after the
required memory operation succeeds. POP memory addresses use an explicitly
staged advanced stack pointer; all other address generation uses its original
register bindings. Source snapshots, FS/SIB addressing,16-bit partial registers
and normal PUSH/POP/LEAVE results are retained.

`tests/stack_fault.rs` checks16 emitted word/dword forms with source-read and
store failures, successful access counts/values, post-POP addresses, local BP,
all core fault fields, unchanged registers/flags/memory on rejected accesses,
SP high-half carries and unsigned wrapping. Its modular logical byte arena
qualifies arithmetic, not actual segment or page wrapping.288 cases per
iteration plus800 worker iterations yield230,688 cases and115,344 injected
faults per target. `tests/stack_physical_fault.c` independently observes12 actual
Linux i386 protected-page contexts through alternate signal stacks: PUSH
immediate/store and memory/source faults, POP register/source faults, LEAVE
source faults, POP absolute/destination faults and POP [ESP] post-increment
faults, each at16/32 bits. The original ESP and relevant EBP survive; observed
fault addresses distinguish advanced POP destinations. Child deadlines, no core
dumps and reaping bound the physical test.

Both default and pinned-header declarations pass. Host UB checks, actual ELF32
UB traps, big-endian MIPS32 QEMU and GCC plus Clang-emulated-TLS/GCC PE32 Wine
fixtures pass. The native i386 kernel-context test is separate from these
logical arena checks. `tools/check_takedawn_stack_fault.sh` and CI retain
width/endian/Windows-link and physical-i386 gates. All123 host and84 Windows-tool
generator tests, strict lint/formatting/shell/YAML checks pass. No physical
Windows, remote CI or whole-game endian qualification is implied.

Coverage and closure remain671,135 admitted/139,042 rejected forms,17,361 closed
functions/168 tails,20,985 provisional,3,014 emission rejects,13,285 dependency
edges,3,624 removed callers and one bad extent. Closed-source58,327,507 bytes;
body58,761,532 SHA1af24f2d0f728c10d702f8284a53e8db1caaacc9fd5bc2ba28c66cc98a342c7e;
complete58,761,558 SHA7844b4e93603a39ae6b5f0de6b9fac41ec7464b9d9e6738bd12332f62ad8b143;
reportca05ca22360de89fdf6aac9e75df5c90cff46a6e7e044312b16b337bebc4714c.
Linux and Windows generation is byte-identical. Entire changed bodies are
freshly O3/strict-warning compiled for ELF64/ELF32/i386 COFF and partial-linked
with current services, resolving all instruction/numerical references without
compiler128 helpers. Host register/kernel/MMIO/dispatch bindings remain open;
this is not a complete executable or playable-package qualification.

Flag-stack/direction instructions, reciprocal/string/atomic/system/BCD gaps,
remaining CPU modes/SS/segment/context/SEH/legacy ABI, real instruction metadata
for memory callbacks, engine/devices/workers/discovery/repair/dispatch, signed
source/build/launch and full-game/platform fidelity remain open. Readiness is
false and private prototype/media are unchanged. No new guest crates, player
Python, assets/game-derived C/private inventory or complete-playability/FPS
claims are introduced.

## Flag-transfer and direction register integration

A typed integer flags lowerer now handles LAHF/SAHF, CLC/STC/CMC, CLD/STD and
word/dword PUSHF/POPF, including reviewed operand/address-size prefix forms.
It validates opcodes, no-explicit-operand shapes and stack-width/mnemonic
agreement. The eager register API canonicalizes defined flag-image fields,
reads DF from its TLS bridge, clears RF/VM only in PUSH images, preserves
read-only VIF/VIP/VM on POP, clears RF, retains upper fields on word loads and
publishes DF consistently. PUSH/POP accesses preserve pre-instruction register
state on memory failures. No host flags are changed by production helpers.

`tests/flags_lower.rs` checks27 emitted forms. The logical fixture executes
7,997,184 image/register/transaction cases, including1,230,336 injected faults,
with800 worker iterations. It covers reserved fields, raw direction/image
consistency, stack-width carries, prefix behavior, status-bit preservation and
source/store rollback. Native status/carry/direction assembly independently
checks229,376 exact cases. Physical instruction checks use valid arithmetic/
direction host flag controls; logical raw-field tests are not CPU-mode tests.

The separate freestanding32-bit protected-mode CPL0 oracle in
`tests/flags_ring0/` links no production helper. It executes POPF/POPFD followed
by PUSHFD under QEMU's Pentium III profile, with all65536 low words, permuted
upper words, four initial AC/ID states and both sizes. It installs a private
IDT handler for262,144 TF debug events, restores a safe baseline before C and
masks unrelated hardware interrupts. Its read/execute and read/write ELF load
segments are separate; execution has a deadline and emits only flag records.
This oracle proves register data effects in that CPU model, not processor-event
delivery in GamePKG. QEMU is maintainer-only; no VM is required by players.

The LE corpus524,288x16+12=8,388,620 bytes has
SHA29650d90d1ac7147175094a6c36c3e6cf1e6165b4418d36c861f8a49c10686b4.
Full words replay exactly with host UB checks, ELF32 UB traps, big-endian MIPS32
QEMU and PE32 GCC plus Clang-emulated-TLS/GCC Wine. Linux32 and both Windows
fixtures additionally run native status/carry/direction and all generated
transaction cases; MIPS runs the logical fixture. Default and pinned generic
runtime declarations pass. `tools/check_takedawn_flags.sh` and CI add the
protected-mode corpus and width/endian/Windows-link gates. All126 host and86
Windows-tool generator tests and strict lint/format/shell/YAML checks pass.
No physical Windows, original Pentium III, remote CI or whole-game endian
qualification is claimed.

All326 backed forms emit:305 LAHF,9 SAHF,4 CLD,1 STD,3 PUSHFD,4 POPFD.
Carry-only instructions are supported but absent from this backed inventory.
Coverage is671,461 admitted/138,716 rejected of810,177. Closed functions grow
by68 to17,429 with170 tails and60,732,799 closed-source bytes; provisional21,088,
emission rejects2,911, dependency edges13,572, removed callers3,659, one bad
extent. Body61,168,524 SHA1495b8d3fa420ea78b519a116d9951905f660554efc92ed1cc224a2be8a1d320;
complete61,168,550 SHA6bdb7d1ac41a7b4d1ef127fa6d5ee87a73bd597246298e646b913cdc891841ce;
report9297f57f6dd1ebf061783e79b53d6f476164a0d7e1e6e6362a92267ffdc0c8f5.
Linux/Windows source and reports agree byte-for-byte. Entire changed bodies are
freshly O3/strict-warning compiled ELF64/ELF32/i386 COFF and current services
partial-link with all instruction/numerical references resolved and no compiler128
helpers. This is not full executable linking, reachability, performance or a
playable-package test; host register/kernel/MMIO/dispatch binding is still needed.

New operations cover eager register data effects in the fixed protected-mode
profile. Legacy lazy flags, interrupt scheduling/inhibition, debug/single-step
and other CPU modes/events, reciprocal/string/atomic/system/BCD instructions,
full context/SEH/resumption, engine/devices/workers/discovery/repair/dispatch,
signed source/build/launch and full-game/platform fidelity remain open. CLI/STI
remain rejected rather than being reduced to an incomplete interrupt scheduler.
Readiness remains false and private prototype/media are unchanged, with no
assets/derived C/private inventory, new guest crates, player Python or complete
playability/FPS claims introduced.

## Restartable byte, word and dword strings

The typed string lowerer handles MOVS/STOS/LODS/CMPS/SCAS, REP and valid
REPE/REPNE comparisons, both address sizes, both directions and FS sources.
Exact opcode/prefix and implicit operand metadata are validated; SSE2 MOVSD/
CMPSD encodings, unsupported segments, duplicate prefixes and REPNE transfers
remain rejected. The emitted call carries the faulting EIP and local EBP.

`runtime/gamepkg_x86_string.c` uses the checked RAM service. It commits each
complete element before advancing indices/count, preserving completed writes
and the most recent LODS accumulator on a later fault. Zero repeated counts
make no memory access. Address-size wrapping preserves upper register halves;
operand width remains independent. Overlapping MOVS is sequential. Offset spans
crossing the flat32 segment limit raise #GP before that operand is accessed.
All arithmetic is integer/logical-word based, independent of host byte order.

Repeated CMPS/SCAS retain pre-instruction flags on a fault, while count/indices
retain partial progress, following the legacy Intel protected-mode target
contract. Comparisons publish the last result on normal completion. The actual
AMD host's24 Linux i386 protected-page observations preserve exact count/index
progress but expose last-comparison flags in all24 cases. That separately
recorded vendor observation does not replace the documented target profile or
relax the software fault assertions. Original physical Pentium III fault-flag
qualification remains open.

An independent freestanding protected-mode kernel executes144 width/address/
repeat/FS variants in QEMU's Pentium III profile,16 scenarios each. The2,304
records contain full result registers/flags and actual changed destination
bytes; replay compares every byte of the81,920-byte arena, including unchanged
memory. Inputs cover zero/one/multiple counts, early/late termination, backward
and overlapping copies, address16 wrapping/high halves and FS linear wrapping.
The LE corpus has313,356 bytes and SHA
67c4b6eb5f080974c843be99b075cabf820f16b520e135f3e5fb63e386167896.
No production helper is linked into the CPU producer.

The generated fault fixture checks every variant at missing/read-only element
boundaries, partial data/register commits, original comparison flags, zero
counts without a RAM binding, offset-limit faults and80 worker iterations.
Repair-and-retry checks finish the remaining work without repeating completed
elements. Exact corpus and logical fault replays pass host UB checks, actual
ELF32 UB traps, big-endian MIPS32 QEMU, and PE32 GCC plus Clang-emulated-TLS/GCC
under Wine. Default and pinned runtime declarations pass. The maintainer-only
`tools/check_takedawn_strings.sh` and CI add these width/endian/link gates.
All130 host/89 Windows-tool tests and strict lint/format/shell/YAML checks pass.

All2,870 backed string sites now emit: coverage674,331 admitted/135,846 rejected
of810,177. Closed functions increase523 to17,952 with201 shared tails and
71,827,840 closed-source bytes; provisional21,851, emission rejects2,148,
dependency edges16,303, removed callers3,899, one bad extent. Body72,276,640
SHA33712b891fb043c6e74419ac1cf5605591f9c53c168b0c3f496bf0dd42846f13;
complete72,276,666 SHA39340874b8a9c8bc6ccca71724dbca2a08dab36ab2d32356b60a3edbba23bdf2;
reportbf94b6592556e2e385b988dfbd25e3feda15bd981a5045b59a44c43ccdbd3eee.
Linux and Windows source/reports are byte-identical.
Entire changed generated bodies freshly compile at O3 with strict warnings for
ELF64, ELF32 and i386 COFF. Partial links include the new string service and
resolve all instruction/numerical references without compiler128 helpers.
These are partial modules, not a complete executable or reachability proof.

The host must bind checked RAM on every worker; strings do not fall back to
unchecked pointers or generic device callbacks. REP instruction-interior event
scheduling, interrupts/debug delivery, full CPU/segment/SEH/resumption and the
legacy flag/register bridge remain open. There is no instruction metering or
artificial count cap. Reciprocal/atomic/system/BCD gaps, full engine/device/
worker/discovery/repair/dispatch/source-kit/build/signed launch and full-game/
platform fidelity also remain required. Readiness stays false; private media
and prototype are unchanged. No player Python, new guest crates, published
game-derived C/assets, physical Windows/original P3 or playability/FPS claim
is introduced by these checks.

## Exchange and atomic RAM operations

The new typed exchange lowerer handles XCHG, XADD and CMPXCHG at byte/word/dword
widths, register and RAM destinations, valid LOCK memory forms, address16/SIB/
displacement addressing and FS memory. Prefix groups, opcode, ModRM register
fields, implicit accumulator, widths and encoded memory operands are checked;
unsupported LOCK-register forms, segments, duplicate prefixes, byte mismatches
and synthetic expressions fail. Register snapshots preserve AL/AH overlap and
source/destination identity; XADD commits its destination last. All defined
arithmetic flags use unsigned intermediates. XCHG preserves flags.

`gamepkg_x86_memory_rmw` acquires the shared RAM object's lock once across the
complete read/modify/write. Every byte requires both read and write capability,
including a failed CMPXCHG, which writes its original destination back. Combined
admission reports write-fault metadata and happens before reading/storing.
Faults release the lock and preserve registers/flags and the whole destination;
register/flag publication follows successful memory work. Byte staging supports
unaligned host-independent logical little-endian words. Unprefixed operations
also use the existing serialized RAM domain; no host-native pointer atomics or
device callback is substituted. All workers must share that same RAM object;
this is not general MMIO/device or CPU bus-lock emulation.

The independent freestanding protected-mode kernel executes59 forms in QEMU's
Pentium III profile with1,024 edge/random input scenarios each:60,416 records.
They include FS,16-bit addresses, LOCK, register identity, AL/AH overlap and
short accumulator exchanges. No production helper computes expected results.
Replay checks all six recorded register/flag words and the complete256-byte
arena, using the CPU's actual destination bytes. Its2,174,988-byte LE corpus has
SHAc227f0b582b029a24f4eaaa75ce38c75c19f38c184a66dd09d9a757c5b79977b.
The logical fixture additionally checks1,253,376 result cases,504 injected
permission/boundary faults and repairs, plus three eight-worker contention runs
of80,000 updates each: fetch-add, compare-and-swap retry and an exchange lock.
The physical Linux i386 probe captures168 protected-page faults with exact
registers/defined flags/EIP/address/user-write error bits and no partial store.
CMPXCHG equal and unequal inputs both fault on read-only destinations.

Exact CPU corpus, logical fault/repair and shared-worker checks pass host UB,
actual ELF32 UB traps, big-endian MIPS32 QEMU and both PE32 GCC and
Clang-emulated-TLS/GCC profiles under Wine. Windows fixtures use native Windows
threads; no extra pthread DLL is needed. Default and pinned generic runtime
declarations pass. `tools/check_takedawn_atomics.sh` and CI add portable and
Windows link gates. QEMU/Wine remain maintainer qualification tools, not player
dependencies or physical Pentium III/Windows proof.

All47 previously unsupported backed sites now emit:17 CMPXCHG,13 XADD,
6 LOCK XADD and11 additional memory XCHG (18 XCHG total). Coverage674,378
admitted/135,799 rejected of810,177. Closed functions increase20 to17,972 with
201 shared tails and71,946,744 closed-source bytes; provisional21,878, emission
rejects2,121, dependency edges16,384, removed callers3,906, one bad extent.
Body72,396,044 SHAbf18952ec48e46a87ecee6d58d9190ca6d3e6c4ad382de426b5dcf9bd7f0b00b;
complete72,396,070 SHA4cf7a156ee42d36675bc3ee6d3974da836c6038353865904511fada8c9331a4d;
reportd4cf61218ed5380a75396e67dd4e366554d382bffbe1827f171f5533669fa2cc.
Linux and Windows source/reports agree byte-for-byte.

Additional tests pass84 successful accesses split across separate RAM regions,
6 offset-limit #GP cases before unbound RAM access, and a valid read through
the same object after each fault to prove lock release. All133 host and91
Windows-tool generator tests pass, including prior x87/MMX and runtime suites;
strict lint, Rust/C format, shell syntax and CI YAML checks pass. Old XADD and
memory-XCHG rejection fixtures now test still-unsupported CMPXCHG8B and GS
exchanges instead, retaining their negative-admission purpose.

Entire changed game bodies freshly compile at O3 with strict warnings for
ELF64/ELF32/i386 COFF. Updated RAM and current numerical/instruction services
partial-link with no unresolved instruction/numerical or compiler128 helpers.
Host register/kernel/MMIO/dispatch binding remains incomplete; this is not a
full executable, reachability, optimization-performance or playable-package test.

Other LOCK ALU/bit operations and CMPXCHG8B remain unsupported. Reciprocal,
system/CPU/segment/SEH/legacy ABI, interior REP events, engine/devices/all-worker
binding, discovery/repair/dispatch/source-kit/build/signed launch and full-game/
platform fidelity remain open. Runtime readiness stays false, and private media
and prototype are unchanged. No assets/derived C/private inventory, guest
crates/player Python, physical Windows/original P3/whole-game endian or
playability/FPS claims are introduced.

## Fixed reciprocal and reciprocal-square-root instructions

RCPSS/RCPPS/RSQRTSS/RSQRTPS use a separate typed approximation lowerer and
integer-only host service. Opcode/prefix/width/XMM metadata, duplicate prefixes
and synthetic operands are checked. Memory inputs stage before destination
publication and packed alignment gates precede reads; scalar high lanes and
all MXCSR bits persist. No numerical SIMD exception is raised, including with
unmasked status bits. Source/output aliases stage safely. Denormal inputs,
signed zeros, infinities, NaN payload/quieting, negative roots and RCP tiny
output flushing follow the explicit fixed profile.

The isolated LGPL-2.1-or-later numerical component adapts unchanged Bochs tables
from revision b0c88410501f48bbe1a966cab1c0592fb504915a. Full original source/license
and two new hashes are retained under runtime/approx; the existing52 x87 pins
remain unchanged. C uses raw integer fields and unsigned exponent arithmetic,
not host float/compiler128. Corresponding source/build/relink materials remain
required for future binary delivery; no signed player package is produced here.

An independent C++ build of original numerical function/table bodies checks
4,681,728 normal bucket-edge/midpoint/sign/exponent cases and44 special values
exactly. GMP supplies8,192 exact integer endpoint certificates, bounding every
normal mantissa bucket by the Intel relative error3/8192. RCP's specified tiny
result flushing is separate. Positive monotonic input/sqrt and exact exponent
scaling, including both root parities, extend the endpoints to the full normal
range. This is a deterministic approximation profile, not physical original
Pentium III/Xbox bit-exact proof.

Native AMD observations cover7,208,960 result words across all32,768 valid
MXCSR words, scalar/packed forms and22 input classes. MXCSR, special values and
upper lanes are exact;819,200 finite words differ from the fixed numerical
profile and are recorded separately without relaxing its assertions.20 emitted
forms exercise scalar/packed/register/alias/FS/address16 inputs, every injected
read-fault position, packed alignment suppression and512 worker iterations.
No numerical exception is synthesized and old unmasked sticky flags alone do
not trigger one.

Capstone5 independently rejects the valid F3-then67 prefix order. A narrow
reciprocal-opcode fallback decodes reordered prefixes while preserving original
bytes/addresses and enforcing the same work limits. Both opcodes with all six
FS/address-size/F3 permutations retain identical typed operands and emit.
Duplicate/unqualified groups remain rejected; other opcodes are unaffected.

The final LE corpus4,681,772x12+12=56,181,276 bytes has SHA
d4cd167e1b13b83cd731dce0bc7e060cb23b5814e7ee61ac5b31f41209e8846c.
Every result replays host UB/ELF32 UBtrap/MIPS32 big-endian QEMU and both PE32
GCC and Clang-emulated-TLS/GCC Wine profiles. Logical emission/state/fault/worker
fixtures also run on all these targets; default and pinned declarations pass.
The maintainer-only tools/check_takedawn_approx.sh and CI add these gates.
All137 host/94 Windows-tool tests and strict lint/format/shell/YAML checks pass.
C++/GMP/native instruction oracles remain absent from player dependencies.

All18 backed RSQRTSS sites now emit; the other three operations have no sites
in this backed inventory. Coverage674,396 admitted/135,781 rejected of810,177.
Closed functions increase20 to17,992 with201 shared tails and72,772,059 closed-
source bytes; provisional21,886, emission rejects2,113, dependency edges16,396,
removed callers3,894, one bad extent. Body73,221,859
SHA6ba07ffe37550a1418d832402442b46f10b3e9929003b9a17273ee2297fa5c7c;
complete73,221,885 SHA7307eeb4949c5f49b9f03a774ae540ae98bb03bf90f77771e724060175d66619;
report4cf8beb882f5e61e277a04ee26afdc81f2fcdc911b8819922fe315e313c9f8fb.
Linux/Windows output and reports are byte-identical. Entire changed bodies are
freshly O3/strict-warning compiled ELF64/ELF32/i386 COFF and partial-linked with
the new approximation component and all current instruction/numerical services
resolved without compiler128 helpers. This is not a full executable or a
reachability/performance/playability test.

Bit/system/other atomic/BCD gaps, complete CPU/SEH/legacy ABI and REP events,
engine/devices/all-worker/discovery/repair/dispatch/source-kit/build/signed
launch and full-game/platform fidelity remain open. Readiness remains false;
private media and prototype are unchanged. No game assets/derived C/private
inventory, guest crates/player Python, original physical CPU/Windows/whole-game
endian or complete playability/FPS claim is introduced.

## Reenterable native dispatcher foundation

A new DispatchProgram builder and lower::dispatch path generate functions whose
entry is an exact qualified guest instruction PC. Each has an explicit label
switch and a generated immutable address-to-block table. Host dispatch preserves
live registers and publishes EIP/local EBP before each instruction; static
internal branches remain native jumps. CALL/RET/indirect/external transfers
return to a single host loop, so guest recursion does not grow the native stack.
RET fetches the real guest return value and supports changed/interior targets;
failed stack/source accesses preserve the pre-instruction commit state.

This addresses a gap in the old C-call path, which skips RET address values.
It is a separate explicit native ABI migration path. The17,992-function count
above remains the old C-call selection. The streaming extension below now
produces whole-inventory dispatcher candidates; kernel/device/legacy bridges,
source-kit and player launch integration remain required. The old source path
is not silently reinterpreted.

The dispatcher reports real faults separately from unresolved targets and host
protocol errors. Missing targets cannot masquerade as successful C returns or
fabricated CPU faults. Full optional floating-register capture freezes binding
descriptors across callbacks, and the existing restore API plus checked PC
reentry retries an interior instruction without replaying completed work.
Nested host dispatch/site cleanup, invalid input/output overlap rejection,
flat32 #SS stack limits and trusted stop hooks have explicit contracts.

Public synthetic fixtures verify120,000 call depth with a bounded hardware stack
pointer, changed returns, direct/register/ESP-source calls, tail jumps, word
returns/cleanup, faults/repairs, current EBP sites, nested callbacks, exact raw
FP retry and256 worker runs. A separate actual i386 instruction execution agrees
on call/changed-return/tail/stack results;18 protected-page contexts verify
restart/commit behavior. The host RET fetch-error bits are recorded separately
from the Pentium III target. No real game bytes are executed by these oracles.

Host ASan/UB, ELF32 UBtrap, MIPS32 big-endian QEMU and both PE32 compiler/TLS
profiles under Wine pass with default/pinned declarations.140 host/96 Windows-
tool tests and strict lint/format/shell/YAML checks pass. The old unchanged game
bodies remain ABI-compatible with freshly compiled fault/dispatch/numerical
services in three partial object links; those are not full dispatcher/game
executables. Reproduce with tools/check_takedawn_dispatch.sh. See the runtime
README for return codes, ownership, FP capture and resumption requirements.

The production loop adds no allocation, instruction metering or artificial guest
call-depth limit. Process time/memory controls still apply. Full CPU/segment/
SEH/events/legacy ABI, engine/device/worker/discovery/repair/source/build/signed
launch and full-game/platform fidelity remain open. Readiness stays false;
private game prototype/media remain unchanged and no playability/FPS claim is
introduced.

## Streamed dispatcher source and initial runtime entry

The maintainer command
`reference-dispatch-source DISC PRIVATE-FUNCTIONS.json PRIVATE-TEMP-OUTPUT.c`
verifies the supported owned disc and executable, decodes the executable only
in memory, and stages dispatcher source in a new file inside a private child
of the operating system's actual temporary directory. Linux and native Windows
generation produce byte-identical source and reports. This command is not yet
the player source-kit/build/launch recipe.

`dispatch::write_streamed` makes two deterministic passes. The first retains
compact map records and per-function input/body hashes, validates all admitted
units and plans the complete output size before writing. The second verifies
those hashes while streaming bodies. Caller staging remains mandatory for I/O
errors or changed second-pass inputs. The existing 192 MiB diagnostic budget
admits up to 32,768 units of at most 16 KiB and 1,048,576 map records; source has
a separate 512 MiB cap. These are generation bounds, not execution metering.
The decoder/lowerer remains bounded and does not keep all bodies live at once.

x86 permits different instruction streams to start inside another encoding.
Dispatcher validation therefore compares overlapping bytes and the complete
typed instruction at every shared PC, instead of requiring disjoint instruction
boundaries or nested extents. Identical PCs have one canonical smallest-entry
owner. Conflicting bytes or typed instructions fail closed. Synthetic actual
i386 execution independently verifies both overlapping entries and both sides
of a conditional branch into another encoding. Streaming/builder equality,
window reuse past 16 KiB, late conflicts, multi-chunk budgets, writer failures,
source caps and same-size second-pass mutations have regression coverage.
The legacy C-call path retains its separate stricter layout policy.

The owned-image diagnostic emits 23,935 functions and 771,838 unique instruction
entries from 23,999 backed candidates, with 64 rejected functions and one
unbacked extent. There are 453 overlapping units, including 450 shared tails.
This is not the old direct-call dependency closure or proof of reachability.
Complete source is 282,175,852 bytes, SHA-256
`3ec1b796658c1e35069e7679b0e2eaaa201e0957bbf445651448fe788dfb29b6`;
body SHA-256 `c284ea2d88c491222ebb0732c60b65da84d2df8185ee03c6288a4c82ef41cc02`;
explicit little-endian PC/owner map SHA-256
`4cb07680c1c502d1acd581427e4ddbeddcf2ff9f1fb8a2902d8cadbe10a7310b`.
The report SHA-256 is
`26fb5f1216de11bc7bb18d9fe1afab3713b5a669c88f4a0914f5a4b07aa02598`.

A private maintainer harness splits only between functions: 62 body units and
one map unit. All generated functions freshly compile at O3 with strict warnings
for ELF64 Linux, ELF32 Linux and i386 Windows COFF; partial links resolve the
current instruction/numerical services without compiler 128-bit helpers.
Maximum compiler resident sizes are 664,456/536,532/536,472 KiB respectively.
This private chunker is not production source-kit integration. The portable
dispatcher fixtures pass host sanitizers, actual ELF32, big-endian MIPS32 QEMU,
and both PE32 compiler/TLS profiles under Wine. All 144 host/100 Windows-tool
generator tests and strict lint pass.

A private ELF64 executable loads all 17 verified sections from the owned ISO
in place, enters the real translated entry at 0x001d2807, and exits the dispatcher
at unresolved PC 0x800000ff (kernel ordinal 255, PsCreateSystemThreadEx), without
a reported guest CPU fault. Its initial context is deliberately minimal: this
is an import-discovery probe, not a complete Xbox loader or playable launch.
No kernel success stub is substituted. Real thread creation, lifetime/handles,
suspension, guest stack/TLS/FS setup and all-worker RAM binding remain required.

The runtime now also provides nonfaulting raw register capture at an import
boundary, with a pinned-context adapter and the existing restore API. It copies
pending x87/SSE conditions and raw MMX/XMM payloads without WAIT/arithmetic,
does not change live local EBP, and rejects invalid/overlapping bindings without
mutation. Alternating contexts and 8,000 worker iterations pass ELF32, MIPS32
big-endian and both PE32 compiler/TLS profiles; host UB checks also pass. This
does not create threads or capture segment/RAM bindings, native frames or SEH.

Kernel/device/legacy integration, remaining unsupported candidates, audited
discovery/repairs, source-kit/build/signing/launcher integration, actual gameplay
and full platform/fidelity qualification remain open. Readiness stays false.
Private source chunks, inventory and media remain outside the repository; no
game assets or derived source are published, no player Python/guest dependencies
are added, and no physical Windows, original-CPU or playable/FPS claim is made.

## Breakpoint traps and diagnostic worker entry

The first requested system startup entry, 0x001d1818, was already present in the
reference inventory but rejected because its final instruction is INT3. The
lowerer now admits the exact unprefixed one-byte CC encoding with no operands
or control-target metadata. Other interrupt encodings, prefixes and IP wrapping
remain explicitly unqualified. INT3 is never skipped or treated as a return:
the runtime delivers vector 3 with no error code and a saved EIP after the
instruction. This follows the breakpoint-trap contract in the
[Intel system programming manual](https://cdrdv2-public.intel.com/812386/253668-sdm-vol-3a.pdf).
IDT/privilege transitions and debugger/SEH delivery remain host integration work.

An actual i386 SIGTRAP context independently checks vector, zero error code,
next EIP, unchanged ESP/GPRs and defined arithmetic/direction flags. Together
with the prior stack cases there are now 19 physical signal-context checks.
Generated dispatch traps after one INC and resumes at the next admitted label,
executing the following INC exactly once while preserving raw FP banks. Exact
encoding/metadata, invalid prefix, operand and IP-wrap rejections are tested.
Host sanitizers, ELF32, MIPS32 big-endian QEMU and both PE32 compiler/TLS profiles
pass. All 145 host/101 Windows-tool generator tests and strict lint pass.

This admits another 21 whole functions: 23,956 total, 772,156 unique instruction
entries, 458 overlapping units including 455 shared tails. There are now 43
rejected functions and the same one unbacked extent. Linux/Windows output and
reports remain byte-identical. Complete source 282,289,701 bytes has SHA-256
`10ce29821f9da4105abae052a86b04ef3f23c0f47c5ac00835d1746350695b09`;
body 282,289,675 bytes SHA-256
`600ef5acdc1342d9bb52eb878d258af6af5ca50aa74f51ddbdf1793cb0dd6086`;
map SHA-256 `1b1a074a866dce0689438f7438b73bb4cfde230e749e0f97ccf6c637472a33e5`;
report SHA-256 `8303201be08f7528f164516997d7d0e06998ce0b7bf1e573c1a3c7059b8aa4ed`.
Every retained function body matches its earlier SHA-256. The 21 additions
(100,946 body bytes), new complete map and current runtime services freshly
compile at O3/strict warnings for ELF64/ELF32/i386 COFF, then partial-link with
the previously compiled identical bodies. All instruction/numerical references
resolve without compiler 128-bit helpers. Source-kit generation remains open.

The captured real thread request asks for a 64 KiB stack, 148 TLS bytes and an
unsuspended system routine. A private diagnostic host worker binds the same
checked RAM object, restores a separate zero/default raw register context and
uses a separate stack, FS/TIB and minimal TLS layout. It enters that real
translated system routine and reaches unresolved 0x80000115, kernel ordinal 277
(RtlEnterCriticalSection), with no reported guest CPU fault. The parent's full
captured register image is unchanged after join. The caller remains stopped
at PsCreateSystemThreadEx: no fake handle, success return or completed thread
service is provided. The diagnostic fixed RAM placements/minimal TIB are not a
production allocator, kernel object or complete Xbox initial state. Real thread
lifecycle, handles, suspension, synchronization and complete loader/runtime
bindings still require implementation and qualification. Readiness stays false;
no signed playable package or full gameplay/platform proof is claimed.

## Blocking critical-section runtime integration

The checked RAM object now owns a native metadata mutex/condition for Xbox
28-byte critical sections. Initialize/try-enter/recursive blocking enter/leave
preserve little-endian guest owner/count/depth and synchronization-event state.
Blocking releases RAM/metadata locks; another lock or arena can progress, and
there is no fixed shadow table or fabricated acquisition on failure. Faults
release native locks before reaching the outer guest exception boundary. The
runtime README documents exact statuses, ownership, waiting and current limits.
Guest wait-block/APC/IRQL/event interoperability and thread lifecycle remain open.

The owned lock's initial seven words match the nxdk ABI. Synthetic layout,
recursive/try/owner/error, permission/fault cleanup, independent arena/lock,
handoff and 16,000 contended recursive-update tests pass host sanitizers, actual
ELF32, MIPS32 big-endian QEMU and both PE32 compiler/TLS profiles under Wine.
Generated stdcall import/return/stack cleanup runs in the dispatcher matrix.
Existing RAM tests and all 145 host/101 Windows-tool generator tests pass;
strict lint/format and CI YAML parsing pass. CI includes actual ELF32 execution
and Windows mixed-compiler linkage for the complete checked-RAM fixture.

A private real-startup worker completes two critical-section import calls using
the backend and real checked guest return/cleanup, then stops at 0x001d3f2f with
no reported guest CPU fault and unchanged parent raw register image. Inspection
identifies CLI/STI in that rejected candidate as the next concrete CPU/runtime
gate. They require proper virtual interrupt state/privilege and STI shadow, and
remain unsupported. No kernel thread creation success or handle is fabricated.
All 23,956 game block bytes and source hashes above are unchanged; current
runtime wrappers freshly compile and partial-link on Linux64/Linux32/Windows32.
Private probe executable SHA-256 is
`1dc1b886b579eb5f89171c07d83f2236cda1fbe0d2e9b2a0bb0960f2dcfafb33`.
Readiness remains false: production thread/loader/source-kit/launch, remaining
43 candidates, full interrupt/device/kernel integration and playable signed
GamePKG/platform/fidelity verification remain required. Media/library and
private prototype are unchanged; no assets or derived source are published.

## Verified interrupt-state foundation; scheduling integration pending

A new standalone runtime component implements CLI/STI flag/privilege transitions
and STI maskable-IRQ inhibition as explicit host-owned CPU state. PE/CPL/PVI/VME,
IF/VIF/VIP and a one-bit retirement shadow are preserved independently of host
CPU flags. GP and invalid-host-state paths leave the image unchanged. A second
STI does not extend inhibition; another retired instruction or delivered event
ends it. The runtime README documents the API and required future CPU bindings.

The3584-case portable table fixture, a production-independent privileged kernel
with2048 flag observations and3 pending-PIT/PIC shadow sequences under the
Pentium3 QEMU model, and256 actual i386 CPL3 GP contexts provide distinct
evidence. The canonical LE65,644-byte machine corpus has SHA-256
`0e0f05dd08eaa3f9a73102721a6e13c912e5b8de8cd29896c54c435add012593`;
all records replay host sanitizers/ELF32/MIPS32 big-endian/both PE32 compiler
profiles under Wine.147 host/102 Windows-tool generator tests, strict lint,
format/shell/YAML checks pass, and CI adds portability/Windows linkage gates.
This is not original physical P3/Xbox, physical Windows, full V86/PVI hardware
or full runtime/game verification.

The generator still rejects CLI/STI, with a dedicated regression guarding
against premature admission. Correct retirement/event handling in the native
dispatcher, shared virtual-CPU ownership/preemption, disabled-IRQ regions,
blocking kernel boundaries, device interrupt posting/acceptance and full saved
CPU state must be connected before that changes. REP interior and SS shadow,
IDT/privilege handler frames, NMI/SMI and scheduler fidelity remain open. Existing
23,956 emitted functions,43 rejected candidates, source hashes and private
startup stop at0x001d3f2f are unchanged. There is no instruction metering or
new guest dependency/player Python, and readiness remains false. Signed playable
package, production thread/source-kit/build/launch and full game/platform parity
remain required.

## Shared CPU reservations for host workers

The runtime now has an explicit shared virtual-CPU ownership domain and opaque
thread contexts carrying interrupt flags/controls/shadow. FIFO acquisition,
eligible handoff, reservation-preserving suspension and host blocking release
are distinct operations. IF0 and STI inhibition prevent another guest worker
from taking the CPU; no native mutex stays locked across guest execution.
Wrong-worker/duplicate entry, live-owner destruction and aliasing snapshots
reject safely. Context references retain domain lifetime. The runtime README
documents the contracts and remaining interrupt integration.

The ownership fixture verifies IF0/STI exclusion, suspend/resume, FIFO handoff,
owner checks, GP/restore, lifetime and16,000 eight-worker serialized entries.
It passes host sanitizers, ELF32, MIPS32 big-endian and both PE32 compiler profiles
under Wine; CI and tools/check_takedawn_cpu.sh reproduce the gates. Full standard
suites pass148 host/102 Windows-tool tests; strict lint/format/shell/YAML pass.

The private startup probe uses CPU ownership around parent/worker and the two
checked critical-section host calls, preserving parent raw and CPU snapshots.
It still stops at0x001d3f2f with no reported guest CPU fault. Executable SHA-256
`7b21f5d5a97f44432b8b39bb42989193aa4dffae4268db884e2eb1b6f3b697e6`.
This is host-boundary integration only: instruction retirement and IRQ delivery
are explicitly false in the report. CLI/STI remain rejected pending full
dispatcher/event/REP integration, IRQ acceptance/handler frames and production
thread/CPU bindings. Source coverage, hashes and readiness=false remain unchanged;
no signed playable launch or full game/platform fidelity is claimed.

## Controlled dispatcher now retires real startup instructions

An explicit controlled builder and streamed source profile now place hooks
before every admitted instruction and after successful retirement, including
branches/CALL/RET. Faults notify the frozen controller after raw capture, and
boundary events return before executing the next instruction. A shared-CPU
adapter validates native ownership/raw flags, performs eligible handoff and
updates the CPU image; it retains ownership for host import/fault handling.
The runtime README covers profile contracts, statuses and tested edge cases.

The new maintainer command is
`reference-controlled-dispatch-source DISC PRIVATE-FUNCTIONS.json PRIVATE-TEMP-OUTPUT.c`.
It uses the existing private source guard, disc integrity and generation bounds.
Linux/Windows output is byte-identical:23,956 functions,772,156 entries,43 rejects
and one unbacked extent. Complete368,019,437-byte source SHA-256:
`590af0d41538538b48ba93e7671ec682c113cc8377bc89cb0275a029b3d38225`;
body SHA-256 `717bbe6eedf0cfd7b24bfed3552d84ba8f2b22b294d63994833dabfdb240c43c`;
map SHA-256 `1b1a074a866dce0689438f7438b73bb4cfde230e749e0f97ccf6c637472a33e5`;
report SHA-256 `3b0071406b4af444178b25d5f3d79f34cbe8a013c509b52a327b7cca9ca674f7`.
The ordinary source/report were also regenerated and match the previous bytes.

All controlled functions freshly compile O3/strict warnings in82 units per
target and partial-link with current instruction/numerical/CPU services on
Linux64/Linux32/Windows32. The portable dispatcher matrix, native instruction/
signal oracles, full148 host/102 Windows-tool tests and strict lint pass.
Synthetic coverage checks retirement order, fault repair, boundary stop/resume,
callback freezing/protocol errors, EBP, raw x87/MMX/XMM preservation and CPU
ownership. The actual private Linux64 startup probe now uses the controlled
map, completes two critical-section imports and preserves parent raw/CPU
snapshots before stopping at0x001d3f2f with no reported CPU fault. Executable SHA:
`f321f84389860259e37005783df9b9c20cb48bc753d50b39fcbcd11e99e35e14`.

Instruction-boundary integration is now true in that probe. REP interior
integration and IRQ delivery remain explicitly false. CLI/STI are still
rejected by both source profiles until those are connected and verified.
Device event posting/acceptance and handler frames, complete CPU/kernel thread
contexts, source-kit/cache profile selection, production build/signing/launcher
and full gameplay/platform parity remain open. This is not a playable release;
readiness stays false and private media/assets/derived source remain private.

## REP checkpoints in the controlled runtime

The controlled profile now calls a string helper that can pause after a completed
REP element when another element remains and qualifies. Count, indices, writes,
and comparison flags describe that exact prefix. Continuing restores this
invocation's entry flags before the next fault-capable element; resuming a
stopped invocation establishes the checkpoint flags as its new entry image.
A partial stop neither retires the macro instruction nor expires an STI shadow.
The CPU adapter publishes checkpoint flags without changing inhibition state.
Ordinary emitted source and its report remain byte-identical to the previous
qualified profile. No instruction metering is introduced.

`tools/check_takedawn_string_checkpoints.sh` generates an independent protected-
mode Pentium III-model TF corpus and a separate whole-instruction corpus, then
replays and resumes 17,556 prefix states, including 11,812 interior stops and
5,744 early completions. Exact registers and the full 81,920-byte RAM image agree
on host sanitizers, actual ELF32, big-endian MIPS32 under QEMU, and both PE32
GCC/Clang TLS profiles under Wine. The existing string/fault, CPU ownership, and
dispatcher matrices also pass. CI adds portable execution and Windows link gates.
A separate actual i386 Linux timer-signal fixture checks 42 asynchronous interior
states, complete shared RAM, comparison flags, and saved RF=1; absence of an
interior observation is a failure. Its optional maintainer
`GAMEPKG_PHYSICAL_INCLUDE` supplies missing platform headers only.

The TF corpus normalizes TF/RF through its handler and PUSHFD; it is not an
interrupt-frame oracle. A private P3-model PIT probe agrees on partial indices
and comparison flags but omits RF in all 42 observed frames. Intel SDM Volume 3B,
section 20.3.1.1, specifies RF=1 for an interrupt during an unfinished REP
instruction; the physical Linux contexts agree. This emulator difference is
recorded rather than treating QEMU as original P3 hardware proof. Future IRQ
entry must distinguish an instruction boundary from a REP interior checkpoint
and construct the appropriate saved frame. See the
[Intel system manual](https://cdrdv2.intel.com/v1/dl/getContent/671427).

Regenerated controlled source changes exactly 2,866 string sites in 1,000
functions; every other source line is unchanged. It retains 23,956 functions,
772,156 map entries, 458 overlaps, 455 shared tails, 43 rejected functions, and
one unbacked extent. Complete source is 368,268,779 bytes, SHA-256
`9027e909e165f00a23eb02d1eecbb29bea21b2e066bb053d981aa453682bd57e`;
body SHA-256 `7ed1d380d3d2107915816163629b8b9c333830e0a7ceead1f03fcc3dac7d8d93`;
map SHA-256 `1b1a074a866dce0689438f7438b73bb4cfde230e749e0f97ccf6c637472a33e5`;
report SHA-256 `6e497d363b050a9b69ce3ad41ab50417818c6eaeb0f88b4ea7ef3e2d197f2ef9`.
Linux and Windows tool source/report outputs agree byte for byte after report
line-ending normalization. All functions freshly compile at O3 with strict
warnings in 82 units per target on Linux64, Linux32, and Windows32, with all
instruction/numerical/CPU services resolved by relocatable linking. Maximum
compiler RSS is 668,588 / 540,292 / 540,340 KiB respectively. These are compilation
and partial-link checks, not full platform executables or performance results.

The private Linux64 startup executable SHA-256 is
`e59354914c476ca60f08efcac98a7e201e9c8fa9950033b8cc423dd1272d7bba`.
The real parent reaches unresolved PsCreateSystemThreadEx; its captured worker
request runs a diagnostic thread through two real critical-section imports and
stops at 0x001d3f2f while parent raw/CPU snapshots remain unchanged. Its original
zero-string/zero-REP counters were later found ineffective and are withdrawn.
The corrected extended-path probe in the CLI/STI section observes four strings
and35 REP checkpoints; see the diagnostic counter correction below. No production thread handle, success result, or continued execution
is fabricated. IRQ delivery remains false and CLI/STI remain rejected. Full
thread/kernel/device integration, IRQ/IRET/debug/RF and SS inhibition, remaining
instructions, production GamePKG build/signing/launch, and full gameplay/platform
fidelity still need completion. Readiness remains false.

An additional all-144-form generator regression compares controlled builder and
streamed output byte for byte, including each exact helper argument and early
return. Sixteen-form batches keep the original 16 MiB fixture budget, and each
batch returns all reservations to its baseline. Final targeted tests pass five
host/four Windows string cases and eight host/seven Windows dispatcher cases;
strict lint and formatting pass. The preceding full suites passed 148 host and
102 Windows tests before this extra case. The final published maintainer script
also passes all five replay profiles and all 42 physical signal cases locally;
remote CI execution and physical Windows remain unclaimed.

## Boundary-scoped event capture and IRQ flag planning

The controlled profile now uses `gamepkg_x86_dispatch_rep_checkpoint` for REP
interiors; ordinary instruction boundaries keep their existing hook. A trusted
boundary callback can query the kind and request a complete raw GPR/x87/MMX/XMM
snapshot from the FP bindings frozen at run entry. These queries reject calls
outside the callback, including retirement and nested ordinary dispatch.
Nested dispatch restores the outer scope. Snapshot output cannot alias the
map, dispatcher result, original/frozen controller or FP descriptor, FP banks,
or active frame. Reentrant boundary/retirement calls and guest faults raised
by a host boundary callback are protocol failures, not admitted instructions.
Tests also mutate the original FP descriptor and verify the frozen bindings
still produce correct snapshots without a WAIT or pending-exception delivery.

`gamepkg_x86_dispatch_run_cpu_events` optionally polls a frozen trusted host
source at eligible instruction/REP boundaries before worker handoff. IF=0 and
an STI shadow suppress polling. A pending request stops with reason 4 and
retains CPU ownership, partial registers, and the still-pending request. It
neither acknowledges an IRQ nor enters a guest handler. The default CPU API
remains available without a source. Synthetic execution verifies interior
comparison flags, raw pending x87/MMX/XMM state, correct resume, no invented
fault, whole-REP shadow exclusion, frozen descriptors, and rejected aliases.

`gamepkg_x86_interrupt_plan_irq` separately plans saved and handler flags for
protected flat32 CPL0 interrupt/trap gates. It preserves RF between instructions,
sets saved RF for an unfinished REP, clears live handler TF/NT/RF/VM, and clears
IF only for an interrupt gate. Inhibited or invalid/unsupported inputs leave
both source and output unchanged. It does not validate an IDT descriptor,
accept/acknowledge a device, write a stack frame, change privilege, or implement
IRET. Other modes, NMI/SMI, and full interrupt delivery remain separate work.

The new asset-free `tools/check_takedawn_irq_flags.sh` creates an independent
P3-model protected-mode oracle: IRETD establishes RF/TF/NT/IOPL and all tested
status combinations, then a pending one-shot PIT interrupt captures both the
saved stack flags and the live interrupt/trap-gate handler flags before the
handler alters them. All 8,192 frames replay exactly on host sanitizers, actual
ELF32, MIPS32 big-endian under QEMU, and both PE32 GCC/Clang profiles under Wine.
The consumer also checks 8,192 interior saved-RF plans and 49,152 inhibited cases;
interior RF is separately supported by the prior 42 actual Linux signal cases
and the Intel specification, not the QEMU REP frame discrepancy. The canonical
196,620-byte LE corpus SHA-256 is
`ea6bd6453835a77a0c66eeeefb1d8b71bb0659ef4275b0ee97e0616244a7163c`.
CI adds portable execution and Windows link gates. Both default and pinned
runtime-declaration dispatcher matrices, existing interrupt/CPU matrices,
150 host generator tests, 103 Windows generator tests, strict lint, and
formatting pass locally. Wine and the P3 model do not qualify physical Windows
or an original P3 processor.

Controlled source changes exactly 2,866 callback identifiers in 1,000 functions;
every other source line and the ordinary source/report remain identical.
Current source is 368,285,975 bytes, SHA-256
`4587ce36c491c22b51732e01ea3d3d056569c741e63448a8211c33ff4c8794be`;
body SHA-256 `20823dc486f1b3552be4cf9c52012eee253b407eac54493225f3d732c6df0f5e`;
map remains `1b1a074a866dce0689438f7438b73bb4cfde230e749e0f97ccf6c637472a33e5`;
report SHA-256 `725aa51a43c19fb1e54de3d600e68aa8925f09f96c66eb60ef4ed0a3a6eb4a7b`.
Linux and Windows tool source/reports agree after report line-ending
normalization. Counts remain 23,956 functions, 772,156 entries, 458 overlaps,
455 shared tails, 43 rejected functions, and one unbacked extent. All functions
freshly compile at O3/strict warnings in 82 units per Linux64/Linux32/Windows32
target; partial links resolve all instruction/numerical/CPU services. Maximum
compiler RSS is 668,952 / 540,384 / 540,092 KiB. These are partial-link and
compilation results, not complete platform executables or FPS measurements.
Old controlled objects must be rebuilt before relying on the new boundary-kind
semantics; instruction hooks cannot identify legacy REP callbacks correctly.

The private startup executable SHA-256 is
`32a5689aa3488e3f25cc0307916f76c331dd729d1ca5bbb9c33d7f4c5a364522`.
A deliberately pending **host diagnostic request** stops at the real entry
0x001d2807 before any game instruction, captures an identical raw snapshot and
CPU image, retains ownership/request state, and plans flags 0x202 saved/0x002
handler without acknowledging a device or entering a handler. Resolving that
diagnostic request and resuming reaches the same PsCreateSystemThreadEx import;
the captured diagnostic worker completes two real critical-section imports and
stops at 0x001d3f2f with parent raw/CPU state preserved. The original zero-REP counters are withdrawn; the corrected
probe described below observes four string entries and35 REP checkpoints. A private owned-image audit identifies five CLI and
three STI sites across five rejected functions; it does not admit them or
publish their instruction data. CLI/STI remain rejected. Actual device request
posting/acknowledgement, gate/stack/handler/IRET integration, production thread
services, remaining instructions, cache/build/signing/launch, and full gameplay
and platform fidelity are unfinished. Readiness remains false.

## Controlled CLI/STI now use the shared virtual CPU

This supersedes the earlier controlled-profile rejection of CLI/STI. Ordinary
instruction lowering and ordinary dispatch still reject them. The controlled
emitter accepts only exact, unprefixed FA/FB bytes with qualified operand/control
metadata and emits a direct virtual-runtime call. It emits no native privileged
instruction. Forged operands/targets/references/text and prefixed encodings fail.
Controlled builder and streamed output agree for both instructions.

The frozen trusted controller has an optional virtual-CPU instruction provider.
A missing provider produces a host protocol stop before changing guest state.
The CPU adapter routes CLI/STI through the qualified PE/CPL/IOPL/PVI/VME/VIP
state machine. Success commits only the virtual flags and inhibition image;
GP preserves the faulting flags/banks and reports vector13 at the actual site.
Normal retirement would erase a newly established STI shadow, so completed
CLI/STI use explicit retirement events2/3 instead. Detected faults remain event1.
The provider cannot yield, fault, or query a partially committed boundary; its
reentrant use is a protocol failure. The controller ABI has changed and all host
callers/runtime services must be recompiled; old experimental controller
objects cannot be mixed with this revision.

Generated execution verifies requests pending before CLI, CLI critical regions,
STI followed by CLI, STI, REP, or a divide fault, protected32 VIF/VIP behavior and
GP, unrelated status/DF preservation, missing/reentrant/error providers, frozen
callbacks, failed retirement after a completed STI, and raw pending x87/MMX/XMM
preservation without WAIT. A real second native worker queues while translated
CLI holds the CPU; it cannot acquire the reservation until STI and its following
INC complete. Eligible event polling happens before that handoff. Existing
P3-model pending-PIT sequences and actual i386 privilege-fault evidence remain
part of the interrupt gate. Default and pinned declaration dispatcher matrices,
CPU/interrupt/IRQ-flag matrices, full151 host/104 Windows generator tests, strict
lint and formatting pass locally. Native object disassembly contains no CLI/STI
instructions. Wine does not qualify physical Windows, and these results do not
establish guest IRQ handler delivery or full platform fidelity.

Regeneration admits two formerly rejected functions, retaining 23,956 previous
function bodies byte for byte and adding 163 instruction entries. Current totals
are 23,958 functions, 772,319 entries, 458 overlaps, 455 shared tails, 41 rejected
functions, and one unbacked extent. Source is 368,364,726 bytes, SHA-256
`dca04bbc610599c2ee66b79d078c480f13f4fa65a32da6ce817e35df1bde927c`;
body SHA-256 `3a08c4d7c4a87b6b5723bf9ced821002ee6b360d6c95d285785c0770ab58a38c`;
map SHA-256 `64a6b0ecdb0e358a58428f953ac6b097f0164f3fd4b059c0f148ca627d1bdf9f`;
report SHA-256 `c53ba51fcafa798895749dd87e46912ddad6efdfeaede1aae1eb4dcefafdceee`.
Linux/Windows source and normalized reports agree. Ordinary source/report remain
byte-identical to their previous qualified output.

A private incremental build verifies every old function hash and each of the81
baseline source chunks, records hashes for every reused native object, and
reuses only those unchanged bodies and their unchanged used ABI. Generated
bodies do not instantiate the changed controller descriptor; the instruction
map layout and existing helper signatures are unchanged. The two new functions
and complete replacement map freshly compile at O3/strict warnings, and every
runtime service/host caller is rebuilt. All23,958 definitions partial-link on
Linux64/Linux32/Windows32 with numerical/instruction/CPU references resolved.
Each target reuses81 body objects and compiles2 fresh game units; maximum fresh
compiler RSS is668,412/540,664/540,348KiB. This is verified object reuse and
partial-link evidence, not an all-function fresh compile or an FPS claim.

The new private startup executable SHA-256 is
`5a00782a7391d923d18f6b01200d4ba01c553f3702d4090998700b7bf6a82091`.
Real-entry pause/resume still preserves the raw/CPU snapshot. The parent still
stops at PsCreateSystemThreadEx, and its diagnostic worker completes two real
critical-section imports. The worker now enters the newly admitted0x001d3f2f
function and stops on an actual vector14 read fault at0x001d3f34/address0x80000142.
Parent raw/CPU state remains preserved. Instrumentation explicitly observes
zero CLI/STI attempts, but four string entries and35 REP interior checkpoints
before that fault. Admission unblocks the function; its earlier data import
must be bound before live startup exercises CLI/STI. Diagnostic cleanup also refuses to clear IF/STI inhibition merely to
release a stopped worker; such a stop retains the CPU until process exit.

Ordinal322 is the `XboxHardwareInfo` data export, not a callable function, as
specified by [nxdk exports](https://github.com/XboxDev/nxdk/blob/master/lib/xboxkrnl/xboxkrnl.exe.def).
Its ABI is an eight-byte structure containing32-bit flags, GPU/MCP revision
bytes and two reserved bytes; the devkit-kernel flag is0x02 in the
[nxdk header](https://github.com/XboxDev/nxdk/blob/master/lib/xboxkrnl/xboxkrnl.h).
A checked, explicit virtual-hardware data profile and correct import binding are
the next loader requirement; the fault is not ignored and no success is faked.
Production thread/TLS/stack lifetime, other kernel/data/device imports, actual
IRQ posting/acknowledgement/handler frames/IRET, remaining instructions and
build/cache/signing/launcher/full-game platform parity remain incomplete.
Readiness is still false. No ROM/assets/game-derived source or private inventory
is published, no real library is changed, and no guest crates, player Python,
instruction metering or native host interrupt manipulation is introduced.

### Correction to earlier startup counter reports

The zero-string/zero-REP reports in the preceding two milestones were wrong.
Their diagnostic linker wrappers were applied after a relocatable link had
already resolved the calls, so those wrappers did not intercept execution.
Those counts are withdrawn. The corrected private probe compiles the original
CPU/string service definitions under distinct private names and supplies the
standard symbols through its observation wrappers before the final link.
Unresolved-symbol and disassembly checks prove the call routing; a separate
host test CPU calibrates one CLI and one STI count, then resets those counters
before actual game entry. The corrected run observes four real string entries
and35 REP checkpoints, plus zero CLI/STI attempts before the hardware-data fault.
The nominal runtime/module is unchanged by this private instrumentation. The
earlier real-entry snapshot and fault/state evidence remain valid; they did not
depend on the ineffective counters. Current probe hash/results above supersede
the old counter claims.

### Validated static kernel data and import transactions

`Image::kernel_imports` validates the complete original import table through a
raw-backed zero terminator before returning an allocation-free iterator. Each
word is read byte by byte through checked virtual mappings, including words
that cross adjacent sections with discontiguous file offsets. BSS, gaps,
missing ordinal flags and zero ordinals cannot silently become valid imports.
Unknown flagged ordinals remain unresolved records.

`kernel_data` describes the reviewed eight-byte `XboxHardwareInfo` (ordinal322)
and `XboxKrnlVersion` (ordinal324) DATA exports with explicit little-endian
encodings. Values are supplied by the host; no physical hardware or complete
kernel implementation is inferred. `bind_platform_data` preflights all source
entries, both target spans and every selected loaded slot before writing any
slot. Duplicate imports are supported. Changed/rebound slots, null or wrapping
addresses, overlapping profiles, title-image overlaps and truncated arenas
fail without partial changes. Other imports remain untouched. The caller must
register and retain separate checked read-only storage before guest entry;
rewriting an import slot alone does not create a memory capability.

Eleven layout/loader tests and the new ABI fixture pass. The fixture checks204
paired hardware/version profiles, exact eight-byte field layouts, byte order
and408 rejected write attempts without changing either profile. It passes
host ASan/UB, actual Linux32 UB checks, MIPS32 big-endian under QEMU, and GCC and
mixed Clang/GCC Windows32 under Wine. These are component results, not physical
Windows or full-game big-endian qualification. CI adds portable and Windows
link gates. Complete generator suites pass158 host and110 Windows tests with
zero failures or ignored tests; strict lint and formatting pass.

The private owned-disc startup probe now binds both real slots to separate
read-only profiles at0x80020000/0x80020010. The explicit virtual hardware is
flags0/GPU0xa1/MCP0xb1; the declared kernel compatibility target is1.0.5838.1,
not a claim that every service of that kernel is implemented. Both writes are
proven to fault before game entry. Startup completes the same two real critical
imports, then stops with a real vector14 read fault at0x001d3ed0/address0x8001003c.
Parent raw/CPU state remains preserved; the correctly routed counters observe
four string entries,35 REP interior checkpoints and zero CLI/STI attempts.
Real-entry diagnostic pause/resume still preserves the raw/CPU snapshot.

The fault is a separate kernel-image header access. A private instruction audit
shows section metadata and an INIT-section address being used to construct a
descriptor. The pinned older backend labels this cache initialization and
supplies a zero-section fake header; that comment alone does not establish the
full behavior, and that bypass is not adopted here. A consistent kernel module
view and the required initialization behavior remain open. Regenerating both
source profiles and reports after the parser/loader changes produces exactly
the previously qualified bytes and hashes. This milestone supplies static data
binding, not production thread lifetime, IRQ delivery, complete kernel services
or a playable signed package. All broader readiness requirements remain open.

### Packed-decimal x87 load/store

FBLD and FBSTP now lower through the shared raw x87 bank. FBLD decodes all18
packed digits exactly, retains signed zero and ignores the unused sign-byte
bits. Invalid A–F digits are not treated as a new invalid-operand exception;
Intel defines their numerical result as undefined. FBSTP uses the existing
integer rounding machinery for all four rounding modes, enforces the18-digit
range after rounding and writes the architectural packed-BCD indefinite on a
masked invalid operation. Invalid conversion supersedes precision, unmasked
invalid suppresses both store and pop, and deferred precision delivery follows
the existing store contract. No host floating-point cast or native128 type is
used. Existing helper signatures remain unchanged; load kind14/store kind7
extend the trusted service protocol. Runtime objects must be rebuilt.

Both instructions require the exact opcode, ten-byte memory operand and
reviewed addressing/segment forms. FBLD checks the complete segment span before
its first staged read; an old pending x87 exception wins first. Failed later
reads leave the complete x87 state untouched. FBSTP retains whole-span
permission admission before conversion and transactional store/pop behavior.
Rejection tests cover altered widths, segments, expressions, mnemonics and
prefixes. Older tests that used FBLD as their unsupported-instruction specimen
now use FISTTP, which remains outside the qualified CPU profile.

A synthetic native-x87 oracle compares344,064 cases across six generated
addressing forms and direct runtime calls. It checks all eight raw registers,
the complete tag word captured independently with FNSTENV, TOP/control,
defined status bits, instruction/data metadata, signed zero, all rounding and
precision-control combinations, all exception masks, stack faults, NaNs,
unsupported inputs, subnormals and the18-digit rounding boundary. Additional
checks cover pending delivery, segment overflow before reads, later staged
read faults and memory-permission priority without partial state or RAM writes.
This is current native-x87 evidence, not original Pentium III hardware proof.

The explicit LE `GPKBCD02` native-reference corpus is32,686,088 bytes, SHA-256
`1ea64305bdbc73ba7f329b5696c74da29592d1b2c9585276e41c9ede5f1ed412`.
Linux32 and GCC/mixed Clang-GCC Windows32 native oracles reproduce it byte for
byte; MIPS32 big-endian/QEMU replays every case against it. Undefined condition
codes are excluded from the comparison. `tools/check_takedawn_x87_bcd.sh`
provides these gates and CI portability/Windows-link modes. The first expanded
tag-word replay exposed overlapping offsets in the new fixture writer; its
encoding was corrected before the final five-profile pass. That was a test
corpus bug, not a relaxed runtime assertion. Full generator suites pass161 host
and112 Windows tests; strict lint/formatting pass.

Ordinary/controlled game source and reports remain byte-identical: these
instructions do not admit another function in the current owned inventory.
Fresh Windows32 generation matches Linux source and normalized reports for
both profiles.
All23,958 definitions partial-link with fresh runtime services on Linux64,
Linux32 and Windows32. The qualified81 unchanged body objects per target are
reused after exact source comparison; two fresh game units and every current
runtime object are compiled, and reused/fresh object hashes are recorded
privately. This is verified reuse, not an all-function fresh compilation.
The new private startup probe SHA-256 is
`8c098404f2c5221874330948d39ec77115571f3786007c5d0e16ae868f21bc0a`.
It preserves the prior entry/parent raw and CPU state, completes two critical
imports and reaches the same kernel-header read fault at0x001d3ed0/address
0x8001003c, with four strings/35 REP checkpoints and zero CLI/STI attempts.
This run does not establish live-game use of the newly implemented BCD
instructions. Kernel image/INIT descriptor behavior, production threads,
IRQ/device integration and signed playable-package/full-game/platform
qualification remain open. Readiness is still false.

### Virtual descriptor-table state and the next initialization helper

A further private owned-image audit identifies the INIT helper's observable
operations more precisely. It constructs a base-zero code-segment descriptor
using the kernel INIT-section boundary, reads GDTR with SGDT, exchanges the
existing code-descriptor words under CLI/STI, reloads the segment with a far
jump, and returns the previous descriptor. The earlier prototype's cache-init
comment does not establish these semantics. The current minimal diagnostic
worker also leaves part of the processor-region setup incomplete. Neither a
fake empty kernel header nor a nonzero field invented to skip this path is
introduced. Kernel metadata, complete processor setup and the full descriptor
transition still need implementing.

The first required CPU component is now present. `GamePkgX86Tables` holds
explicit logical32-bit GDTR/IDTR bases and16-bit limits on the shared processor
domain. `gamepkg_x86_cpu_init_tables` accepts a copied, one-time configuration
before any thread exists. A configured zero register is valid and distinct
from missing configuration. Repeated/late setup, invalid limits, pointer
alignment/overlap errors and configuration after the last created thread is
destroyed are rejected. Snapshot access requires the current native worker to
own the CPU; suspended/inactive/other-owner accesses leave outputs untouched.

`gamepkg_x86_cpu_store_table` implements the six-byte memory image for the
legacy protected32 SGDT/SIDT service. The32-bit operand form retains all base
bits; the legacy16-bit form retains24 and clears the sixth byte, matching the
[older Intel instruction reference](https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-vol-2b-manual.pdf)
and the independent Pentium-III-profile QEMU fixture. Later processor manuals
describe different16-bit behavior; no modern-host or physical-Pentium-III
identity is inferred. The service uses the existing checked byte-image
transaction, retaining full-span admission and no partial writes. It does not
broaden the scalar memory helper's supported widths. Invalid op/size, missing
configuration, wrong ownership and unqualified real/v8086 mode return a host
error before RAM access. It preserves the CPU table/interrupt images and does
not retire an instruction or create a RAM capability. No native host table
instruction or host table address is used; the new runtime object's disassembly
contains no SGDT/SIDT/LGDT/LIDT opcode.

The independent ring0 fixture temporarily installs explicit GDTR/IDTR values
inside QEMU only, stores them using both operand sizes, restores the original
tables, and records all bytes and adjacent sentinels. Its2,448 records cover
zero/all-one/single-bit bases and limits. The58,764-byte GPKTBL01 corpus SHA-256
is `b5a484ba6a28c49037500238cf18981eff1844c001db4188497756f5a7e480fb`.
Replay passes host ASan/UB/leak checks, actual Linux32 UB checks, MIPS32
big-endian/QEMU, and GCC/mixed Clang-GCC Windows32/Wine. The fixture also checks
read-only/truncated memory faults without partial writes, invalid/missing/late
configuration, exact register selection, inactive ownership, unchanged CPU
images, and1,024 leases across eight real workers. Four explicit byte vectors
run in the standard Linux Rust suite. The existing CPU ownership matrix still
passes16,000 eight-worker entries per target. CI adds portable and Windows-link
gates. Complete generator suites pass162host/112Windows with zero failures or
ignored tests, and strict lint/format checks pass. LeakSanitizer needed execution
outside the managed sandbox because its process inspection is restricted there;
leak checking remained enabled in the final qualification.

At that component milestone, SGDT/SIDT and the helper's far jump were rejected.
The following integration adds controlled table stores. Far-jump descriptor
validation, code-segment cache updates, instruction-fetch bounds and complete
segment fault/context handling remain required. No generated
game function is newly admitted by this component. The prior real startup
still stops at the kernel-header read; this milestone does not claim a new
successful boot, native window, signed package or gameplay. Production threads,
IRQ/device integration, complete kernel/legacy ABI and full-game/platform
qualification remain open.

### Controlled SGDT/SIDT execution

Generated controlled blocks now admit exact memory SGDT/SIDT encodings with
six-byte operands, qualified FS/operand-size/address-size prefixes, and complete
ModRM/SIB/displacement lengths. Duplicate or unsupported prefixes, register
forms, wrong opcode fields/widths, synthetic expressions, mismatched segment
metadata and transfer/immediate metadata are rejected. Ordinary lowering still
rejects both instructions. Builder and streaming output agree for all32 tested
forms, including16-bit addressing, FS, SIB, EBP, ESP+6 and absolute addresses;
fresh Windows and Linux fixture generation agrees byte for byte.

A new `GamePkgX86DispatchTables` descriptor is frozen for each controlled run.
`run_controlled_tables` scopes its pure virtual-table provider independently of
the existing four-field controller ABI. CPU dispatch supplies the active
owner's explicit shared table image, rejecting missing configuration or an
unqualified real/v8086 profile before RAM access. Callback reentry/fault/error,
missing providers and invalid returned limits stop with a host protocol error.
No native host descriptor-table instruction or table address is used.

The helper exits callback scope before any genuine guest fault, validates the
entire six-byte segment span, distinguishes SS/GP overflow from paging, and
commits through the existing checked byte transaction. Successful execution
retires exactly once as OTHER, ending an earlier STI shadow before the next
instruction may observe a pending host request. Failed memory stores do not
retire; normal CPU fault handling retains its existing exception semantics.
A retirement callback failure retains the already completed store. Table and
raw floating-register images are preserved, including a pending x87 exception.
Nested dispatch gets only its own explicitly bound provider. Descriptor/output/
map/control/FP-bank alias checks and checkpoint-output guards protect both the
frozen and original provider descriptors.

The new fixture passes192 generated/provider cases plus explicit missing,
erroneous, reentrant, faulting, aliased, nested, segment-overflow and retirement
failure gates. Host ASan/UB/leak, actual Linux32 UB, MIPS32 big-endian/QEMU and
GCC/mixed Clang-GCC Windows32/Wine all pass. The independent2,448-record ring0
byte corpus also passes again with its unchanged hash. Existing dispatcher
qualification passes on all five profiles, retaining deep calls, fault/resume,
REP checkpoints, CLI/STI ordering, worker exclusion and raw-state tests.
CI adds portable and Windows-link gates. Complete generator suites pass165host/
114Windows, zero failures or ignored tests, and strict lint/format checks pass.

Owned-disc ordinary/controlled source and reports remain byte-identical to the
previous qualified output:23,958 functions,772,319 entries and41 rejected
functions plus one unbacked extent. Fresh Windows output matches Linux. The
INIT helper remains rejected because its far jump is not yet implemented.
All23,958 definitions partial-link on Linux64/Linux32/Windows32 with53 current
host-runtime objects and two game units freshly compiled per target, while
81 unchanged body objects and qualified unchanged numerical pools are reused.
Private qualification records exact source/object hashes; this is verified
reuse rather than an all-function fresh compile. The initial private link list
omitted the new store object and correctly failed its unresolved-symbol gate;
the final list includes it and all instruction/numerical services resolve.

The fresh startup probe SHA-256 is
`59c0ebadd73da451b5f4d2820db4b39842dca752651f6c726f26f46786e570b6`.
It preserves the prior entry/parent raw and CPU state, completes two critical
imports and reaches the same genuine kernel-header read fault at0x001d3ed0/
0x8001003c. Correctly routed counters still observe four strings/35 REP
checkpoints and zero CLI/STI attempts. This does not prove live-game SGDT/SIDT
execution. Far-jump/descriptor validation, code-segment cache and fetch bounds,
complete kernel metadata/processor/thread setup, IRQ/device/legacy ABI and
signed playable-package/full-game/platform qualification remain open. No
header/processor flag is invented to skip initialization; readiness stays false.

### Direct-code far-jump planning and fetch boundaries

The INIT helper still needs a real far transfer. New pure legacy protected-mode
planning in `runtime/gamepkg_x86_far_jump.c` separates selector lookup from
code-descriptor validation. Null selectors, unusable LDTs and descriptor-table
bounds are checked before any descriptor read. Direct code checks preserve
nonconforming/conforming privilege rules, type-before-presence ordering,
selector-bearing GP/NP errors, operand-size truncation, expanded limits, full
32-bit bases and normalized visible CS. System descriptors remain unsupported
host inputs because gates/tasks need different transitions. Rejected host
inputs leave the output unchanged; guest-fault plans propose no cache update.
`gamepkg_x86_segment_fault` can deliver reviewed GP/NP/SS selector errors through
the existing fault boundary without changing raw flags into an exception frame.

A successful plan contains a proposed code cache and a request to mark an
initially clear descriptor accessed bit. It does not modify RAM or CPU state.
That request still requires a checked, locked rights-byte update before cache
commit. A separate full-instruction-span check accepts one through fifteen
bytes without arithmetic wrap. In particular, EIP equal to the segment limit
can be a valid transfer followed by a fault fetching a multi-byte instruction;
that later fault must retain the newly loaded CS.

The independent ring0 fixture executes real far-JMP instructions inside QEMU's
Pentium III profile at all four privilege levels. Its132,816 records cover all
CPL/RPL/DPL combinations, conforming/nonconforming code and data types,
presence, GDT/LDT, both operand/default sizes, flag bits, base bit patterns,
limit boundaries, invalid selectors and unusable LDTs. The9,562,764-byte
GPKFAR02 corpus SHA-256 is
`51b8f4b78eb2352e32a721a00331f897ad1c1f711fa2853e6d795ab755e6c24c`.
Independent GCC11/GCC13 fixture builds produce identical bytes. Replay checks
7,584 completed transfers,12,800 NP faults and112,432 GP faults, including
error/CS/flag priority and nonmutation. It passes host ASan/UB/leak, actual
Linux32 UB, MIPS32 big-endian/QEMU and GCC/mixed Clang-GCC Windows32/Wine.

Two reference limitations are explicit: QEMU omits all4,384 observed clear-A
updates and permits instruction fetch across CS.limit. Neither omission is
adopted as architectural behavior. A separate native ELF32 process-local LDT
probe verifies that transfer beyond the limit retains the old CS, a valid
transfer commits CS before a crossing MOV fetch faults, and a one-byte INC
retires before the next fetch faults. Physical saved fault flags contain RF1.
Linux has already set A on this probe's descriptors, so it verifies preservation
of an already set bit, not a physical clear-to-set update. Raw corpus bytes are
retained unchanged; no original-Pentium-III or physical-Windows claim follows.

Five hand-authored architectural vectors also run in the standard Rust suite;
these are unit vectors, not fresh hardware observations. Full generator suites
pass166host/114Windows with zero failures or ignored tests; strict lint and
format checks pass. CI adds portable replay and Windows-link gates.

This is a validated planning prerequisite, not admitted far-jump execution.
Descriptor RAM reads/locked accessed updates, owner-controlled cache commit,
full CPU context, dispatch/fetch integration and instruction retirement remain
required. The game helper remains rejected and the last actual startup fault
is unchanged; no new boot or live-game instruction execution is claimed.
Kernel metadata/processor/thread setup, IRQ/device/legacy ABI and signed
playable-package/full-game/platform qualification remain open.

### Checked descriptor reads and accessed-bit transactions

`gamepkg_x86_far_resolve_code` now connects the pure far-jump plans to checked
RAM. Explicit trusted GDT/LDTR metadata is copied by value. Selector/table
prechecks precede any memory access; a complete eight-byte descriptor read
precedes type/privilege/presence/offset checks. Valid direct-code descriptors
then receive a conditional locked accessed-bit update. Only after that succeeds
is the proposed code image published, with no remaining accessed-write request.
Invalid host inputs and unsupported system descriptors return an error with
unchanged output. Real selector and page faults use the live fault boundary,
also preserving the entire output. Failed write admission changes no RAM.
No CPU code cache or instruction PC is committed by this service.

The RAM object provides two narrowly scoped internal descriptor operations.
Reads stage both logical LE words before publishing either. Accessed updates
hold the existing shared lock across rights-byte read, conditional write
admission and bitwise OR, preserving every other bit under concurrent RAM
operations. An already set A bit needs no write permission in this P6 profile.
Implicit descriptor accesses report supervisor page-fault U/S=0 even at CPL3;
ordinary memory retains its existing user-fault behavior. Both paths continue
to enforce the same immutable host RAM capabilities. These rules follow
[Intel's segment-update LOCK semantics, section 7.1.2.1](https://www.intel.com/content/dam/support/us/en/documents/processors/pentium4/sb/25366821.pdf)
and [implicit supervisor accesses, section 4.6.1](https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-vol-3a-part-1-manual.pdf).
The current capability mapper still does not implement CR0.WP/page-table
walking, paging accessed/dirty bits, MMIO or privileged host-memory access.

The synthetic fixture checks 862 cases: selector faults before absent RAM,
semantic faults before forbidden A writes, present/read/write fault errors,
complete-read rollback, all 256 rights-byte values across permissions,
GDT/LDT/entry-zero selection, CPL3 normalization, operand truncation,
discontiguous maps and every descriptor split including linear32 wrap.
Outputs, sentinel RAM, raw GPR/flag fields and pending raw x87/MMX/XMM state
are checked through fault boundaries. Eight real workers perform 16,000
concurrent accessed/CMPXCHG/XADD sequences without losing the counter's other
bits. These are synthetic architecture/host-synchronization checks, not a new
physical clear-to-set measurement; the prior QEMU and physical-A limitations
remain. The initial fixture compared host padding as if it were architectural
x87 state; final comparison uses the established normalized snapshot and also
checks the untouched original source bytes. A Windows helper-name collision
was renamed before final qualification; runtime comparisons were not weakened.

All cases pass host ASan/UB/leak, actual Linux32 UB, MIPS32 BE/QEMU and GCC/
mixed Clang-GCC Windows32/Wine. Standard suites pass167host/114Windows with zero
failures or ignored tests; the corrected final fixture is rerun separately.
Strict lint/format and CI syntax checks pass; CI adds portable/Windows-link
gates. Existing RAM/critical-section tests also pass host, BE32 and both
Windows profiles: 74,112 scalar cases, 526,336 bounded-image cases and the
existing contended/lifetime/critical-section checks per profile.

Fresh Linux64/Linux32/Windows32 partial modules contain all23,958 definitions
and resolve the new descriptor services alongside the existing instruction/
numerical services. Each target freshly compiles55 current runtime objects and
two unchanged game units, reusing81 verified unchanged body objects and the
qualified numerical pools. Copied source parts and all reused object hashes
are checked and recorded privately. The unchanged complete controlled source
SHA remains `dca04bbc610599c2ee66b79d078c480f13f4fa65a32da6ce817e35df1bde927c`;
this is not an all-function fresh compile or new instruction admission.
Fresh startup probe SHA-256 is
`55692364585dce9bb5de5ce5ebcc5df81ba9984fe94c7e6d598dacd565dff14f`.
It preserves entry/parent state, completes the same two critical imports and
reaches the same actual kernel-header fault at0x001d3ed0/address0x8001003c,
with four string entries,35 REP checkpoints and zero CLI/STI attempts.

The caller must still qualify protected32/non-v8086 mode and real CPL/LDTR
state. CPU ownership, code-cache/context commit, full fetch/dispatch/retirement
integration and actual INIT-helper admission remain next. Kernel metadata,
processor/thread setup, IRQ/device/legacy ABI, signed package/launcher and
complete playable-game/platform fidelity are still open; readiness stays false.

### Owned code caches and coherent execution snapshots

The shared CPU now keeps an explicitly initialized CS/LDTR cache image on each
thread handle. `gamepkg_x86_cpu_init_code` requires the active native owner,
configured shared tables, a present/accessed code cache compatible with the
current protected, non-v8086 CPL, and representable expanded limits. There is
no implicit flat cache. Visible selectors, bases, limits, descriptor flags and
LDTR usability are logical integers; no host segment register is read or
loaded. Ignored hidden fields of an unusable, null LDTR remain explicit.

`gamepkg_x86_cpu_snapshot_execution` captures interrupt state, shared table
identity and code caches coherently under one metadata lock.
`gamepkg_x86_cpu_restore_execution` validates the whole proposal before changing
interrupt/cache state together. Shared table identity must match and is never
rewritten by restoration. Invalid pointers, alignment/overflow/CPU-object
aliases, inactive/foreign ownership, malformed caches and incompatible modes
leave state/output unchanged. The old interrupt-only restore/checkpoint paths
cannot introduce a mode/CPL inconsistent with an initialized code cache.
Suspension retains the owning reservation and cache; blocking/handoff preserves
each thread's independent image. This is a partial execution context: PC,
GPR/FP banks, other segment caches, current x87 selector bindings and RAM
bindings still require their own coherent host integration.

`gamepkg_x86_cpu_far_transfer` connects active-owner context to checked
GDTR/LDTR descriptor resolution and the locked A update, then commits only
visible/hidden CS. No CPU mutex spans a guest memory operation or fault;
resolution has no yielding callback and retains the same CPU reservation.
Selector/page/target-limit failures precede cache commit. Commit rejects a
pending accessed-write request or malformed proposal. It preserves CPL, LDTR,
flags and STI shadow. The caller still must commit the truncated target PC and
retire once before the next instruction/event; this helper does neither.
`gamepkg_x86_cpu_check_code_span` uses the actual cached CS for all instruction
bytes and retains a newly committed cache when the subsequent fetch span faults.
It checks segment bounds, not executable RAM, decoding or dispatch membership.
Raw fault flags remain separate from future architectural exception frames.

The final synthetic fixture passes 141 transfer/fetch fault cases, including
all four CPL/RPL values, GDT/LDT, operand truncation, conforming DPL0 code at
CPL3, independent 16-bit target default size, read-only A update, incomplete
descriptor read, and old-CS transfer fault versus new-CS fetch fault.
Raw GPR/flag and pending x87/MMX/XMM snapshots remain unchanged. Additional
configuration/alias/mode/commit/restore rejection gates cover explicit missing
state, both unsupported real/v8086 initial modes, foreign native ownership and
immutable table identity. Eight real workers verify 1,024 independent
restore/suspend/resume leases. All pass host ASan/UB/leak, actual Linux32 UB,
MIPS32 BE/QEMU and GCC/mixed Clang-GCC Windows32/Wine. These are virtual runtime
checks, not a new physical segment-load or original-Pentium-III measurement.
The prior 16,000-entry CPU ownership matrix passes all five profiles again.
Complete generator regressions pass 168 host/114 Windows tests, zero failures
or ignored tests; the expanded final cache fixture is also rerun separately.
Strict lint/format and CI syntax pass; CI adds portable/Windows-link gates.

All 23,958 definitions partial-link on Linux64/Linux32/Windows32 with 56
freshly compiled current runtime objects and two unchanged game units per
target, reusing 81 verified unchanged body objects and qualified numerical
pools. Source parts and reused object hashes are checked privately against
the preceding baseline. No generator instruction is newly admitted and the
controlled source SHA remains
`dca04bbc610599c2ee66b79d078c480f13f4fa65a32da6ce817e35df1bde927c`.
Fresh startup probe SHA-256 is
`4c7d6ad2ce86d019f07c5e86cd80e74d857dfaa9c8382ede76e5d645fa93356d`.
It preserves entry/parent state, completes two critical imports and reaches
the same real kernel-header fault at 0x001d3ed0/address 0x8001003c, with four
string entries, 35 REP checkpoints and zero CLI/STI attempts. The existing
startup context has not been given fictional default segment/kernel metadata.

Generated far-jump admission, scoped dispatcher code providers, target-PC
commit/retirement, mode-aware per-instruction fetch checks and x87 current-CS
binding remain next. Complete kernel/processor/thread/IRQ/device/legacy ABI,
signed package/launcher and playable-game/platform fidelity remain required.
No live-game cache transfer, new successful boot, FPS or playability is claimed;
readiness remains false.

### Scoped far-JMP execution and instruction fetch spans

Controlled blocks now admit exact legacy EA immediate far-JMP encodings, with
one optional operand-size prefix for a 16:16 pointer. Byte length, selector,
offset, immediate list, canonical decoded text and every typed operand field
are cross-checked. Ordinary lowering still rejects these instructions; they
never become near JMP. Extra/duplicate prefixes, forged expressions/metadata
and memory/gate/task forms remain unqualified. Every emitted controlled
instruction now calls `gamepkg_x86_dispatch_instruction` with its actual byte
length before execution.

`GamePkgX86DispatchCode` is a separately scoped, frozen host capability. Its
pure read supplies qualified legacy CS/CPL/GDTR/LDTR state; commit accepts the
fully checked cache proposal without RAM/PC/retirement or yielding. Descriptor
and output/map/control/table/FP-bank alias checks protect entry, and checkpoint
capture excludes original/frozen capability storage. Nested runs bind only
their own providers and restore the outer current-x87-CS binding. Existing
four-field control ABI remains unchanged. Missing/malformed/error/reentrant or
faulting providers stop as host protocol errors, preserving diagnostic faults
rather than manufacturing guest exception retirement.

The far service exits query scope before real selector/paging faults, resolves
all descriptor checks and the conditional locked A update, then commits CS,
target PC and current x87 CS before exactly one OTHER retirement. The retirement
notification names the completed source instruction while the committed target
PC is already visible. Completed CS/PC and descriptor changes survive a
retirement callback rejection. Earlier descriptor/selector/offset failures
retain the old cache/PC; a later crossing fetch fault retains the new cache.
Host errors after an A update do not roll that completed RAM change back.

The new strict CPU entry `gamepkg_x86_dispatch_run_cpu_code_events` requires
explicit table/code-cache setup and active ownership. It binds actual owner
snapshots and commit operations. Checkpoints poll/yield before the next
instruction's span check, so a pending request after STI inhibition ends can
stop at the new target before fetching it. No IRQ is acknowledged or injected.
Flat-base/default32 is the qualified generated execution profile. Transfers to
another valid legacy profile commit and retire but stop before an unqualified
target instruction executes. Older CPU/run entries keep their existing
checkpoint behavior and cannot execute a far transfer without a code provider;
they do not acquire implicit default caches.

`gamepkg_x86_x87_code_segment` changes only the current selector for future
instruction metadata, preserving past FIP/FCS/FDP/FDS, raw FP/MMX/XMM registers
and the independent DS/SS/FS data selection. Nested dispatch restores the
outer binding. Per-instruction fetch spans are not executable-RAM permission,
byte-integrity, dynamic decoding or complete control-flow validation. Near
CALL/JMP/RET target-limit fault and stack-commit ordering still requires
integration; destination fetch guards do not establish that ordering. Complete
segment/thread/handler contexts and production bootstrap remain open.

The ten generated pointer forms agree between builder and streaming output.
The 130-case CPU fixture covers null/out-of-table selectors, GDT/LDT, both
pointer widths, source/target/second-instruction fetch limits, partial/read-only
and absent descriptors, STI request ordering, legacy-provider refusal and
unqualified target profiles. Additional frozen/nested/error/fault/reentry/
alias/boundary/retirement-rejection gates check actual CS-PC-retirement order,
raw pending x87/MMX/XMM preservation and future selector/data metadata.
All pass host ASan/UB/leak, actual Linux32 UB, MIPS32 BE/QEMU and GCC/mixed
Clang-GCC Windows32/Wine. Existing five-profile table-store and deep-dispatch
matrices also pass, retaining 120,000 call depth, 256 worker runs, fault repair,
REP/CLI/STI and CPU ownership checks. Standard generator suites pass 171 host/
116 Windows tests, zero failures or ignored tests; the final far fixture also
passes separately. Strict lint/format and CI syntax pass. Final sanitizer runs
retain leak checks outside the managed sandbox's ptrace restriction. CI adds
portable and Windows-link gates.

Owned-disc controlled generation now admits the actual helper at 0x001d6aac:
23,959 functions, 772,330 map entries, 458 overlaps/455 shared-tail entries,
40 rejected functions and one unbacked extent. No prior admitted function is
removed. All 23,958 prior bodies change because instruction sites gain byte
lengths; none of their older body objects is reused. A private streaming audit
checks all 808,502 emitted label/hook pairs, exact source/body/map accounting
and 83 source chunks. Whole source SHA-256 is
`6a2a7e8632ca953d1f46ba2e051a38c6f72fed055d9492866c322dd703b0bb35`;
body SHA-256 is
`73ccf58afd4f420a4fc03e03be18b5050a7bd7b3bc3515e18ce7d4228bb98304`;
map SHA-256 is
`75b5cbb0573d1603f07b8cb28cb7cedc0f74cf31d9a7bb33135e9bef4b8a35ae`.
Fresh Windows generation matches Linux source and normalized report byte for
byte. Ordinary generation remains unchanged at 23,956 functions.

All 23,959 definitions freshly compile and partial-link on Linux64/Linux32/
Windows32: 83 body units plus a fresh map and 56 fresh current runtime objects
per target. Only unchanged, privately hash-verified numerical pools are reused.
There is no old game-body reuse across the new fetch ABI.

An additional private probe extracts the unmodified newly admitted helper
body, verifying SHA-256
`ccbfff203814d27687653e99f423ad7acd01837237b011b9b2cf40206fbb6b62`
against the generator's function hash. With an explicit synthetic caller frame,
GDT and isolated CPU/RAM domain, its eleven translated instructions run through
the strict CPU adapter on all five profiles. Pre-partial-link symbol routing
observes one SGDT, two descriptor exchanges under virtual IF0, one CLI/STI pair,
one far transfer under STI shadow, one checked A update before one CS commit,
and the previous descriptor returned in EDX:EAX with RET8 stack adjustment.
The cache changes from a 4-GiB code limit to 0x007fffff; exact LE GDTR bytes and
final descriptor match, and pending raw FP/MMX/XMM state stays unchanged. This
is isolated execution of owned translated code with explicit test inputs, not
natural startup progress or a physical Pentium III/Windows measurement. Its
valid return target does not qualify out-of-limit near-return fault ordering.
Game-derived source/executables and all detailed hashes remain private.

Fresh full-map startup probe SHA-256 is
`6eea72ad52a9f494df838881b13c17668ed4c65db27a79b00670021bd1660955`.
This regression still uses the legacy CPU adapter and minimal diagnostic
thread/TIB setup. It preserves entry/parent state, completes two critical
imports and reaches the unchanged genuine kernel-header fault at 0x001d3ed0/
0x8001003c, with four string entries, 35 REP checkpoints and zero CLI/STI
attempts on that startup path. It does not reach the helper or demonstrate
strict-cache bootstrap. No kernel header or processor flag is supplied to
skip initialization. Near-transfer ordering, production code-cache/table/
thread/processor setup, kernel metadata, IRQ/device/legacy ABI and signed
playable-package/full-game/platform qualification remain required. Readiness
stays false; no FPS or playability is claimed.

### Near-transfer limits and transactional stack/counter changes

Controlled CALL/JMP/Jcc/LOOP/RET now check prospective offsets against the
current cached CS limit before committing branch effects or retiring the
instruction. Only taken conditional destinations are checked. LOOP stages the
new ECX/CX value privately, preserving the original counter on a target fault;
untaken LOOP still commits its decrement. A target exactly at the limit is
valid, even when the subsequent instruction fetch crosses that limit. That
later fetch fault retains the completed branch's stack/counter changes.

CALL resolves an indirect operand using the original registers before stack
admission. Stack span/write permission is checked before target rejection,
without writing the return slot; after successful target validation the normal
checked push commits ESP. RET reads the complete return offset before target
validation and only then commits the pop and optional cleanup. Failed transfers
preserve the stack bytes, ESP, ECX, flags and pending raw x87/MMX/XMM state.
Memory JMP operands are captured before retirement, correcting a previous path
that evaluated the operand after retiring the instruction.

`dispatch_check_target` uses the scoped/frozen code provider and returns a host
protocol stop on provider failure/reentry or an unqualified source profile.
Real target overflow raises GP(0) at the source instruction. It does not reload
CS descriptors or commit a new cache. The old `dispatch_call` ABI and ordinary
source remain unchanged. Missing code capabilities retain the legacy flat
behavior; host imports executing outside dispatch still require explicit CPU
validation. Operand16 CALL/JMP/Jcc remain rejected. These checks do not add RAM
execute permissions, instruction-byte verification or dynamic decoding.

The independent `near_physical.c/.S` oracle runs actual ELF32 synthetic
instructions in a process-local LDT code segment (base 0x10000000, limit127),
with protected/missing operand and stack pages and alternate signal handling.
It records 5,796 cases across 23 instruction forms, three targets, six memory
profiles, seven counters and both ZF values. GCC11 and Clang18 builds produce
byte-identical explicit little-endian GPKNEAR1 records: 602,796bytes, SHA-256
`c4e28194b11fcd088df136e0aa91e677bef82521b7f6c68d96efe5758ebc2427`.
The corpus contains 2,184 completed branches stopped at a subsequent UD2
sentinel, 210 SS faults, 2,184 GP faults and 1,218 page faults. Sentinels mark
completion only; the replay does not claim to emulate UD2. Native source-site
LOOP faults preserve the counter; boundary-fetch faults preserve the completed
branch's decrement. Simultaneous invalid-target/stack-write faults establish
stack admission before target rejection, without a partial push. This is
modern native-host evidence, not an original Xbox/Pentium3 or physical Windows
qualification; Intel's legacy [instruction reference](https://www.intel.com/content/dam/support/us/en/documents/processors/pentium4/sb/25366621.pdf)
also documents the transfer and limit-check operations.

Generated replay checks every record through both custom owner-backed controls
and the strict CPU/code adapter, plus 26 fixed rows through both and one
provider-failure row: 11,645 executions per profile. Exact faults/source sites,
post-branch fetch faults, retirement counts, ESP/ECX, stack bytes, unchanged
cached CS/LDTR and pending FP banks pass host ASan/UB/leak, actual Linux32 UB,
MIPS32 big-endian/QEMU and GCC/mixed Clang-GCC Windows32/Wine. The maintainer
script `tools/check_takedawn_near_dispatch.sh` and portable/Windows-link CI
gates reproduce this qualification. No player Python or external game crates
are introduced.

Final generator suites pass 173 host and 117 Windows tests, zero failures or
ignored tests; strict lint, formatting and CI syntax checks pass. The prior far
130-case, table192-case and deep dispatcher/REP/interrupt matrices pass again
on all five profiles. Fresh private owned generation still admits 23,959
functions and 772,330 map entries with 40 rejected functions and one unbacked
extent. Compared with the far-transfer baseline, 9,060 bodies changed and
14,899 are identical, with none added or removed. Ordinary source/report and
fresh Windows/Linux outputs match their respective qualified counterparts.
All 808,502 instruction hooks remain audited. The new controlled source SHA is
`1c6a7b45551862dac1956982d128d34e1f2d33fc8f09f7cacced57198318c093`,
body SHA `a9f928f5693c8d98a6b449ab50ee56c0946d997096f82aeee57975cee880db5c`,
and report SHA `29554e984d315cc164c1d8f3e8a0bd749dff893dee06a3143bd9d56b96234c27`;
the map SHA is unchanged. Every body is freshly compiled: 85 body chunks plus
the map and 56 current runtime objects per Linux64/Linux32/Windows32 target.
Private qualification hashes every source/object and verifies reused numerical
pool hashes; no old game-body objects are reused.

The exact owned helper 0x001d6aac has an unchanged generated body and again
passes its isolated five-profile strict-CPU probe with 11 checked instructions,
SGDT, IF-protected descriptor exchanges, STI shadow, A update, CS commit and
RET8, preserving pending FP banks. The fresh full-map startup executable SHA is
`2704eccc4c835248eb2816206c0681183b95ecf131eecc4cd2a39b6659cb608b`.
Its structured evidence matches the previous startup exactly: parent state is
preserved, two critical imports complete, and the genuine kernel-header fault
remains at 0x001d3ed0/address0x8001003c. That path still uses the legacy adapter
and minimal diagnostic thread context. No fabricated kernel header or
processor flag bypasses initialization. Strict production bootstrap, kernel
INIT metadata, processor/thread lifetime, IRQ/device/legacy ABI, remaining
instruction coverage and signed-package/launcher/full-game/platform fidelity
remain required; readiness stays false. Owned media stays in place and derived
source/executables remain private. No full playability, FPS, physical Windows,
original-Pentium3 or whole-game big-endian claim is made.

### Checked host returns and strict owned startup probe

`gamepkg_x86_cpu_plan_return` supplies the missing near-return operation for
trusted host kernel adapters running outside a dispatcher frame. It snapshots
the active owner's explicitly initialized code/table context, rejects an
unsupported nonflat/default16 profile or invalid host argument before RAM,
checks the complete 2/4-byte stack read, and validates its offset against the
cached CS limit. It returns a by-value status/offset/next-ESP plan. No pointer
output can alias a guest bank. The service does not commit ESP, PC, CPU state,
memory or retirement: the adapter commits the two staged values without
rereading the stack and applies its host-call retirement contract. A source
SS/PF/target-GP leaves the caller's live fault boundary before any commit.
Cleanup arithmetic wraps explicitly through uint32; a return to the CS limit
is valid independently of later fetch. General SS caches, privilege returns,
RETF/IRET and kernel implementations remain separate requirements.

`cpu_return_fixture.c` replays all 1,008 RET16/RET32/cleanup rows from the
independent native near-transfer corpus. It checks source-site faults and
accepts a completed return whose later sentinel fetch faults. It also verifies
fixed success/fault cases, absent code context, inactive ownership, nonzero-CS
base and default16-profile rejection before unreadable RAM, invalid widths and
cleanup, wrapping stack spans/cleanup and ignored adjacent bytes for RET16.
The plan stays untouched on real faults; complete raw GPR/FP/MMX/XMM state,
stack/top-of-address-space bytes and coherent CPU context stay unchanged.
Final replay runs 1,092 cases per host-ASan/UB/leak, actual Linux32-UB,
MIPS32BE/QEMU and GCC/mixed-Clang-GCC Windows32/Wine profile. The new maintainer
script and portable/Windows-link CI gates reproduce the independent native
records and these checks. The earlier CPU code-cache/far/fetch/context matrix
also passes all five profiles: 141 cases and 1,024 eight-worker leases each.

The private full-map owned startup probe now uses
`dispatch_run_cpu_code_events` for the parent and diagnostic worker, rather
than the legacy CPU adapter. Its two actual critical-section host calls use
the new return plan. The probe supplies an explicit private diagnostic GDT
containing its null/base-zero flat CS descriptor, an explicitly unused IDTR,
and an unusable LDTR; these are synthetic test inputs, not a claim to have
recreated the Xbox kernel bootstrap. Shared tables are configured before any
thread and code caches only under active ownership. Parent event-stop and
worker handoff now compare the coherent interrupt/table/code image as well as
the raw banks. The worker checks that its CS/LDTR are preserved after both
checked host returns. No GDT/CPU host pointer becomes a guest address.

Fresh current runtime objects (56 per target) compile and link with all 23,959
owned functions for Linux64/Linux32/Windows32. All 86 unchanged game units and
numerical pools are reused only after source/object hash checks, rather than
claimed as fresh. Controlled source/report/map hashes and instruction
admission remain unchanged. The exact owned setup-helper body again passes
its isolated five-profile probe. The new strict startup executable SHA is
`84bab9445d25d22b36626fca474a0479e408219de9916af11d5c40cb3a1537c7`;
its glue SHA is
`3aa494b16c2d8cde8b926a1b9e1ff8f520890dccd6b98921a6b3ae47b80a57e2`.
All previous structured startup evidence matches exactly, with the additional
record confirming two checked host returns and preserved CS/LDTR. The genuine
kernel-header fault remains at 0x001d3ed0/address0x8001003c. No kernel PE/INIT
header or processor flag was invented to bypass it.

The generator suites pass 174 host and 117 Windows tests with zero failures
or ignored tests. The final test-only adjacent-byte expansion also passes its
focused sanitized test and all five profiles. Strict lint, formatting and CI
syntax checks pass. The strict diagnostic path is progress toward production
integration; real kernel/INIT metadata, processor and complete thread setup,
IRQ/device/legacy ABI, remaining instruction coverage, signing/launcher and
full gameplay/platform fidelity remain required. Readiness stays false; owned
media/source/executables stay private, no installed library changes, player
Python/guest crates/instruction metering, FPS/full-playability/original-P3 or
physical-Windows/full-game-endian claims are introduced.

### Backed virtual kernel image and actual owned section lookup

`kernel_image.rs` builds GamePKG's own 24-KiB PE32 virtual kernel module at
0x80010000. Its four backed sections are `.text`, `.rdata`, `.data` and the
last `INIT` section. The module contains explicit caller-provided hardware and
version data, ordinal-only exports, guarded host trap entries, a writable GDT
and actual translated INIT instructions that initialize the null and flat CS
descriptors. The entry point and last section refer to this real code, rather
than an empty header used to skip initialization. The loader still supplies
explicit boot-mode/code caches and table registers; no host segment/table
address or retail kernel/BIOS is copied into the image.

Current reviewed exports are function ordinals255/277/291/294 and data
ordinals322/324. Function entries contain a guarded trap marker and ordinal,
not a successful generic RET/no-op implementation. Exact entry-PC lookup is
separate from data/interior/INIT addresses and from guest instruction decoding;
UD2 remains unadmitted. Unknown exports/imports remain explicit unresolved
services. Import binding validates selected original/loaded slots and ranges
before any write, including duplicate slots, and preserves the entire arena
on failure. The immutable module retains its image budget charge. RAM mapping
and translated code registration are separate loader responsibilities; this
work does not add general RAM execute permissions or instruction-byte checking.
The PE layout follows [Microsoft's specification](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format),
and the ordinal identities come from the CC0
[nxdk export interface](https://raw.githubusercontent.com/XboxDev/nxdk/master/lib/xboxkrnl/xboxkrnl.exe.def).
A strict independent LLVM reader rejected zero RVAs for empty export-name
arrays; the final image backs these arrays too. Both LLVM18 readobj and MinGW
objdump parse the final headers, all sections and exports successfully.

Rust tests check section/raw spans, address/size/entry consistency, exact LE
profiles and export bytes/types, invalid/duplicate/rebound/unbacked slots,
unknown services and budget rejection. Generated C executes the actual five
INIT instructions through the strict CPU adapter. Six cache/memory profiles
and 24 read-only section-write checks verify real descriptor writes, retained
hidden CS on GDT changes, per-instruction partial completion before later
faults, stack/fault sites and unchanged pending x87/MMX/XMM state. All30 checks
pass host-ASan/UB/leak, actual Linux32-UB, MIPS32BE/QEMU and GCC/mixed-Clang-GCC
Windows32/Wine. The maintainer script and portable/Windows-link CI gates
reproduce this qualification. The final full generator suites pass178host and
120Windows tests, zero failures/ignored; strict lint, format and CI checks pass.
Windows/Linux metadata and translated INIT fixtures are byte-identical.

Private owned integration now loads this backed image, binds six reviewed
function/data slots, executes its INIT before title entry and restores the
explicit caller frame. Its GDT is shared with the strict CPU adapter. The
natural worker still completes two critical imports and preserves parent
raw/coherent CPU context. An added null-page guard exposes a previously hidden
earlier dependency: FS:[0x20] is not yet a configured PRCB pointer. The genuine
fault is now PF14 at0x001d3fe4/address0x250, before the kernel section lookup.
Earlier probes reaching the header had implicitly obtained zero from mapped
low RAM for that unconfigured pointer's +0x250 read; they did not prove a valid
processor bootstrap. The earlier observed header fault is superseded by this
guarded evidence. No PRCB pointer or opaque flag value is fabricated here.
The code's branch at0x001d3fec skips the lookup for a nonzero field; zero
executes it. The field's original semantic identity remains unqualified:
[Cxbx's incomplete KPRCB declaration](https://raw.githubusercontent.com/Cxbx-Reloaded/Cxbx-Reloaded/master/src/core/kernel/common/types.h)
also leaves that offset inside an unknown region. No host Windows structure is
substituted. The initial interpretation that startup had progressed beyond
the header was corrected after tracing this actual control flow.

A separate isolated explicit caller executes the unchanged owned lookup at
0x001d3ecb against the backed image and its initialized GDT. It genuinely reads
the PE header, recognizes `INIT`, constructs the descriptor and calls the
actual setup helper at0x001d6aac. Exactly one CLI and STI accompany the far
reload; CS limit becomes0x80015fff, old descriptor0x00cf9b00:0x0000ffff is
stored in the title globals, and the caller returns with its pending FP banks
preserved. This tests the complete lookup/reload chain with synthetic caller
inputs; it is explicitly not a natural-startup/PRCB or full-gameplay proof.

Final kernel image SHA is
`bddd8d466b80b2ce40e7099a9cf74aa49072d053c7f7a365f3ccdb66f426fb1c`,
generated INIT/header SHA
`0678ca7199d5631d99ce0aaf8186124080fada63c0f5b1ae1574518acae0d0a5`,
and combined private probe SHA
`28b09c2518a273c3f3a355fc264d99e28f5ac219aec98563eabbe5b99e06cf55`.
Fresh owned controlled source/report remain byte-identical to the near-transfer
baseline:23,959functions/772,330entries,40rejects/oneunbacked; no owned body,
map or helper ABI changed. Private qualification rechecks all reused86game
units/56runtime objects and numerical pools per target, plus the full owned
probe module; no reused object is claimed fresh. The combined host probe adds
one virtual INIT function and five entries to the execution map. Kernel image
code/data are GamePKG's own; owned title source/disc bytes remain private and
in place. Complete processor/thread/kernel services, IRQ/device/legacy ABI,
remaining instruction coverage, signing/launcher and full gameplay/platform
fidelity remain required. Readiness stays false; no installed library changes,
player Python/guest crates/instruction metering or FPS/full-playability,
original-P3/physical-Windows/full-game-endian claims are introduced.

### Explicit sparse processor/thread context and natural INIT path

`processor.rs` constructs initial processor and thread field images using fixed
little-endian values and logical32-bit addresses. Known offsets follow the
public [Cxbx/OpenXDK declarations](https://raw.githubusercontent.com/Cxbx-Reloaded/Cxbx-Reloaded/master/src/core/kernel/common/types.h);
no host layout or pointers are copied. The CPU-global self/PRCB pointers, cold
IRQL/next-idle/DPC state and initial thread NT_TIB/current-thread/stack/TLS
metadata are separate projections. PCR and thread reservations cannot overlap
each other or the declared stack. Address wrap, alignment, TLS extent and
entry-frame bounds fail before an image is exposed. Loader installation
validates every destination before writing any field. Unknown bytes remain
untouched and must be carved out of broad RAM mappings. This constructs initial
state; it does not allocate a kernel thread or implement a scheduler.

The constructor requires the explicit `RunTitlePath` virtual compatibility
profile. It supplies read-only zero at PRCB+0x250 because the verified owned
branch executes the complete INIT/code-limit path for zero. A nonzero skip
profile is unavailable. The field's original Xbox semantic identity remains
unqualified; this is GamePKG's declared initial virtual profile, not a guessed
host CPU feature or physical-Xbox value. This differs from leaving an invalid
PRCB pointer to obtain accidental zero from low RAM. Other unmodeled processor
and thread fields are not supplied as readable zero objects.

The API separates cold CPU-global fields from initial thread fields. A future
scheduler must preserve guest-mutated NT_TIB/SEH/TLS/kernel-stack state and
activate the current-thread projection under CPU ownership. Replaying an
initial image on resume would erase real changes. Shared PCR activation,
thread allocation/lifetime/exit, IRQL/DPC/IRQ integration and the opaque word's
broader original meaning remain open requirements. The private parent still
uses its earlier minimal diagnostic setup; this does not claim a production
shared-PCR scheduler.

Tests cover explicit LE field spans, self/list links and thread/TLS metadata;
address-wrap/high-address boundaries, misalignment, stack/TLS/structure overlap,
wrong-processor images, truncated/wrapping/unbacked RAM and failure atomicity.
Generated code performs actual FS-to-PRCB/current-thread/TLS/DPC/profile-word
loads. Nine complete/missing-field/null-FS traversals and27 read-only/unknown-gap
gates pass per host-ASan/UB/leak, actual Linux32-UB, MIPS32BE/QEMU and GCC/mixed
Clang-GCC Windows32/Wine profile, preserving pending x87/MMX/XMM banks. Inactive
unused fields are not incorrectly required by the traversal. Windows/Linux
field and translated-source fixtures are byte-identical, SHA
`4d2549d2d8beffd030b832503f4b30864efb4a083405c67b67d6971325620150`.
The new maintainer script and portable/Windows-link CI gates reproduce the
qualification. Full suites pass182host/123Windows tests with zero failures or
ignored tests; strict lint, formatting and CI syntax checks pass.

Owned startup now calls this Rust loader builder for the diagnostic worker.
PCR0x03fb0000/reserved0x280 and thread0x03fb1000/reserved0x140 are separate,
correcting the previous overlapping test layout. The bound memory object
carves both reservations out of general RAM, registering only seven modeled
field spans, with the profile word read-only. Before entry, actual unknown-gap
reads and a profile-word write are required to fault without raw-state changes.
The null-page guard remains. The worker naturally reads its explicit PRCB,
executes the original section lookup and actual setup helper, performs exactly
one CLI/STI, loads CS limit0x80015fff and stores the original descriptor in the
title globals. Its LDTR and parent raw/coherent CPU image remain preserved;
two critical imports complete. This is the natural path, extending the earlier
isolated lookup proof. No lookup branch is patched or skipped.

The worker then stops explicitly at unresolved ordinal47,
`HalRegisterShutdownNotification`, PC0x8000002f. The inspected request returns to
0x001d2f02, registers object0x003c0388 with callback0x001d6861 and priority bits
0x80000000 (lowest signed priority), with initially zero links. The callback
is admitted by the current translator and includes restoration of the saved
code descriptor. Registration, ordered callback dispatch/unregistration and
correct shutdown integration remain required; the old backend's never-invoked
stub is not substituted. The initial informal identification as PCI access was
corrected against the [nxdk ordinal interface](https://raw.githubusercontent.com/XboxDev/nxdk/master/lib/xboxkrnl/xboxkrnl.exe.def):
PCI access is ordinal46, shutdown registration is47. The
[nxdk callback declaration](https://raw.githubusercontent.com/XboxDev/nxdk/master/lib/xboxkrnl/xboxkrnl.h)
also specifies the registration argument, signed priority and list entry.

Private combined probe SHA is
`222e45e34f9ca63fe32b005dfd71441a3c92746260dd910d0657ff98a9804329`,
glue SHA `62ad5b0f64fbb4acc4ee6c17ce7197871c154d4685381bb3538ed71669dc6735`,
and loader SHA
`27952b1073cd7fd778cd0de6e9fe738c5fa52c38ecd981eae8e117e15ddef02d`.
Owned source/report/map, the backed kernel image and native instruction helpers
remain unchanged; all reused game/runtime/numerical objects are rehashed in
private qualification. No reused object or whole-platform gameplay is claimed
fresh/qualified. Kernel services, complete scheduler/context activation,
IRQ/device/legacy ABI, remaining instructions, signing/launcher and full-game
platform fidelity remain required. Readiness stays false; no installed library
mutation, extracted/public owned media/source, player Python/guest crates,
instruction metering or FPS/full-playability/original-P3/physical-Windows/
full-game-big-endian claim is introduced.

### Shutdown registration and checked guest callbacks

`runtime/gamepkg_x86_shutdown.c` implements the16-byte LE shutdown-registration
ABI: guest routine PC, signed32 priority and intrusive list links. Higher signed
priorities run first; equal priorities use stable registration order. The
[nxdk interface](https://raw.githubusercontent.com/XboxDev/nxdk/master/lib/xboxkrnl/xboxkrnl.h)
specifies the registration argument and priority ordering. Guest addresses are
logical32-bit values, never cast to native callbacks or host pointers. Routine
PC is reread when reserving a callback; sorting uses its registration-time
priority. Registered objects can be removed during shutdown, and new entries
can be added while draining. Self-registration uses a distinct queued host node
while the prior callback ticket remains active. Duplicate queued objects and
overlapping/corrupt link state fail without mutation or guest-pointer chasing.
Unlinking preserves the removed record's own stale links, changing only the
neighbor/head words.

Every operation requires active ownership in the exact borrowed CPU domain,
the exact bound supervisor RAM object and the qualified flat/default32 CPL0
profile. New nonmutating CPU/RAM identity guards enforce this before RAM.
The CPU and backing memory must outlive the service. All list access must use
that CPU reservation; this does not claim atomic visibility to arbitrary
out-of-band host writers. All potentially faulting reads/write admissions
precede host-node allocation and guest-list writes. Immutable live bindings
make the subsequent commit nonfallible under that contract. No native mutex
is held over a fault boundary or callback. Heap exhaustion reports an error,
with no successful registration fabricated and no execution-count quota.

Shutdown reservation unlinks one entry and issues a host-only value ticket.
Only one ticket may be active. A checked host-to-guest stdcall1 transition
admits the whole8-byte return/argument frame, checks the target, and then writes
it. This is an explicit ABI transition, not a claim to emulate a physical
PUSH/CALL instruction sequence. Frame faults preserve guest registers, CPU
state and pending FP banks and retain the ticket for retry. Callback execution
uses the strict guest dispatcher. Completion requires the prepared return trap
and original ESP after RET4; wrong PC/ESP, stale tickets, faults and unresolved
imports cannot finish it. Callback changes to RAM, FP and code caches remain
observable. Normal release requires a sealed drained registry. Explicit abandon
is reserved for quiescent terminal process/image teardown, with no guest resume
or successful-cleanup claim; it also works after an unsupported cache change.

The virtual kernel image appends ordinal47 without moving existing trap PCs,
reserves a backed list head at0x80014010 and a non-exported callback return trap
at0x80011100. It remains an own virtual kernel image, not a retail BIOS or
complete kernel implementation. The return trap does not admit guest UD2 or
become a callable kernel export. Independent LLVM/MinGW PE readers accept the
extended image. The loader initializes the list explicitly after kernel INIT.

Generated tests cover signed extremes/equal priorities, live routine changes,
priority capture, cancellation and registration during draining, self-repeat,
stale/duplicate completion, corrupt links, overlapping objects, unreadable and
read-only RAM, wrapped frames, target-GP versus stack-PF ordering, wrong CPU,
wrong/missing/supervisor RAM binding, inactive ownership and terminal abandon.
The first concurrent fixture correctly failed because its workers had omitted
the required memory binding; after fixing the fixture, the ownership/binding
contract and1024eight-worker register/remove cycles pass. Each of five profiles
passes63checked transitions and10completed actual generated guest callbacks,
plus a real callback fault that remains unfinished: host-ASan/UB/leak, actual
Linux32-UB, MIPS32BE/QEMU and GCC/mixed Clang-GCC Windows32/Wine. Pending raw
x87/MMX/XMM state and no-partial-mutation checks pass. The kernel image's30case
matrix also passes again. New portable/Windows-link CI gates are present;
full suites184host/124Windows pass with zero failures/ignored. Strict lint,
format and CI syntax pass. Windows/Linux callback and kernel fixtures match
byte-for-byte; callback fixture SHA is
`6b634dca07a31d1d81c7b6d387a3a7d47ebcc005f5921d01c2d65eee5ef10404`.

Owned startup now binds seven reviewed function/data imports. The actual
ordinal47 call registers the real0x003c0388 object with callback0x001d6861 and
lowest signed priority, updates real checked guest list links and returns using
the checked host-import return plan. Natural startup advances to unresolved
ordinal107 (`KeInitializeDpc`), PC0x8000006b, while preserving the actual INIT/CS
reload, parent raw/coherent state and earlier two critical imports. No fake
success or never-invoked shutdown stub is substituted.

A separate diagnostic-close attempt reserves that actual owned callback,
builds its stdcall frame and enters it through the full strict map. It stops at
unresolved ordinal129 (`KeRaiseIrqlToDpcLevel`), PC0x80000081, before its device
cleanup/descriptor restoration can complete. The ticket stays unfinished;
normal callback completion is not claimed. The development probe explicitly
abandons the process image after this stop. This verifies real callback entry,
not fully functioning shutdown or integration with the final GamePKG launcher.
DPC initialization, interrupt-priority state, device/kernel services, callback
continuation and final lifecycle integration remain required.

Private combined probe SHA is
`3fd94566da6ae877be72f6ee646d109271bfb69dcb3f10f146899d705a7fdb15`,
glue SHA `fb98e2ff6ceb13ecb47baad5cb4ac3d72b1c819c20bd4b5a183a2562c222a073`,
and extended virtual kernel SHA
`996dad96627feb1783e2a2c4f14001ea5f1fc1410a73e9177101b28baa8954a4`.
All23,959owned definitions link on Linux64/Linux32/Windows32 with57fresh current
runtime objects per target and86hash-verified reused game units. Numerical pools
are also rechecked; no unchanged game body is claimed fresh. Owned source/map
and instruction admission remain unchanged. Production processor/thread
lifetime, IRQ/device/legacy ABI, remaining instructions, signed package/launcher
and complete gameplay/platform fidelity remain open. Readiness stays false;
owned media/source stay private and in place, with no real library mutation,
player Python/guest crates/instruction metering or FPS/full-playability/
original-P3/physical-Windows/full-game-endian claim.

### Kernel IRQL/DPC integration and the next actual startup stop

The checked runtime now has explicit required-IRQL CPU domains, supervisor PCR
mirror binding, get/raise/lower operations and scheduling inhibition at
DISPATCH_LEVEL or above. The strict kernel dispatcher activates each retained
priority on entry/resume and detects unexpected guest mirror edits. Partial CPU
restore cannot lower priority. Host stop requests remain distinct from device
interrupt delivery. The new DPC initializer writes only defined fields after
whole selected-span admission. See runtime/README.md for ownership, lifetime,
fault-ordering and remaining scheduler/DPC obligations.

The owned-media private probe freshly compiles59 runtime objects per target and
links all23,959 previously admitted owned definitions on Linux64/Linux32 and
Windows32. Its86 reused game units per target and numerical pools are rehashed
against the qualified baseline; reused bodies are not claimed freshly compiled.
Owned source/report/admission remain unchanged. The virtual kernel appends
ordinals107/129/160/161 without moving existing trap PCs. Both LLVM and MinGW
still accept its backed PE sections/exports. Eleven reviewed owned import slots
are now bound; other services remain unresolved.

Natural startup executes `KeInitializeDpc` with a checked stdcall return and
advances to unresolved ordinal113, `KeInitializeTimerEx`, at0x80000071. The real
INIT/CS reload, saved previous descriptor, two critical-section calls and parent
raw/coherent-state preservation remain verified. This is a diagnostic worker
created from captured actual arguments, not full `PsCreateSystemThreadEx` or a
production shared-PCR scheduler.

A separate diagnostic close enters the actual registered callback0x001d6861,
completes `KeRaiseIrqlToDpcLevel` and retains priority2 in both trusted state and
the PCR mirror. It then faults on the unresolved device-object export: checked
supervisor write at0x001d6870 to0x80000175, vector14/error2. The callback stays
unfinished. The process reports the retained reservation and exits78; it does
not lower priority to resume the parent, claim successful shutdown, or fabricate
a zero-filled device object. The ordinary startup probe leaves that diagnostic
close disabled and terminates from its separate PASSIVE-level service stop.

`kernel_priority` adds two Linux/one Windows tests. Full locked release suites
pass186 Linux/125 Windows tests, zero failed/ignored. Five-profile priority/DPC
qualification passes1,104 checked transitions and1,024 eight-worker activations
per profile, preserving pending x87/MMX/XMM state. Linux/Windows generated
fixture bytes match, SHA256
`021d39cb8f95469db3a6167fd324d8c1e21a8cc24bc44549baa4f2fdfa191fce`.
Existing30-case virtual-kernel execution/protection checks pass all five profiles.
Portable and Windows-link CI gates include the new fixture; remote CI was not run.

Private startup executable SHA256
`b88c8fdc948ea79dea98fd9221fa3375faa87173b8bcbff616e91a439af86d43`;
glue `1868d8414b7383eaf3ddf3cb83075cdb08880d91ff0c1a2216da399231b2f3f4`;
virtual kernel `2c9661053767b3092fc85a4aaa076dfb5c74765037c833beda9a5ed9a3981c84`.
Evidence and derived owned code remain private temporary artifacts. No player
Python, guest dependencies, instruction metering or user-library changes are
introduced. Timer/event/device integration, DPC queues and callback completion,
full kernel/thread/IRQ/context integration, legacy FP ABI convergence, remaining
instruction rejections, launcher/signing and complete gameplay/platform fidelity
remain required. Burnout3 readiness remains false; these checks do not establish
full playability, FPS, original-Pentium3 or whole-game-endianness parity.

### Timer initialization completes on natural startup

The virtual kernel appends113 `KeInitializeTimerEx`,112 `KeInitializeTimer`,
and108 `KeInitializeEvent`, keeping older host-trap addresses stable. The checked
runtime initializes16-byte event and40-byte inactive-timer layouts with selected
write preflight, explicit supervisor/owner/code-profile guards, little-endian
fields and preservation of untouched data. It does not implement arming,
expiration, waiting, cancellation or DPC delivery. Those behaviors remain open.

The actual owned worker now completes ordinal113 and its checked stdcall return.
Its notification timer at0x00754d60 has Type8, Size10, SignalState0 and both wait
links0x00754d68. One timer initialization is observed; no original event/default
initializer call is claimed. Twelve owned imports bind to the reviewed backed
kernel. Natural startup advances to unresolved24 `ExQueryNonVolatileSetting`
at0x80000018. A fault-bounded inspection captures its actual five arguments:
index17 (`XC_MISC`), four-byte value buffer, separate Type/ResultLength pointers,
and return0x001d294a. The settings query remains unimplemented; no fabricated
success/value or unrelated host setting is substituted. The next integration
must model the virtual console setting and the checked five-argument ABI.

Real INIT/CS reload, previous descriptor storage, two critical-section calls,
DPC initialization, parent raw/coherent state and the isolated pending-FP
lookup checks remain verified. The separate actual shutdown callback still
raises priority2 then faults at0x001d6870 writing0x80000175 through unresolved
`IdexChannelObject`357. It retains CPU ownership and exits the diagnostic78,
without completing the ticket or resuming the parent. This is diagnostic thread
startup from captured arguments, not completed production PS/thread lifecycle.

Fresh60-object runtime builds link all23,959 admitted owned definitions for
Linux64/Linux32/Windows32. All86 reused game units per target and numerical pools
are rehashed against the prior qualification; unchanged game bodies are not
claimed freshly compiled. Owned admission/source/report hashes remain unchanged.
The expanded virtual PE still parses in LLVM/MinGW readers and passes its
existing30-case execution/protection matrix on five profiles.

New event/timer tests pass752 checked cases and eight generated reads per
profile on sanitized host, actual Linux32, MIPS32 big-endian/QEMU and both
Windows32/Wine compiler profiles. Linux/Windows generated fixtures match SHA256
`17a95c033cffafc0705c1cee66d722040fad9f60c0aaf6b16de327521a0a755b`.
Full locked release suites pass188 Linux/126 Windows tests, zero failed/ignored.
Portable and Windows-link CI gates include the new component; no remote CI run
is claimed. Startup executable SHA256
`3563042ebcc8d99fc0954a7d8693ed60fe8c31d8d7b3303b1fa2eeba44a8716e`;
glue `06dd70fd59d9e493d5eb04213404720f725d0d7b15e1173bbba5deebd509968b`;
virtual kernel `c77110495f4d13b583390eb56d305791b336bbcbf5621e614bd6e4d4c78c4b28`.
Derived owned code/evidence remain private. No player Python, guest crates,
instruction quota, user-library mutation or playable/FPS claim is introduced.
Settings/device/clock/timer/event/DPC semantics, full thread/IRQ/context and FP ABI
integration, remaining instruction rejections, signed launcher delivery and
complete gameplay/platform fidelity remain required. Readiness stays false.

### Settings query completes; error-status conversion is next

The virtual kernel appends ordinal 24 without moving older trap addresses.
`gamepkg_x86_nv.c` freezes explicit logical console values and implements the
checked query ABI, required type/value outputs, optional result length, short
buffers, unknown-index error, deterministic output overlap and whole-query write
preflight. A known setting absent from the supplied profile stops as an
unconfigured service; no zero-filled fallback pretends to implement it. This is
read-only logical configuration. Persistent save, raw EEPROM checksums and
crypto are not implemented by this component.

The private owned startup uses an explicitly configured cold virtual-console
`XC_MISC` value of 0 (no special flags). It does not read host settings or claim
physical-console identity. The actual five-argument query completes, with
REG_DWORD type 4, value 0 and ResultLength 4 independently checked in guest RAM,
then returns with the checked 20-byte stdcall cleanup. Thirteen owned imports
are bound to the reviewed kernel. Startup naturally reaches unresolved ordinal
301, `RtlNtStatusToDosError`, at 0x8000012d. The captured actual conversion
request has NTSTATUS 0 and return address 0x001d2951. Conversion remains
unimplemented; no native host call, constant return or skipped caller code is
substituted. The ordinal and one-argument ABI are checked against nxdk.

INIT/CS reload, saved descriptor, critical-section calls, timer/DPC initialization
and parent raw/coherent state remain verified. The separate actual shutdown
callback still raises priority 2 and faults at 0x001d6870 while writing
0x80000175 through unresolved `IdexChannelObject` 357. The ticket remains
unfinished; the diagnostic retains CPU ownership and exits 78 without resuming
the parent. No full PS/thread lifetime or device cleanup is claimed.

All 23,959 admitted owned definitions link on Linux64/Linux32/Windows32 with 61
fresh runtime objects per target. The 86 reused game units per target and
numerical pools are rehashed; unchanged game bodies are not claimed freshly
compiled. Owned source, admission and report hashes remain unchanged. LLVM and
MinGW still parse the expanded virtual PE, and its 30-case execution/protection
matrix passes all five profiles. The new query fixture passes 3,686 checked
cases and 11 constructor rejections per profile while preserving pending
x87/MMX/XMM and IRQL. Linux/Windows fixture bytes match SHA256
`b0a3c9d8d39b79c1827e194fa18cda63ca2772be58b01f7c685dbbcbd8410635`.
Full locked release suites pass 190 Linux / 127 Windows tests, zero failed or
ignored. Portable and Windows-link CI gates include the component; no remote
CI run is claimed.

Private startup executable SHA256
`a94c37d862f1b22321a7b53d31f67742905f061b841ef62ef917535e657d3025`;
glue `91dbe7227f3e5bdae672003b64261fcc63ca0ed283acf7fdcbda0b785b3875fe`;
virtual kernel `3222a554085309e8868871fec829cc31931ee9cf172db57dc0823b3d24199e5b`.
Derived owned code/evidence remain private. No player Python, guest crates,
instruction quota or user-library mutation is introduced. Status conversion,
remaining settings/persistence, clocks/timer/event/DPC semantics, devices,
full thread/IRQ/context and FP ABI integration, remaining instruction rejections,
signed launcher delivery and complete gameplay/platform fidelity remain open.
Readiness is false; these checks do not establish full playability, FPS,
original-Pentium3 or full-game endianness parity.

### Status conversion completes; the first timer arming is next

The checked virtual kernel appends ordinal 301 while preserving earlier trap
addresses. Its portable converter uses all 1,586 entries/43 ranges from pinned
Wine 9.0 sources, retained with their LGPL-2.1-or-later notices, full license and
corresponding regeneration/relink inputs. It does not call native ntdll in the
player or write host TEB state as guest state. Exact original retail Xbox
extended-status table parity remains unqualified; the profile is explicit.

All 730,816 selected cases match both the separately compiled original NoTeb
body/table and the installed Wine 9.0 ntdll exports. GCC and mixed Clang/GCC
Windows32 native corpora match the original-source corpus byte for byte. The
5,846,552-byte little-endian corpus replays on five profiles while preserving
pending x87/MMX/XMM and GPR banks; SHA256
`8fee9d8937127372b77ee40bc66fc20d31ae7619cddefe3825b54a33f3d63f7e`.
The checked generator rejects unrecognized expressions, verifies all five
upstream file hashes and reproduces exact numeric/reference products. Original
reference/native-oracle code is absent from player runtime links.

Actual owned startup now completes conversion of NTSTATUS 0 to DOS error 0,
including its checked one-argument stdcall return, and continues through the
original caller instructions. Fourteen imports are bound. The next natural
unresolved service is ordinal 149, `KeSetTimer`, at 0x80000095. Fault-bounded
inspection captures timer 0x00754d60, due-time words b5659000:ffffffcd
(low:high), DPC 0x00754d40, callback 0x001d67bf and return 0x001d67be.
The signed due time is -216,000,000,000 units of 100 ns: a relative six-hour
request. The notification timer is inactive, signal/old-DPC/period are zero,
and the initialized DPC has Type19 and context0. Timer arming, expiry and DPC
delivery remain unimplemented; no immediate callback, constant-success arm or
skipped timer is substituted.

INIT/CS reload, saved descriptor, critical sections, timer/DPC initialization,
actual settings outputs and parent raw/coherent state remain verified. The
separate shutdown callback still raises priority2 and faults on unresolved
device-object state at 0x001d6870, writing 0x80000175. It retains its unfinished
ticket/CPU reservation and exits the diagnostic78 without resuming the parent.
Full PS/thread lifetime, device cleanup and production bootstrap remain open.

All 23,959 admitted owned definitions link for Linux64/Linux32/Windows32 with
62 fresh runtime objects each; 86 reused game units per target and numerical
pools are rehashed. Owned source/report/admission remain unchanged. Reused game
bodies are not claimed freshly compiled. LLVM/MinGW accept the expanded PE and
the 30-case kernel execution/protection fixture passes all five profiles.
Full locked release suites pass 193 Linux / 129 Windows tests, zero failed or
ignored. Portable and Windows-link CI gates include conversion checks; remote
CI was not run. Linux/Windows generated fixture SHA256
`2172cecb5e0172361d4947037c810ee7f7f1f3803c1e8bfa31cefc0104d46d4b`.

Private startup executable SHA256
`35d0d7fbb0208e890e85e846aa3eba8e4853c324f109565e1eae1f7aa43344c4`;
glue `7a0f1659b51bb1c60c1ac45ee148a634121b49f8b3236778031cab9fa1a4e785`;
virtual kernel `23442dca5628be4ccac6fc020926e1af56f344471ab039331efaf40ad8292ef1`.
Derived owned code/evidence stay private; no signed player package is produced.
No player Wine/Node/Python, guest crates, instruction quota or library mutation
is introduced. Clock/timer/DPC/event delivery, devices, complete thread/IRQ/
context and FP ABI integration, remaining instruction rejections, source/signing/
launcher delivery and complete gameplay/platform fidelity remain required.
Readiness is false; no full playability, FPS, original-Pentium3, physical-Windows
or whole-game-endianness claim follows from this component qualification.

### Timer and DPC queue integration (2026-10-22)

The runtime now provides timer arming/rearming/cancellation, relative and
absolute deadlines, wall-clock adjustment, periodic coalescing and checked DPC
queue/claim/frame/return operations. Timer expiry sets the signal and queues its
DPC with system-time low/high arguments. Cancellation preserves signal state
and already queued callbacks. Duplicate DPC insertion preserves arguments and
FIFO order. Active callbacks can enqueue themselves and rearm timers.

The services use explicit little-endian guest records, host-owned membership,
exact CPU/RAM ownership, overlap checks and complete selected-write admission
before mutation. Guest links are validated against trusted metadata rather
than chased as native pointers. A DPC claim raises trusted/mirrored IRQL2;
completion requires the host's actually observed prepared return PC/ESP and
restores the previous level. An unfinished/faulting callback retains its ticket
and CPU reservation. Stack admission precedes target GP, and faults permit a
checked retry. No host mutex crosses a guest fault. Waiters currently return
an explicit unsupported dispatcher-wait result; they are never falsely woken.

A generated asset-free fixture checks 5,468 transitions and 11 actual generated
callbacks per profile, including callback timer rearming, IF/STI/priority
inhibition, duplicate/self-requeue, every timer-write and late DPC argument
byte fault, bad links/overlaps, stack-PF-before-target-GP ordering, callback
fault repair and raw pending x87/MMX/XMM preservation. An independent Rust
i128 oracle supplies 1,024 concrete clock rows covering signed due-time and
counter extremes, INT64_MIN, clock jumps and maximum periodic intervals.
Linux64 address/undefined/leak checks, Linux32 UB checks, actual big-endian
MIPS32 under QEMU, and Windows32 GCC plus mixed Clang/GCC under Wine all pass.
These are component portability checks, not whole-game/platform parity.

Five virtual-kernel traps are appended without moving old entries:149
KeSetTimer,150 KeSetTimerEx,97 KeCancelTimer,119 KeInsertQueueDpc and137
KeRemoveQueueDpc. DPC returns have a separate guarded marker at0x80011108;
return markers and timer-list storage cannot invoke host services. Existing
PE/export/INIT protections remain checked.

Private owned startup binds19 imported slots, successfully arms the original
six-hour timer0x00754d60 with DPC0x00754d40 and executes the original caller.
It now stops at original code0x002443fc, an indirect small-copy target absent
from the generated map. No replacement memcpy or skipped game call is used.
The private driver samples monotonic uptime and real system time, polls for
host boundary work and executes eligible DPCs with checked guest frames. It
saves/restores interrupted raw/cache context separately from persistent
callback RAM effects. This is diagnostic integration; complete thread/PCR,
SEH/IRQ/device and FP-context behavior remains unqualified.

A diagnostic virtual six-hour advance runs the actual callback0x001d67bf and
observes RET16 reaching the DPC return marker. The unmodified cold-start flag
is zero, so this qualifies only its cold branch. A separate diagnostic setting
that flag to1 reaches unresolved ordinal360 HalInitiateShutdown, retains IRQL2
and the unfinished callback, and exits78 without resuming the parent. The
existing shutdown callback separately retains the unresolved device-object
fault at0x001d6870/0x80000175. Neither path is replaced with invented success.

All23,959 previously admitted owned definitions link for Linux64/Linux32/
Windows32 using64 fresh runtime objects and86 hash-verified reused game units
per target. Owned source/report and admission are unchanged. Full locked
release suites pass195 Linux/130 Windows tests with zero failures or ignored
tests. Generated Linux/Windows timer fixture bytes are identical. Portable and
Windows-link CI gates include the timer checks; remote CI was not run.

No ROM extraction/publication, player Python/Node/Wine, guest dependencies,
instruction metering, library mutation or relaxed package/safety limits is
introduced. Complete waits/events, device/clock IRQ delivery, all indirect-code
admission, full thread/FP ABI, rendering/audio/input, signing/launcher integration
and gameplay/platform fidelity remain open. Readiness remains false.

Private startup SHA256
`ac8f59fd4c6fab9c7e0f7e3f7a3de3c028c37ad15720977bbef63f85a922b0e1`;
glue `8a5038edaeab0a558d2fe0f201d4848587e049974ff9a6b8e78070a38c6677e3`;
virtual kernel `19759dc79cc2ecb5ac3b2120ae599b0e8ac1e4d12f5fa996e40395ecf9815660`;
generated fixture `8f46faba613ed7f3d099b7a88e1c8260de4176c175ee053abf79a6f8d230e84f`.
The kernel image's30-case protection/INIT fixture also passes all five
profiles. Strict all-target release lint and formatting pass.

### Reviewed jump-table recovery and original copy routine (2026-10-22)

The native generator now accepts reviewed, hash-bound static table profiles.
Its new `reference-controlled-dispatch-tables-source` command takes the usual
owned disc, private function inventory and private output, followed by a table
profile. The streaming writer applies identical profiles in both input passes,
retaining byte/same-PC conflict validation and rejecting changed input. Metadata
reads are bounded, unknown fields fail, and unused/wrong-range/wrong-hash
profiles cannot silently authorize other code. Existing commands retain their
original generation behavior. This remains an explicit maintainer interface;
player discovery/source-kit/signing delivery is still unfinished.

Tables are explicitly declared for exact canonical JMP [index*4+disp32] PCs.
Targets are read little-endian from the immutable raw-backed function span,
including declared wrapped negative indexes. Reachable table branches add
further CFG entries recursively. There is no guessed length or linear sweep
through table data. Empty/overflowing/out-of-span tables, undecodable targets,
targets/instructions overlapping table data, duplicates, unsupported encodings
and unreachable declarations fail. Review supplies table/index ranges; this
is not a proof of all possible runtime index values or dynamic code mutation.
Generated jumps still load current guest RAM and dispatch its exact target.

An asset-free fixture executes26 live-table/wrapped-index/missing-target/fault
cases per profile. It changes table values to prove jumps remain dynamic and
denies each table-read byte to check fault behavior, stack preservation and raw
pending x87/MMX/XMM preservation. Rust regressions cover recursive tables,
image/function hashes, metadata bounds, unused profiles, builder/stream identity
and changed second-pass input. Sanitized Linux64, actual Linux32, big-endian
MIPS32/QEMU and Windows32 GCC/mixed-Clang under Wine all pass. Portable and
Windows-link CI gates include this fixture; remote CI was not run.

The pinned NTSC-U copy routine at0x002442a0..0x002445dd uses16 reviewed table
branches. Its exact original-byte SHA256 is
`9e73092be40a49e349380a434bece6412b40256644691910273c1dd1e72e86fc`.
`profiles/ntsc-u-copy-tables.json` contains only bounds/hash/table metadata,
no executable or game assets. Recovery expands its53 original admitted PCs
to231, adding178 map entries:772,508 in the current private map. All previous
function bodies except this regenerated original routine remain byte-identical.
No host memcpy/memmove replacement is inserted into game execution.

The privately regenerated original routine passes39,168 memory/return/callee-
state/pending-FP cases per profile on all five targets. Cases cover every
length0..128 plus129/255/256/511/512/1023/1024, all source/destination alignments
mod4 and eighteen positive/negative/zero displacements, including overlapping
and zero-length copies. Every complete arena is compared with independent
host memmove. A private actual-i386 oracle executes the original instructions
from the verified image in a separate32-bit test process and matches every row.
This qualifies that routine/profile, not the whole game's ABI or hardware.
Original bytes and all derived C/executables remain private.

All23,959 admitted game definitions link on Linux64/Linux32/Windows32. Only the
changed body partition and map are freshly compiled per target;84 unchanged
game units,64 unchanged runtime objects and numerical pools are rehashed and
reused. They are not claimed freshly compiled. The CLI reproduces the exact
recovered function and rejects a mismatched hash without publishing output.
Full release suites pass201 Linux/135 Windows tests, none failed or ignored;
Linux/Windows generated fixture bytes match and strict all-target lint passes.

Actual strict startup now executes the original copy routine through its return
and reaches unresolved NtAllocateVirtualMemory184 at0x800000b8. Checked guest
arguments request base hint0, ZeroBits0, a1MiB MEM_RESERVE(0x2000) region and
PAGE_READWRITE(4), returning to0x001d5524. Reservation is not substituted with
immediate commitment or a constant-success allocation. Parent raw/coherent
state and priority0 remain verified. Real cold timer callback completion and
both unfinished shutdown/device paths remain verified separately; active
callbacks retain priority2 and terminate the diagnostic78 without parent resume.

Virtual reservation/commit/protection/free and physical backing, complete
thread/PCR/IRQ/SEH/FP ABI, remaining instruction/indirect admission, devices,
renderer/audio/input, source/cache/signing/launcher integration and whole-game
fidelity remain open. No player scripting/Wine dependency, guest crate policy
change, fuel/instruction quotas, ROM/media publication, user-library mutation
or relaxed limits is introduced. Readiness remains false.

After strengthening the captured/live fault-bank assertions, the targeted6
Linux/5 Windows tests and all five portable profiles pass again. Formatting
and both qualification-script syntax checks pass.
Private startup SHA256
`e9a9f22263ae28347be327715d2e605297c5f4e061549a0b7894e6ba11d296bf`;
glue `f1e881e36cf52a8d4125e1d817ecb25dbacff61d07c3b33a95d81b4d42f177e6`;
profile `1ab461f382f36c17f9bf3213523b8bfa7a262cc1881c3f4a7602983f81bcb650`;
recovered controlled body `d9aac06a55ff03ba68b6b1c617c7a339b22611a155fda9ec84948960a68f1932`;
final synthetic fixture `46abcd901f88e2fe46d4720ee037a012f83e538416b48a5912fafb5b49a06b3d`.

### Virtual pages and first actual NT allocations (2026-10-22)

The shared RAM resolver now has an optional page store with distinct reserved
and committed state, zeroed new backing, per-page protections, idempotent
recommit/decommit, partial-release splitting and coalesced state queries. Fixed
regions remain immutable. A configured virtual arena can exclude page-rounded
loader mappings, preventing reserve/commit/free from replacing them. Backing
capacity is independent of reserved address space; reserving1GiB with a one-page
physical profile succeeds without committing it, and excess commit fails.

All existing RAM services use the same locked resolver. No-access/uncommitted
pages fault nonpresent; readonly writes fault present. Whole-span admission
prevents late-page faults from partially writing. Staged host allocation and
split metadata commit only after success, with allocation-failure injection
covering reserve+commit, mixed-page commit and both partial-release remnants.
No guest/native pointer or executable-host-page mapping is introduced. Native
locks are released before guest faults. Mutations require an exclusive CPU
host boundary and cannot occur inside another service's admission/commit span.

The NT-style frontend supports basic Allocate/Free transitions,64KiB reservation
alignment,4KiB coverage, ZeroBits and top-down placement, bounded pointer spans,
ordered BaseAddress/RegionSize outputs, physical exhaustion and exact active
CPU/flat32 supervisor/PASSIVE0 ownership. Output rights are admitted before
mapping mutation. Pointees inside affected mappings stop unsupported rather
than being dereferenced after unmapping/protection. Recognized MEM_RESET,
MEM_NOZERO, guard/cache/COW facilities remain explicit unfinished host results;
no invented success substitutes for them. Execute bits are recorded, while
instruction-fetch permissions, dynamic code mutation, real PFNs/aliases,
retail edge-case rounding/failure precedence and complete VM query/handle ABI
remain separate work. This is a checked compatibility profile, not full retail
memory-manager parity.

The page fixture passes102,724 counted transitions/accesses per profile,
including an independent16,000-step flat-state oracle, user/supervisor fault
bits, no-access/readonly/recommit behavior, physical exhaustion, partial releases,
sparse fixed-map exclusions, late-store atomicity and injected allocation
failures. The NT fixture passes83 calls/generated transitions per profile,
including actual generated faulting loads/stores and repaired resumption,
readonly output faults, output aliases, affected-page aliases and priority
rejection. Pending logical x87/MMX/XMM state remains exact. Both fixtures pass
sanitized Linux64, actual Linux32, big-endian MIPS32 under QEMU and Windows32
GCC/mixed-Clang under Wine. Host/Windows fixture bytes match. Full locked release
suites pass205 Linux/137 Windows tests, zero failed or ignored. Strict all-target
lint passes. CI adds portable/Windows-link gates; remote CI was not run.

Actual owned startup now uses loader pages from all17 verified XBE sections,
combines rights for shared section pages, coalesces equal adjacent rights and
maps only the known processor/stack/kernel spans. The former broad unused-RAM
mapping is gone. The64MiB diagnostic machine profile charges7,933,952 fixed
page bytes, leaving59,174,912 bytes for committed virtual backing. An independent
page-union check reproduces that charge. Writes to readonly title code and an
unallocated former-flat-RAM address both fault. This does not qualify retail
PFN allocation, DMA aliases or complete thread/loader residency accounting.

Two stable traps are appended:184 NtAllocateVirtualMemory and199
NtFreeVirtualMemory. The kernel binds21 actual imported slots. The original
game reserves1MiB at0x00780000 with zero committed bytes, then commits its first
4KiB and continues through original heap initialization. It next stops at
unresolved NtOpenFile202, requesting the29-byte ANSI storage-root name
\Device\Harddisk0\partition1\, attributes0x40, access0x100001,
share3 and options0x800021. Checked object/name reads capture the complete name;
no host directory is opened or fake file handle returned. Parent raw/coherent
state and priority0 remain intact. Cold timer callback completion and both
unfinished priority2 shutdown/device branches are rechecked separately.

All23,959 admitted definitions still link for Linux64/Linux32/Windows32.
All86 game units and63 runtime objects per target are hash-verified reuse;
only memory and virtual_memory are freshly compiled, bringing the current
runtime set to65 objects. Current owned map/body/admission are unchanged.
The recovered original copy routine again passes39,168 cases per profile and
its actual-i386 original-instruction oracle against the changed resolver.
The expanded kernel's30-case protection/INIT fixture passes all five profiles;
LLVM and MinGW independently read the PE successfully.

ABI/behavior references include [nxdk declarations](https://github.com/XboxDev/nxdk/blob/master/lib/xboxkrnl/xboxkrnl.h)
and the pinned [Cxbx VMManager](https://github.com/Cxbx-Reloaded/Cxbx-Reloaded/blob/585c49a50af1255ab155099e06f24505f9c5a800/src/core/kernel/memory-manager/VMManager.cpp).
The latter's blob93bd7c3b55d3dee057d8c9590e11ef6395020fca and SHA256
40a7ae0c8d00f198f353c017ff7a8024ea22e1b91e8b7fa4cffe6e9306f2220e
are verified; reference code is not compiled into the runtime.

Private startup SHA256
`450f45fcc00d4b13f776d9d444bee1bf01ed3a1455902b6eb03e4bcc965d079c`;
glue `fb5406f3d1c8d0b20b19ea04f4e012ce383a5bec3dc252815403ddbd70bd0f6e`;
virtual kernel `07e3608fc7e01a340d6b4af7ef074a5a6ec28b1d21ec4391c3c3d4bf3c795c73`;
page fixture `56c01485dcd6464a61cb0e899195bf7b229736efd9a9629e4df1b2c40fa88d6e`;
NT fixture `e73b554d3a8e2d60c058d965a85635e29f6d5507a908cb358c216f9ee9785a3e`.

Scoped storage/file handles, complete VM flags/query/PFN/device behavior,
PS/thread/PCR/IRQ/SEH/FP ABI, remaining instruction/indirect admission,
rendering/audio/input, source/cache/signing/launcher delivery and complete
campaign/platform fidelity remain open. No player scripting/Wine dependency,
guest crate allowance, instruction metering, generic limit relaxation, media
publication or user-library mutation is introduced. Readiness remains false.

## Scoped directory I/O and shared handles

The next runtime increment adds `gamepkg_x86_handles.c/.h`,
`gamepkg_x86_io.c/.h` and the trusted Rust `io_backend` module. The kernel
appends NtOpenFile202, NtClose187 and NtQueryVolumeInformationFile218 without
moving earlier export traps. It now binds24 actual imported slots. These are
host services for the unfinished controlled backend; player source still cannot
import crates or supply native callbacks.

The per-CPU table maps nonzero aligned32-bit guest IDs to typed host objects,
never native pointers or file descriptors. IDs cycle over the full namespace,
skipping live objects, with no lifetime operation quota. Reservations precede
external opens; metadata commits cannot deliver a guest fault. Failed closes
retain their live object. Terminal drain closes objects while their backend
contexts remain alive, allowing dependent services to be released before the
empty table. All guest operations require the exact active CPU, supervisor RAM,
flat32 CPL0 and configured PASSIVE0 with a matching priority mirror.

The directory frontend reads counted Xbox ANSI strings and12-byte object
attributes through the guest memory adapter. It checks complete inputs and
outputs before allocating an object or calling external storage. Mounts are
explicit immutable NT prefixes; the longest component match wins. Relative
names resolve through a directory handle owned by the same service. Dot
traversal, malformed separators, control bytes and host path syntax fail.
Unsupported ANSI code pages, object attributes/options and absolute namespaces
stop explicitly. No ambient host fallback or successful placeholder exists.
The supported volume query serializes24 explicit little-endian bytes, with
64-bit capacity counts and declared sector geometry.

The Rust backend accepts an already-opened `cap_std::fs::Dir`. Guest text cannot
select an ambient host root. Each component is opened relative to that
capability with no-follow semantics; renamed roots remain pinned to the original
opened directory. Case-insensitive lookup rejects ambiguous host names. Access
and symmetric sharing are checked against actual opened-object identities;
metadata-only handles follow the independently tested Windows sharing rule.
Memory leases account for backend and per-handle storage. Native token IDs
support the full64-bit range and wrap around live IDs. Actual volume capacity
comes from `fstatvfs` on Unix or native volume queries on Windows, converted to
an explicit guest geometry. Locks and Rust guards are gone before control can
return to guest fault delivery. Pinned cap-std/cap-fs-ext4.0.3 and rustix1.1.5
are host dependencies; the guest dependency policy is unchanged.

The asset-free C protocol fixture passes147 calls/fault boundaries on sanitized
Linux64, actual Linux32, big-endian MIPS32 under QEMU and Windows32 GCC/mixed
Clang under Wine. Cases include every byte of open/volume outputs failing
before native side effects, output aliases, malformed names, relative/longest
mount resolution, failed close retention, terminal drain, pending reservations,
priority-mirror rejection and exact pending x87/MMX/XMM/GPR preservation. The
synthetic backend in this fixture tests the protocol and byte layout. Separate
Rust tests use actual filesystem capabilities, capacity queries, symlink and
case-collision attacks, renamed roots, a1,000-attempt component-swap race,
sharing/access rejection and full-width native tokens. Windows compares4,096
sharing combinations against native Windows APIs under Wine. This does not
claim physical Windows, full retail error precedence or whole-game big-endian
portability.

Full locked release suites pass212 Linux and142 Windows32 tests, zero failed or
ignored. Strict all-target lint and formatting pass. The30-case virtual-kernel
protection/INIT fixture passes all five profiles and both PE readers. CI adds
portable and Windows-link I/O gates; hosted CI was not run. Reproduce the
protocol matrix with `tools/check_takedawn_io.sh`, using the same compiler,
QEMU and Wine overrides as the other portability tools.

The private owned startup actually opens
`\Device\Harddisk0\partition1\` as guest handle4, queries real volume capacity
with FileFsSizeInformation3, closes the handle, and continues original code.
It then stops without a guest fault at unimplemented IoCreateSymbolicLink67.
The checked next request is `\??\D:` targeting `\Device\CdRom0`, with counted
lengths6/14 and capacities7/15. A virtual device/drive namespace and a read-only
VerifiedDisc-backed DVD filesystem remain required; host symbolic links are
not a substitute. The parent system-thread creation remains unresolved, with
the startup child still a diagnostic probe rather than complete thread
lifecycle integration.

All23,959 admitted owned definitions link for Linux64/Linux32/Windows32 with
the new services. The prior baseline links,86 game units,65 runtime objects
per target and numerical pools are rehashed and reused; handles and io are the
only two fresh runtime objects per target. No game body or dispatch map changed.
The actual startup exercises the Rust/C backend together on Linux64. Object
links and separate Windows backend tests do not establish a complete Windows
game startup.

Private startup SHA256
`bdf1e4d78395afc524e0c43baf32ff5fbda4372cee85de50b2fd4014e902b74d`;
glue `5e636adec41798104affc2668d948d2c67010a482ea4f2a7eb25d361c1cc5dd9`;
virtual kernel `a69d2736ffd1057483e6aa186d31a7ee7d8e2c92c9f0c9d1c4f92b5a494af3b9`;
protocol fixture `294f0012aa367028fe31c9e69d9ed8bded47d42efa8cf4301f222dcb039c39a2`.

Regular-file read/write/create/query, virtual DVD/drive mappings, complete NT
objects and VM/thread/device/IRQ/SEH behavior, remaining instructions,
rendering/audio/input and source-kit/cache/signing/launcher delivery remain
unfinished. No instruction metering, generic media-limit relaxation, player
Python/Wine requirement, owned-media publication or user-library mutation is
introduced. Readiness remains false.

## Virtual drive links and the next controlled-flow boundary

The I/O service now owns a process-local virtual object-name link table.
IoCreateSymbolicLink67 and IoDeleteSymbolicLink69 receive checked counted Xbox
ANSI strings and require the same exact CPU/PASSIVE0/supervisor-memory owner as
file operations. They do not create native filesystem symbolic links, select
ambient host roots or grant storage permissions. The two exports are appended
at stable traps0x800110c0/0x800110c8; earlier traps remain unchanged. Actual
owned imports now bind26 slots.

Create stages both complete descriptors/names before allocation or publication;
delete stages its complete name before removing anything. Duplicates and
existing virtual directories/mount objects collide case-insensitively. Root,
DOS-device and device namespaces are explicit, with intervening object
directories implied by mounted device paths. Missing parent paths fail, and
creation below a mounted filesystem/device or through another link remains an
explicit unsupported result. This is not the complete retail object-directory,
security, reference-counted link-handle or relative/codepage ABI.

Open resolves the first symbolic-link component, preserving the path suffix,
before choosing an explicit backend mount. OBJ_CASE_INSENSITIVE controls open
lookup; create/delete follow case-insensitive object-name matching. The ASCII
virtual DOS prefix `\??\` and drive colon are recognized without allowing them
into native host paths. Traversal/control/wildcard/invalid-separator inputs are
rejected. Relative/codepage/trailing-target and other-colon semantics remain
explicit unsupported families. Dangling and cyclic targets can be registered;
an absent backend still cannot produce a successful file handle.

Resolution uses exact-path Floyd cycle detection, with no fixed link count or
hop/instruction quota. An81-link chain resolves, and repeated use of the same
link can terminate when it consumes suffix components. Exact cycles stop with
an unfinished host result rather than inventing retail loop error semantics.
Expansion beyond the counted ANSI USHORT span returns NAME_TOO_LONG. Native
allocation failures return NO_MEMORY without publishing/removing a link or
opening a backend object. Temporary resolution buffers are freed before guest
memory outputs or native callbacks. No native mutex or Rust guard crosses a
guest fault. Existing handles remain valid after their lookup link is deleted.

The expanded asset-free protocol fixture passes434 calls/fault boundaries on
sanitized Linux64, Linux32, big-endian MIPS32/QEMU and Windows32 GCC/mixed Clang
under Wine. It includes every descriptor/name byte faulting before create
publication, readable alternate copies proving no publication, every link-name
byte faulting before delete with the entry retained, namespace/mount/parent
collisions, case-sensitive rejection, deletion/recreation with live handles,
long chains, terminating repeated links, exact and expanding cycles, malformed
counted names and allocation failures throughout resolution/create/delete.
Pending logical x87/MMX/XMM/GPR state is exact. The allocation-failure hook exists
only in maintainer test builds; player builds have no failure countdown or
execution metering. The final repository-integrated sanitizer test also passes.

Full locked suites pass212 Linux and142 Windows32 tests, zero failed/ignored.
The final namespace refinements are additionally checked by the integrated I/O
test and all five protocol profiles. Strict lint/format passes, and the updated
kernel passes30 INIT/protection/export cases on all five profiles with both PE
readers. Existing portable/Windows-link I/O gates cover the new fixture; remote
CI was not run. The trusted interface references include Microsoft's
[IoCreateSymbolicLink](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-iocreatesymboliclink)
and [IoDeleteSymbolicLink](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-iodeletesymboliclink)
contracts; the checked Xbox ANSI profile is not a claim of complete Windows or
retail Xbox object-manager equivalence.

Actual owned startup completes the prior storage open/volume/close sequence,
then successfully registers `\??\D:` targeting `\Device\CdRom0`. It continues
through original instructions and stops without a guest fault at0x00246696,
an unadmitted destination inside the original routine0x00246622..0x00246d89.
The prior unresolved parent system-thread request remains unresolved; this is
still a diagnostic child rather than complete thread lifecycle integration.

Private in-memory inspection verifies the1895-byte routine SHA256
`b31a5546ce7640e1946133e36240e3c22b0d487a2f3f824ddd22a33cfb3108c5`.
Its indirect branch at0x0024668f selects an eight-entry table at0x00246d89 after
an unsigned0..7 bound check. The actual index1 selects0x00246696. This table
has been identified but is not yet added to the reviewed table profile or
admitted into generated source. Continuing it requires that reviewed-table
extension and original-routine verification, not a dispatcher bypass or host
replacement. Disc file access remains unfinished despite successful drive-name
registration.

All23,959 admitted owned definitions still link on Linux64/Linux32/Windows32.
The earlier VM baseline links,86 game units,65 runtime objects and numerical
pools are rehashed unchanged; handles/io are compiled fresh per target. No
owned body or instruction map was regenerated for this namespace increment.
The prior qualified I/O startup/kernel/glue hashes are also rechecked unchanged.
Actual combined Rust/C owned startup is Linux64 evidence; separate Windows
backend tests/object links do not establish full Windows game startup.

Private startup SHA256
`7631b183b4a6bc68d135c428db65b2ccf283704dac44eb6d986ad0fbc221ef1a`;
glue `3b633f48759682cf5ae4317cf1b2c025b9d061d4b9db1ea059bcea5092b08205`;
virtual kernel `8d77065d84ded3e9d57c0c770fa06f9f9d4ad39f23f1efafc6b16c486111fc05`;
protocol fixture `9ba92dd6828762c54b09f70bf712cac351adbdcbf0536d801f6be97274ba98f2`.

The remaining reviewed control-flow/instruction coverage, read-only
VerifiedDisc-backed DVD filesystem, regular-file/complete NT object APIs,
thread/IRQ/SEH/VM/device integration, rendering/audio/input,
source-kit/cache/signing/launcher and campaign/platform fidelity remain open.
Readiness remains false. No user-library mutation, owned-media publication,
guest crates, player scripting/Wine dependency, instruction metering or generic
media-limit relaxation is introduced.

## Reviewed formatter dispatch table and TDATA creation boundary

The new `profiles/ntsc-u-format-tables.json` binds the original routine at
0x00246622 to its canonical indexed jump at0x0024668f. The jump follows an
unsigned0..7 check and reads eight little-endian entries at0x00246d89. The
original inventory's1895 code bytes end at that table; its32 data bytes finish
at0x00246da9, exactly where the next function begins. The reviewed1927-byte
code-and-table span is raw-backed executable-section input and does not overlap
the next function. No generic extent, BSS, overlap or target check is relaxed.

The original code-only SHA256 remains
`b31a5546ce7640e1946133e36240e3c22b0d487a2f3f824ddd22a33cfb3108c5`;
the full reviewed span is
`fff9a5d3b68068071ebfe9010237597845f0080a9a63c475ec140a7b92895208`.
All607 decoded instructions, including their original bytes, operands and
metadata, match the independent code-only linear decode and cover all1895
code bytes. The table bytes are excluded from executable instruction entries.
This verifies instruction identity/admission, not complete original-routine
behavior across every format, locale, floating-point or error case.

The controlled body now admits607 entries instead of48, retaining every earlier
entry. The current complete map grows from772,508 to773,067 entries, with no
cross-function entry collision and the same23,959 function definitions. Runtime
jumps still read the actual guest table and transfer only through the existing
checked dispatcher; there is no cached replacement switch, host formatter,
original-code patch or unreviewed indirect-call bypass.

The normal diagnostic CLI generates the same controlled body from the reviewed
span, reports one function/607 entries and rejects a deliberately wrong hash
before creating output. These explicit metadata inputs and the private function
replacement are maintainer work; automatic source-kit/build/launcher application
of all reviewed profiles is still required.

An asset-free trailing-table regression models the same bounded eight-way
shape with an unaligned table immediately after its code. It checks exact
code/data hash binding, all eight targets, truncated/oversized table declarations,
targets inside table data or beyond the span, and controlled generation. The
expanded generated-runtime fixture passes79 cases on sanitized Linux64,
Linux32, big-endian MIPS32/QEMU and Windows32 GCC/mixed Clang under Wine. Cases
include all eight branches, live guest-table changes, unsigned out-of-range
indexes that must not read even an inaccessible table, every byte faulting in
every entry, unresolved runtime targets and pending x87/MMX/XMM preservation.
Existing wrapped-index cases remain. The MIPS compiler now clears an inherited
host C_INCLUDE_PATH, matching the other portability tools; this changes no
player runtime behavior.

Full locked suites pass213 Linux and143 Windows32 tests, zero failed/ignored.
Strict all-target lint and formatting pass. Existing portable/Windows-link
jump-table CI gates cover the expanded fixture; hosted CI was not run. All
23,959 owned definitions link again for Linux64/Linux32/Windows32. Exactly two
game objects per target are freshly compiled: blocks-0060 and map. Their source
diffs are checked to replace only this one original body and its map entries.
The other84 game units, all67 current runtime objects and numerical pools are
rehashed and reused. Kernel/Rust startup input and the earlier qualified
namespace startup/kernel/glue remain unchanged by hash.

Actual owned startup completes real storage open/volume/close, registers its
virtual DVD drive, then executes past the formerly unadmitted formatter target.
It stops without a guest fault at unresolved NtCreateFile190 (0x800000be).
Checked inspection captures the complete34-byte name
`\Device\Harddisk0\partition1\TDATA`, capacity35, RootDirectory0,
attributes0x40, access0x100001, file attributes0x80, share3,
disposition3 (open-or-create), options0x4021 and no allocation-size pointer.
The capture preserves the complete guest context. No directory is pre-created,
creation result fabricated or successful file handle supplied for this request.
The unresolved parent system-thread request and diagnostic-child limitation
remain. Actual combined owned startup is Linux64 evidence, not a complete
Windows game startup or full original formatter oracle.

Private startup SHA256
`3408a114725203ee123403ad54a1ca214ae28d1f4bd80aeaef6baa76e71878d1`;
glue `a288b4306df89ac771bdc8fb77cffe9fa5974b8df352333f83d6833f7a9ad9a1`;
controlled routine `17938c201119d8285c50421b1708dfb938eeb7cb1d569d7849b844960a6be126`;
current map `c1ca2248c5dd96cb341e03712f3f90d6e1ea09fe82c71dbdc89b611aaf2d4c92`;
reviewed profile `979673db6898283438ae67a218a3f569f5a7f9bcf354829c71f55572e7aa22d6`;
asset-free C fixture `adfaab5fa7cfcb66350e61ff2cc88aeb23fe6ba43ba93fdefd9dd3c452ce0c56`.

Directory creation/complete NtCreateFile semantics, regular files and the
read-only VerifiedDisc-backed DVD filesystem remain required, along with full
original-routine behavior, remaining instructions/control flow, complete
thread/VM/device/IRQ/SEH integration, rendering/audio/input,
source-kit/cache/signing/launcher and campaign/platform fidelity. Readiness
remains false. No player scripting/Wine dependency, guest crate allowance,
instruction metering, generic limit relaxation, user-library mutation or
owned-media publication is introduced.

## Checked directory creation and title-save path initialization

NtCreateFile190 is now appended at stable trap0x800110d0, preserving earlier
exports and binding27 actual imported slots. Its Xbox nine-argument frontend
shares open's checked names, virtual drive resolution, mount selection,
relative handles, whole-output admission and typed handle reservations. A
nonzero allocation-size pointer is read as8 explicit little-endian bytes before
allocation or native work. Handle/IOSB outputs are preflighted before any mkdir;
only a successful native result commits a guest handle. Success reports the
backend's FILE_OPENED1 or FILE_CREATED2 information rather than a constant.

The trusted, unpublished backend ABI appends an optional create callback:
five native pointers in total, nine logical host request words and five output
words. Existing callback argument/word layouts remain fixed; all providers and
consumers using the new descriptor are rebuilt together. A missing callback
stops unsupported without leaking a reservation or inventing a file handle.
No native pointer, file descriptor or callback is supplied by the guest.

The real directory backend supports FILE_OPEN1, FILE_CREATE2 and FILE_OPEN_IF3
with FILE_DIRECTORY_FILE, default/normal attributes and absent/zero allocation.
Other file/attribute/allocation/option families remain explicit unfinished
results. FILE_OPEN_FOR_BACKUP_INTENT is recognized but grants no privilege
beyond the configured capability and host permissions. Read-only roots can
reopen an existing readable directory but cannot create a missing one.

Directory creation uses the already-opened root/parent capability, validates
all FATX-style components, opens parents and the leaf without following native
symlinks, and retains real opened-object identities. A per-context native mutex
serializes case-insensitive creation and sharing publication. Concurrent
open-or-create requests produce one actual directory, one CREATED result and
OPENED results thereafter. Resource leases are charged before mkdir. Locks and
Rust guards are gone before returning to fault-capable guest code. Failed native
opens allocate no backend token; no unsafe path-based rollback deletes entries
that an unrelated host actor could have replaced. Atomic creation/returned
identity against every unrelated host filesystem race, full retail failure
precedence/IOSB contents, security/backup privileges and complete attributes
remain separate qualification. This is not full NtCreateFile parity.

The asset-free C fixture now passes499 calls/fault boundaries on sanitized
Linux64, Linux32, big-endian MIPS32/QEMU and Windows32 GCC/mixed Clang under
Wine. New cases cover creation information, relative parents, optional-provider
rejection, full-width allocation words, wrap rejection, every output byte
faulting before native side effects and every optional allocation byte faulting
before a reservation. Pending x87/MMX/XMM/GPR state remains exact. Synthetic
backend acceptance of nonzero allocation bits tests transport only; the real
directory backend explicitly stops unsupported for nonzero allocation.

Separate actual-filesystem tests cover create/open/reopen/collision, relative
child directories, read-only policy, budget exhaustion before side effects,
sharing, malformed names, symlink parents/leaves and eight simultaneous
case-insensitive requests. Renamed roots remain pinned for creation. A1,000
parent symlink-swap creation race checks successful objects against the original
parent's identity and confirms zero outside writes; the earlier1,000-attempt
open race remains. Native Windows APIs under Wine match24 disposition,
normal/default-attribute and absent/zero-allocation cases: success status and
information, failure status, existence and handle closure. Failure IOSB layout
is not claimed. The existing4,096-case Windows sharing oracle also passes.

Full locked suites pass218 Linux and147 Windows32 tests, zero failed/ignored.
Strict all-target lint, Rust/C formatting and the expanded30-case kernel image
matrix on all five profiles pass. Both PE readers accept the image. Existing
portable/Windows-link I/O gates cover the expanded fixture; hosted CI was not
run. All23,959 owned definitions link on Linux64/Linux32/Windows32 with all86
game units and66 runtime objects rehashed/reused per target; only io is freshly
compiled. The773,067-entry owned map and reviewed original bodies are unchanged.
The prior qualified formatter startup/kernel/glue remain hash-identical.

Actual owned startup is checked against fresh and existing private storage.
Fresh storage creates the real TDATA directory and reports guest handle8 with
FILE_CREATED2; existing storage reports FILE_OPENED1. Original guest code closes
the new handle successfully and continues, with no guest fault, to unresolved
RtlInitAnsiString289 at0x80000121. Both runs confirm zero remaining native
backend handles after terminal teardown. The complete next terminated source
name is `\Device\Harddisk0\partition1\TDATA\4541005b`; checked inspection
preserves the complete guest context. The title-specific child directory is
not pre-created. Parent system-thread creation remains unresolved and this
continues to be a diagnostic child, not complete thread lifecycle integration.
Actual combined startup evidence is Linux64; Windows backend tests and object
links do not prove a complete Windows game startup.

The trusted interface references Microsoft's
[NtCreateFile contract](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/nf-ntifs-ntcreatefile).
This checked Xbox ANSI profile and limited Windows oracle are not complete
retail Xbox or Windows filesystem equivalence.

Private startup SHA256
`0076300187cc3e1e2e526de72a37c9665509ae222971ae039afe90e1033553e1`;
glue `89c567b5b88d4d02c6bf172b35b6ff3de75f52151cf56eca713d882111f7086b`;
Rust bootstrap `4db344a68487f9812e10c96e8d834b2e4ff5340b5f7bcf4e4862b870a96c87aa`;
virtual kernel `91302ee2cbe96f00316b8dba51793a51c4f2119eac1e56c398f9b3255667a60e`;
C fixture `d75652134dbc441fe6e626d9952447fcf97dfb8b5ff1af53980e075c45e7477b`.

The string helper, further directory/file operations, read-only
VerifiedDisc-backed DVD filesystem, complete objects/thread/VM/device/IRQ/SEH,
remaining instruction/control-flow/original-routine fidelity,
rendering/audio/input/source-kit/cache/signing/launcher and campaign/platform
fidelity remain unfinished. Readiness remains false. No user-library mutation,
owned-media publication, guest crate allowance, player scripting/Wine
requirement, instruction metering or generic media-limit relaxation is added.

## Checked legacy ANSI initialization and title-folder startup

`gamepkg_x86_rtl_string.c/.h` adds the checked RtlInitAnsiString289 helper;
the kernel appends its trap at0x800110d8 and binds28 actual imported slots.
It borrows the exact active CPU/supervisor RAM, requires flat32 CPL0 and a
configured priority mirror, and accepts resident memory at configured priority
levels, including the tested1/2/31. It does not retire guest instructions,
change priority/IF, allocate, alter GPR/FP state or carry a native lock across a
fault. The import driver separately performs its checked stdcall2 return.

The helper scans the complete NUL-terminated source through guest memory reads,
then admits and writes the complete8-byte Xbox descriptor in explicit LE
fields. Buffer retains the guest32 source address. NULL source clears all
fields; non-NULL empty source retains its pointer and has maximum1. Input/output
aliases are scanned before writes. It reads no byte beyond the terminator,
never partially writes on a late destination fault, and imposes no artificial
scan/instruction quota. Native partial-fault/write ordering remains unqualified.

Length behavior is an explicit legacy16 compatibility profile: full scan,
then16-bit length/maximum wrap. Native Wine9 RtlInitAnsiString matches that
behavior, as does the earlier prototype's casts. Microsoft's current
[RtlInitAnsiString documentation](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/content/wdm/nf-wdm-rtlinitansistring)
describes saturation for oversized strings. The helper does not claim that
modern profile, physical Windows equivalence or original retail-Xbox behavior
for oversized strings. The actual captured game strings fit below that boundary.
The Wine oracle is explicitly enabled only for that maintainer profile; player
code never calls host RtlInitAnsiString or requires Wine.

The expanded I/O fixture passes535 calls/fault boundaries on sanitized Linux64,
Linux32, big-endian MIPS32/QEMU and Windows32 GCC/mixed Clang under Wine.
It tests lengths0/1/43/255/65533/65534/65535/65536/100000, null, aliases,
all eight destination-byte faults, inaccessible source bytes, a terminator
immediately before inaccessible memory, wrapped destination rejection and
priority1/2/31. Pending logical x87/MMX/XMM and GPR state remains exact. Both
Windows compiler profiles additionally compare nine non-null lengths and null
against native Wine RtlInitAnsiString, including borrowed pointer identity.
The final integrated sanitizer test passes after explicit oracle scoping.

Full locked suites pass218 Linux and147 Windows32 tests, zero failed/ignored.
Strict lint/format and the expanded30-case kernel-image matrix on all five
profiles pass, with both PE readers. Existing portable/Windows-link I/O gates
include the new runtime object and expanded fixture; hosted CI was not run.
All23,959 owned definitions still link on Linux64/Linux32/Windows32. All86 game
units,67 earlier runtime objects and numerical pools are rehashed/reused; only
rtl_string is freshly compiled. The773,067-entry map and original bodies remain
unchanged, as do the prior qualified directory startup/glue/kernel and current
Rust bootstrap. No player package or ready launcher is produced by this step.

Fresh and existing private-storage runs each complete four ANSI initializations
and four real directory operations. The resulting TDATA/4541005b and
UDATA/4541005b folders are created on a fresh root and reopened on an existing
root. D: and T: virtual mappings register successfully. Native backend handle
count is zero at teardown. Original code continues without a guest fault and
stops at unresolved RtlEqualString279 (0x80000117), comparing counted strings
`$$XTINFO` (length8/capacity9) and `.text` (length5/capacity6), with case-insensitive
argument1. Checked inspection preserves the complete guest context. The parent
system-thread request still remains unresolved; combined owned startup is
Linux64 diagnostic-child evidence, not complete thread or Windows game startup.

Private startup SHA256
`b03351d7102024c3e6ccf4c2b92c828b715f1cfb2745eb8e09a7a18cbed5d329`;
glue `d9d004f640ae46b45f4e823033a27b5af29aa2d138d0bea08926010ce9ed5627`;
virtual kernel `e133c7c4552c0fb352d9f6ed104b163aad3e49f5d429a48433e873ce2437a25e`;
fixture `276dcaed6f0a09384fdafb4a6078b8d8c4d2e2f94611b451461a1a1edc992474`.

Counted-string comparison, full file/DVD operations, complete thread/VM/device/
IRQ/SEH and original instruction/routine fidelity, rendering/audio/input,
source-kit/cache/signing/launcher and campaign/platform fidelity remain open.
Readiness remains false. No user-library mutation, media publication, guest
crates, player scripting/Wine dependency, instruction metering or generic
media-limit relaxation is introduced.

## Counted ANSI equality and the title-metadata file boundary

The shared RTL memory service now implements RtlEqualString279, appended at
stable trap0x800110e0; actual imported bindings grow to29. It uses the same
exact-owner/supervisor-memory/flat32/priority-mirror rules as initialization.
It compares counted bytes, including embedded NUL, and ignores MaximumLength.
Only the two length fields are read first. Different lengths return false and
empty equal lengths return true without reading buffer fields or data. For
nonempty equal lengths it reads the two guest32 pointers, then stops at the
first differing byte. There is no allocation, guest write, instruction quota,
priority change or native lock across a guest fault.

The explicit byte-case profile folds ASCII a..z and preserves all other bytes.
Only the low8 bits of the BOOLEAN argument select case mode. All256x256 byte
pairs in both modes match native Wine9 RtlEqualString, including its52
nonidentity ASCII matches. This is a measured compatibility profile, not full
retail locale/NLS, fault-precedence or volatile-register parity. The import
adapter returns canonical BOOLEAN in AL, preserves upper EAX bits and performs
a separately checked stdcall3 return.

The asset-free fixture now passes587 calls/fault boundaries and131,072
byte-pair comparisons per profile on sanitized Linux64, Linux32, big-endian
MIPS32/QEMU and Windows32 GCC/mixed Clang under Wine. Each Windows profile
also checks the complete byte matrix against the explicitly scoped native
oracle. Additional cases cover embedded NUL in both modes, maximum65535-byte
lengths, a last-byte difference, Boolean255/256, ignored/inaccessible maximum
fields, length mismatch and zero lengths with inaccessible buffer fields,
early mismatch before inaccessible bytes, and every required descriptor/data
byte fault. Existing initialization, directory, namespace and I/O cases remain.
Pending logical x87/MMX/XMM/GPR state remains exact. The final integrated
sanitizer test also passes.

Full locked suites pass218 Linux and147 Windows32 tests, zero failed/ignored.
Strict lint/format and the29-export kernel's30-case protection/INIT fixture pass
on all five profiles, with both PE readers. Existing I/O portability/Windows-link
CI gates cover the expanded fixture; hosted CI was not run. All23,959 owned
functions still link on Linux64/Linux32/Windows32. All86 game units,67 earlier
runtime objects and numerical pools are rehashed/reused; only rtl_string is
freshly compiled. The773,067-entry map, original bodies and Rust bootstrap are
unchanged. Earlier qualified ANSI startup/glue/kernel remain unchanged by hash.

Fresh and existing private-storage startup runs complete54 string initializers
and48 equality calls, including two true comparisons. Four directory operations
create/open TDATA/UDATA and their4541005b children as expected. No guest fault
occurs, and terminal native handle count is zero. Execution reaches the resolved
NtCreateFile190 trap with an unfinished regular-file request for the complete
57-byte name `\Device\Harddisk0\partition1\UDATA\4541005b\TitleMeta.xbx`
(capacity58). The request asks for access0x40100000, SYSTEM attribute4,
share-read1, OPEN_IF3, options0x22 (write-through plus synchronous nonalert),
RootDirectory0, case-insensitive object attributes0x40 and no allocation pointer.
Checked capture preserves the complete guest context.

The directory-only provider stops explicitly unsupported for that file request;
it returns no file handle and creates no placeholder TitleMeta.xbx. Regular
file creation, persistent attribute behavior and write-through semantics are
therefore concrete next requirements. Parent system-thread creation remains
unresolved, and combined actual startup is Linux64 diagnostic-child evidence,
not complete thread, Windows game startup or gameplay evidence.

Private startup SHA256
`39be8aa1ce8e31646c048287b7bec9ff7fdbb6fef1e43be40155b2762c14f98a`;
glue `080f5b121bca8e23f37064b79a26415cf61294294adefc23259102034e4604a8`;
virtual kernel `ef270daa83e3de50980ba36874577bc1b7ef2d89c80af0deb78b6434e2f4997a`;
fixture `8cea8764fce4928ab66fea7fa01cfe029754d7778251741480daa0aea1699899`.

Regular file/DVD operations, complete object/thread/VM/device/IRQ/SEH behavior,
remaining instruction/original-routine fidelity, rendering/audio/input,
source-kit/cache/signing/launcher and campaign/platform fidelity remain open.
Readiness remains false. No user-library mutation, owned-media publication,
guest crates, player scripting/Wine dependency, instruction metering or generic
media-limit relaxation is introduced.

## Regular metadata-file creation and the next information query

The capability backend now retains typed directory and regular-file objects.
Synchronous regular OPEN/CREATE/OPEN_IF requests support absent/zero allocation
and default, normal or SYSTEM creation attributes. The frontend forwards
WRITE_THROUGH for creation; unsupported options still stop explicitly. Regular
file handles cannot serve as relative directory roots. Automatic directory
selection without an explicit object-kind option, overwrite/supersede,
nonzero allocation, asynchronous requests and complete retail failure precedence
remain unqualified.

On Linux/Android, a new file starts as an unnamed O_TMPFILE in the pinned parent.
Its virtual FATX flags are stored as four little-endian bytes in the inode's
`user.gamepkg.fatx.attributes.v1` attribute before publishing its name. No
sidecar is visible to the game. The live descriptor is linked without replacing
an existing destination; failed initialization leaves no named placeholder.
Write-through uses O_SYNC, file synchronization and parent synchronization.
Host filesystems lacking the required primitives remain explicitly unsupported.
On Windows the backend uses native capability-relative NtCreateFile with native
attributes, write-through and no-follow flags. This adds no player Wine dependency.
A durability failure after publication may leave the actual file present; there
is no unsafe unlink rollback against an externally replaceable name.

Existing opens never truncate. The actual opened inode must be a regular file
with one link; symlinks, reparse points, special files and hard links are denied.
Access, sharing, readonly policy and per-handle memory accounting remain enforced.
No Rust guard survives a callback into fault-capable guest execution. This is
containment for the reviewed guest interface, not complete parity against an
unrelated native process racing filesystem mutations.

The full release suites passed221 Linux and149 Windows tests, with no failures
or ignored tests. Final backend refinements then passed all14 backend tests on
Linux64 and actual Linux32, and all11 in the local Windows32 Wine environment.
Strict Linux and Windows all-target lint and formatting pass. The final I/O
fixture passes617 calls/fault boundaries plus131072 counted-byte comparisons in
each of five existing profiles: sanitized Linux64, Linux32, MIPS32 big-endian,
and GCC/mixed-Clang Windows32. The regular-file transport checks cover the
actual access0x40100000, SYSTEM4, share-read1 and options0x22 request, typed
handles, creation/open information, unsupported flags and every output-byte
fault before native work. Pending x87/MMX/XMM and general registers stay intact.
The final integrated sanitizer test passes as well; hosted CI was not run.

All23959 original definitions still link on Linux64/Linux32/Windows32. The86
game units,67 runtime objects and numerical pools are hash-verified reuse; only
I/O is freshly compiled in each target. The773067-entry map, game bodies, Rust
bootstrap and kernel image stay unchanged. Prior ANSI startup/glue/kernel proof
artifacts retain their hashes.

The same final Linux64 startup binary was run with fresh and existing private
storage. It completes54 initializers and48 string comparisons, then all five
real create/open operations: four directories and `TitleMeta.xbx`, with guest
handles8/12/16/20/24. Fresh operations report CREATED2; existing operations
report OPENED1. Both files retain SYSTEM|ARCHIVE0x24 on the actual inode, and
terminal native handle count is zero. The file is correctly empty at this point:
no write service has run yet. No disc contents were extracted or user library
changed.

Execution next reaches unresolved ordinal211, NtQueryInformationFile, with
handle24, a56-byte output and class34 (FileNetworkOpenInformation). Checked
argument capture preserves the full guest context. No metadata record or query
success is fabricated, and the guest is not resumed past that missing service.
Parent system-thread creation255 remains unresolved; this is diagnostic-child
startup evidence, not full thread lifecycle or Windows gameplay evidence.

Full read/write/query/set/delete/disposition/async/DVD semantics, remaining
threads/VM/devices/IRQ/SEH/instructions, rendering/audio/input, source-kit/cache/
signing/launcher delivery and campaign/platform fidelity remain open. Playable
and production-bootstrap readiness remain false.

Private final startup SHA256 `ae2ac5df696a5fd51425ecd6cc58d02f043a0ecb71675394a71b47981a5e53f8`;
glue `fdf2632cc47194b23fb108329905bc3d27367e8e67e5fb475d141f8a5d25e29c`;
fixture `164f12bdd62e1df6f47100bb8782e3d573385d9250492e6d4686f0c6555efde8`;
backend `4da5f0258b83e1e88ed45cab5b33dd6d5fda61fa0c6cff9d9547c7c441c71435`.

## File-network information and the next original-code branch

NtQueryInformationFile211 is now appended at trap800110e8, retaining all earlier
trap PCs and both reserved callback-return guards. The native backend descriptor
has six pointers, adding an optional query callback. Its14 logical host words
carry status, four64-bit times, allocation/EOF sizes and attributes; no native
padding or guest pointers cross the callback boundary. All consumers were
rebuilt with that layout. Other information classes remain explicitly unfinished.

Class34 requires a56-byte result buffer. The frontend preflights all56 bytes and
all8 IOSB bytes before querying the native object. It writes52 defined bytes in
explicit little-endian order, preserves trailing padding and excess buffer
bytes, then writes IOSB status/information56. Short buffers do not invoke the
backend. Closed/foreign handles, output wrapping, missing providers, backend
failures and invalid signed sizes have checked failure paths. Defined output
aliasing follows payload-first, IOSB-last order. Complete retail error precedence,
async behavior and arbitrary native-provider concurrency remain unqualified.

Linux/Android metadata reads actual opened-inode creation/access/write/change
times, EOF and persistent FATX attributes. Creation time is not replaced by
ctime when unavailable. Dates convert to NT100ns units with checked epoch,
fraction and representability handling. Missing creation-time support or malformed
virtual attributes stops explicitly. Android execution remains unqualified.
Windows reads actual NtQueryInformationFile metadata from its native handle;
internal file opens include the metadata-read right, while the guest's logical
data-read permissions stay unchanged.

Allocation size is rounded from the real logical EOF to the explicitly configured
virtual cluster geometry. It is a virtual allocation figure, not a claim about
host sparse-file block consumption. Directory EOF/allocation are zero; host
storage occupied by directory entries is not exposed as a guest byte stream.
Native Windows compatibility measurements confirm those directory fields and
that56 is the record's minimum/result length with padding untouched. The
standalone file/directory probes cover42 native calls over three access masks
and lengths51..57. They are Wine9 measurements, not physical Windows or retail
Xbox error/fault-parity proof.

The [Microsoft record definition](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/ns-wdm-_file_network_open_information)
describes the fields, epoch and current Windows metadata-read requirement.
This supervisor-kernel compatibility profile permits metadata queries on an
already opened object, including the actual startup's write-only data handle.
The [pinned Cxbx kernel reference](https://github.com/Cxbx-Reloaded/Cxbx-Reloaded/blob/585c49a50af1255ab155099e06f24505f9c5a800/src/core/kernel/exports/EmuKrnlNt.cpp)
was inspected for object lookup context, not copied or used as retail proof.
No successful-looking record substitutes for an unavailable native query.

All223 Linux and150 Windows release tests pass, with no failures or ignored
tests. All16 backend tests also pass in an actual Linux32 process; Windows32
passes12 backend tests. The final I/O fixture passes952 calls/fault boundaries
plus131072 byte-pair comparisons in each of the five established profiles.
It covers every IOSB/result/padding byte fault before native work, lengths0..64,
closed handles, unsupported classes/providers, aliases, backend errors and
malformed sizes. Pending x87/MMX/XMM and general registers remain intact.
Metadata tests cover actual dates/attributes/EOF, cluster boundaries, malformed
inode attributes, negative Unix dates and sub100ns rounding. Strict Linux and
Windows lint and formatting pass. Kernel export/INIT tests and both independent
PE readers accept211 at10e8. Existing CI gates consume these tests; hosted CI
was not run.

All23959 original definitions still link on Linux64/Linux32/Windows32. All86
game units,67 earlier runtime objects and numerical pools are hash-verified
reuse; only I/O is freshly compiled per target. The773067-entry game map, game
bodies and Rust bootstrap stay unchanged. Earlier regular-file proof artifacts
retain their hashes.

Fresh and existing private storage runs of the same final Linux64 binary complete
all five create/open operations, then class34 for handle24. The56-byte guest
record has actual timestamps independently checked against the native file,
SYSTEM|ARCHIVE0x24, and zero EOF/allocation for the newly created empty file.
There is no data write yet. Both runs close every native handle at terminal
teardown and continue without a guest fault to original PC002487fa.

That target lies inside0024877e..00248f22. Its existing map contains51 entries;
an indirect branch at002487f3 selects an eight-entry trailing table at00248f22.
Private inspection covers all1956 code bytes and603 instructions, with each
reviewed table target on an original instruction boundary. Hash-checked private
recovery matches an independent code-only linear decode and excludes the32
trailing data bytes. Both ordinary and controlled C generation succeed. Those
new bodies are not linked or admitted in this qualification; the live map still
stops at002487fa. Table review, regression qualification and actual admission are
the next original-code work.

Parent system-thread creation255 remains unresolved, so startup evidence is
still a diagnostic child rather than complete thread lifecycle or gameplay.
Complete files/DVD/objects/threads/VM/IRQ/SEH/devices, remaining instructions and
original-routine fidelity, rendering/audio/input, source-kit/cache/signing/
launcher and campaign/platform fidelity remain open. Playable and production
bootstrap readiness remain false. No user-library writes, media extraction or
publication, player scripting/Wine dependency, guest crates, instruction metering
or generic-limit relaxation is introduced.

Private metadata startup SHA256 `623f935867f60e67ec5c2787e0f61c71cf46b8eed912f3640f7cc98281e567a5`;
glue `83c896a4e24057f3682f1bd7ea14714e42df054d93683b3333458d7aced49de6`;
kernel `bd68334b08eef3f83b505a8badd2f61dc53e1d391f9855de78660052240b95d6`;
fixture `9227da40e0dfe673e36f062505c0519f6fe86a7ebeceee547e18ae8826717b21`;
backend `45b96804708c67762060b87093c61ad72513930dee53708116639f3a763108f1`.

## Original wide-format table admission and the first metadata write

`profiles/ntsc-u-wide-format-tables.json` reviews the complete0024877e..00248f42
raw-backed span. The1956 code bytes end at00248f22; exactly32 trailing bytes
hold eight little-endian targets for JMP002487f3. The original unsigned0..7
guard remains original code. The complete code/table SHA256 is
`1f57389bdaf4c48b4c13596f5418ba48c2a3e0137705353f375e1cc375fc9c59`;
code-only SHA256 is
`786320b9a291ffa514f81a9e199cb9bec26b4750f0bca7b8acaadb676698265b`.
Every target is an original instruction boundary outside the table data.

The reviewed decoder recovers603 instructions, matching the independent
code-only linear decode in every instruction field and covering every code
byte. Ordinary and controlled emission both succeed. The freshly verified normal
CLI produces the identical controlled body; a mismatched code/table hash fails
before creating output. No generic extent, raw backing, overlap or indirect-call
rule changes. Runtime jumps still read the actual guest table and check their
exact destination against the closed map. There is no host formatter replacement,
skipped original body or executable table data.

The controlled map replaces51 existing entries for this routine with603,
retaining every previously admitted entry and rejecting cross-function overlaps.
Its total grows from773067 to773619 entries. Only blocks-0061 and the global map
are freshly compiled per target;84 game units, all68 runtime objects and numerical
pools are rehashed and reused. All23959 original definitions link on Linux64,
Linux32 and Windows32. Kernel image and Rust bootstrap stay unchanged. Earlier
metadata-query startup/glue/kernel proof artifacts retain their hashes.

All seven existing jump-table tests pass. The existing trailing-eight-entry
fixture passes79 cases in each of five profiles, including live table changes,
wrapped/out-of-range indexes, missing targets, each table-byte fault and pending
x87/MMX/XMM preservation. These existing generic checks are reused rather than
replaced by a profile-shaped successful stub. The223 Linux/150 Windows full-suite
evidence from the unchanged core/runtime baseline remains recorded; it was not
rerun for this metadata-only addition. This does not establish every original
format grammar, numerical case, error edge or performance characteristic.

Fresh and existing private storage runs of the same final Linux64 binary now
pass the former002487fa stop, retain the successful metadata query, and reach
unresolved236 NtWriteFile at800000ec with no guest fault. The call uses file
handle24, IOSB03fcfe68, a44-byte buffer, null event/APC/APC-context and null byte
offset. Checked buffer/argument capture preserves the complete guest context.
The buffer contains the expected UTF16LE BOM and title-name metadata line,
confirming this actual formatting path completed. Complete formatter semantics
and physical Windows gameplay remain unqualified.

The write is not fabricated or resumed. The real file remains empty, and all
native handles close at terminal teardown. Next work is a general synchronous
transfer path with correct access, current/explicit offsets, actual bytes and
errors, budgeted buffer staging and fault cleanup; the44-byte request must not
become an arbitrary special-case transfer limit. Read/write/seek/query/set/delete/
async/disposition/DVD behavior, complete parent thread/object/VM/IRQ/SEH/device
integration, remaining instruction/original-routine fidelity, rendering/audio/
input, source-kit/cache/signing/launcher and campaign/platform fidelity remain
open. Parent255 is still unresolved; these are diagnostic-child startup results.
Playable and production-bootstrap readiness remain false. No media extraction,
publication, user-library writes, player scripting/Wine dependency, guest crates,
instruction metering or generic-limit relaxation is introduced.

Private wide-format startup SHA256 `42d6ca891715b795a853d8b8ddcbf1b76aac88a3f2ced853e5cdb8dca1a60fe0`;
glue `0ba13e3e32dab37cad5d5a82969e1a2743239e6ce1d2079c3559cc62ee0a3404`;
map `991d7e34df08832852bc255be29fd01224eeabdcdce63596a36dc7c3edee04f2`;
controlled body `cf8f56b511f4b4fe2d49fe70ca948e2fccea23882db770278229dec690c76c37`.
