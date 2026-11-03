# Guest control state and fault boundary

`gamepkg_x86_fault.c` is reviewed host runtime code, not a guest SDK. Link it
alongside generated native C and the pinned runtime's thread-local registers.
The generator's DIV/IDIV instructions call its non-returning divide-error
entry with the guest instruction address and the current local EBP. SSE1
comparison/control emission additionally links `gamepkg_x86_sse.c`.

Enter generated C through `gamepkg_x86_run(entry, &fault)`. A return of0 means
normal completion,1 means a guest fault, and-1 means invalid host arguments.
On a fault, the record contains the exception vector, faulting guest EIP, all
eight GPRs, virtual flags, FS base, direction and guest MXCSR. Vectors currently
transported are0 (divide error),12 (stack segment),13 (general protection),14 (checked RAM page
fault),16 (deferred x87 condition) and19 (unmasked SIMD condition). The current
local EBP is also published to the pinned register bridge. Results from the
faulting division are not committed. Earlier guest instructions remain
executed, including a caller's guest return-slot write. Native frames are
abandoned to the nearest live host boundary, not returned through normally.

The host must handle return1 as an exception; it must not treat the generated
entry as successfully completed. The boundary itself does not walk guest SEH,
select a handler or resume/retry guest code. Those full-game recovery and ABI
services remain required. This component does not emulate general memory/FP
exceptions. It is not a substitute for
the final game-process, time, memory or permission controls.

For floating-register capture, include `gamepkg_x86_pinned_context.h` after the
reviewed runtime's `recomp_types.h` and call
`gamepkg_x86_run_pinned(entry, &fp_fault)`. The adapter uses the actual typed TLS
XMM lane arrays and the shared raw x87/MMX state on the current worker. The
existing 64-byte core record stays unchanged. `gamepkg_x86_run_with_fp` also
accepts explicit trusted host register bindings; it copies the binding descriptor
before entry and captures live register values when a fault is delivered.
Capture retains all eight physical raw80 values, tags, TOP/status/control,
instruction/data pointers, opcode/selectors and all 32 raw XMM lanes. No float
conversion, MMX shadow file, allocation or host padding is used in the snapshot.
Its layout is a host format, not a guest FXSAVE image or portable serialization.
Invalid/null/misaligned/overflowing or overlapping register/output spans are
rejected without entering guest code or changing output. The host must still
provide readable, live storage; pointer validation cannot establish ownership.
Normal completion clears the result; a fault records already committed state.
`gamepkg_x86_restore_pinned(&fp_fault)` now loads the recorded GPRs, eager
virtual EFLAGS, direction, FS base, MXCSR and raw x87/MMX/XMM registers on the
current worker. `gamepkg_x86_restore_fault_registers` accepts explicit writable
host bindings. It validates the entire input/target layout, reserved MXCSR bits
and boolean direction before any mutation. All register banks must be distinct
from the input, descriptor and one another. Host padding is zeroed. The input
record remains unchanged. These are trusted host snapshots, not guest
SETCONTEXT or FXSAVE images; the host still supplies live writable storage.

Restoration performs no numerical interpretation, tag reclassification or
pending-fault delivery. A restored pending x87 exception is delivered by the
next waiting instruction; old unmasked SSE status alone does not raise #XM.
This restores register values only: local C EBP/native call frames, RAM,
segment/device bindings and instruction dispatch are not restored. EIP stays
in the input record and must be checked separately against an executable entry
map. Never cast it to a host pointer. The legacy lazy-flag ABI still needs its
bridge before mixing engines. Guest SEH/resumption and remaining CPU state,
worker ABI and player-process integration remain required.

Frames are thread-local, nested and allocated on the native C stack, with no
heap allocation. `setjmp` remains in the live entry wrapper. The jump frame's
fields are initialized before `setjmp` and never modified afterward; the
externally owned result receives the fault record. A missing boundary aborts
as a host integration error rather than returning a fabricated result. Do not
jump across the wrapper through unrelated nonlocal-exit machinery, or cross
C++ scopes requiring destructors. Complete guest longjmp/SEH integration and
floating environment preservation need their own qualification.

The synthetic `division_lower` tests compile this actual runtime with either
the pinned generic register header or default CI declarations. Linux64, actual
ELF32 and actual PE32 under Wine pass exhaustive byte-dividend/divisor coverage,
word/dword boundaries/random cases and register aliases. There are12,633,635
successful actual x86 result comparisons and21,133,789 checked guest fault
cases on each target. Linux additionally confirms15 representative real x86
SIGFPE outcomes in reaped children with core dumps disabled; this physical-fault
oracle is not run on Windows. Nested generated calls verify precise fault EIP,
local EBP, guest stack/return slot, no following instruction execution and no
partial quotient/remainder writes. Nested host boundary restoration and8threads
with1,000 faults each pass. This proves the integer boundary's tested contract,
not complete game exception behavior or physical Windows gameplay.

Private full-selection C also compiles at O3/strict warnings and relocatably
links these services on ELF64, ELF32 and i386 COFF:16,341 generated definitions,
concrete control/comparison/arithmetic/fault services and no unresolved
generated-function or service calls. Other runtime helpers remain external; this is not a full
executable link or gameplay test. No executable-derived code is included here.

## SSE1 control and comparison

Guest MXCSR has its own per-thread storage, initially0x1f80, and accepts the
Pentium III mask0xffbf. Host control state is never copied into this profile.
DAZ and AMD's misaligned-SSE mask are rejected along with all other reserved
bits, leaving guest state unchanged and delivering vector13. A valid LDMXCSR
may install sticky flags with their masks clear without raising an immediate
exception. STMXCSR stores the guest value through the logical memory helper.

COMISS/UCOMISS compare binary32 bit patterns using only integer classification
and ordering. They recognize both signed zeros, infinities, subnormals and
quiet/signaling NaNs without converting them through a host floating type.
COMISS raises invalid for either NaN; UCOMISS only for signaling NaNs. NaNs
take precedence over a subnormal in the other operand. Newly raised conditions
update guest status before an unmasked condition exits through vector19.
EFLAGS updates happen only on completion; XMM contents are never modified.
The modeled exception mode assumes SSE and OS SIMD exception support enabled;
complete guest CR0/CR4 exception selection still needs integration.

The actual host advertises MXCSR_MASK0x0002ffff, accepting DAZ and AMD MM. Its
extra controls are tested as host-supported while the guest continues rejecting
them. Physical GP tests select only bits rejected by the host's FXSAVE mask.
This prevents a host extension from silently changing the Xbox profile.

Linux64, actual ELF32 and actual PE32 Wine each pass537,088 hardware result and
MXCSR comparisons,32,768 valid control round trips,6,400 mask combinations,
6 explicit unmasked SIMD cases,17 guest-reserved-bit rejections, injected
load/store ordering and8threads with1,000 independent control states each.
Linux64/ELF32 additionally capture21 physical fault contexts on this host:
6 SIMD faults (including exact MXCSR and unchanged arithmetic flags) and15
host-reserved-bit GP faults with unchanged MXCSR. Windows does not execute the
physical signal-context oracle. Counts vary with the host's advertised reserved
mask; guest rejection does not. All36host/27Windows-tool tests and strict checks
pass. These qualify comparison/control services, not SSE arithmetic/conversion,
complete guest exception recovery or physical console/game fidelity.

## Portable binary32 arithmetic

gamepkg_x86_arithmetic.c adds scalar/packed add, subtract, multiply, divide and
square root with the pinned SoftFloat subset in softfloat/. Sources operate on
integer bit patterns; their8086-SSE specialization describes guest NaNs, not
host instruction requirements. Link the20 listed sources and the host wrapper.
Its TLS library state is saved/restored before guest faults. All packed results
stage until pre/post exception checks succeed. Tiny exact results, FTZ and
unbounded-exponent precision require explicit SSE handling beyond SoftFloat's
default masked IEEE result. Independent lanes retain their own exception
contributions. See softfloat/README.md for the two reviewed adaptations.

718,080 direct and218,880 emitted hardware cases pass on Linux64, ELF32 and
PE32 Wine, with30 emitted fault gates and 8,000 threaded comparisons. Linux64/
ELF32 inspect2,640 physical outcomes across rounding, FTZ and exception masks.
A720,720-case LE corpus adds these outcomes to the normal comparisons and
matches exactly on native64, actual PE32 Wine and real big-endian MIPS32 under
QEMU. The MIPS check exercises actual32bit integer/TLS/fault-boundary behavior;
it does not qualify full MIPS gameplay. Asset-free CI uses the portability
script and locally generated corpus. All40host/30Windows-tool tests pass.

Remaining conversions, other SSE/MMX/x87 instructions, full guest control-mode/
exception/ABI integration and measured gameplay optimization are still required.

## SIMD memory alignment

Generated MOVAPS and legacy packed memory operations now call the vector13
service before memory access when their final guest linear address is not
16byte aligned. This uses the same mandatory fault boundary and live local
EBP as other faults; no host pointer alignment or native float cast is used.
Constant aligned addresses need no check. The new fixture passes1,344
generated cases on Linux64, actual ELF32 and PE32 Wine, plus420 physical GP
faults each on Linux64/ELF32. Current full suites pass42 host and31 Windows
tests. The16,643-function private module compiles and links these services on
all three object targets. Page/segment validation, partial-store rollback,
complete guest exception recovery and full game/runtime integration remain
required. See the generator README for source hashes and exact test scope.

## Scalar float/integer conversion

The SSE service also exposes gamepkg_x86_convert_f32: operation0 converts a
signed32 bit pattern to binary32,1 converts binary32 to signed32 with guest
rounding and2 truncates. All computation is bounded unsigned integer arithmetic;
no host floating-point environment or SoftFloat state is touched. New IE/PE
status is delivered through the existing fault boundary before the returned
result can commit. Denormal-operand exceptions are not raised by these
instructions, and FTZ does not flush their integer results. Invalid conversion
returns80000000 when masked; unmasked invalid/precision leaves the destination
unchanged. Generated code supplies exact instruction/local EBP and performs
memory reads first. The service is host-only, not a guest crate/API.

1,771,152 masked and36,960 emitted hardware comparisons, all64 exception masks,
8,000 threaded comparisons and20 physical Linux fault contexts pass on the
qualified targets described in the generator README. Linux32 uses actual ELF32;
Windows32 uses actual PE32 under Wine. These checks do not provide guest SEH
resumption or complete native game/runtime integration.

## Shared x87/MMX state

Link gamepkg_x86_x87.c and the updated fault service for generated MMX/control
instructions. The service owns thread-local physical80bit registers with full
significands, sign/exponents, control/status/tags and initial environment fields.
MMX reads/writes share those bits with future x87 operations; the old upstream
separate g_mm/g_fp_stack files are not bridged by this implementation. State
accessors are trusted host APIs and never guest pointers or a save-image ABI.

MMX writes validate tags, set TOP0 and set the destination exponent/signFFFF.
Read-only operations preserve physical data while changing tags/TOP; EMMS
empties tags. FNCLEX preserves condition codes/TOP while clearing errors/busy;
FNINIT leaves data bits intact. FNSTCW/FNSTSW are non-waiting. MMX and FWAIT
first deliver vector16 when exception flags are unmasked, regardless of an
inconsistent restored summary bit. All result/state updates follow successful
memory reads; multi-lane write rollback is still a separate requirement.

393,336 hardware/generated cases compare every80bit register, status/control
and nonempty tags across all TOP positions,16 binary operations and transfers.
65,536 FNCLEX cases match hardware.288 pending-exception gates include real
Linux64/ELF32 signal outcomes, with equivalent software checks on PE32 Wine;
8,000 threaded transitions verify independent state. Full suites pass48 host/
35 Windows tests. Actual x87 arithmetic, complete save/restore and tags
reconstruction, CR0/CR4 selection, full fault context and guest SEH/resumption,
thread/kernel/manual ABI and signed GamePKG launch integration remain open.

## Non-committing extended arithmetic

Link gamepkg_x86_ext80.c and both softfloat/sources.txt and
softfloat/ext80-sources.txt for default masked add/subtract/multiply/divide/
square-root numerical results on raw80bit values. The new backend is separate
from architectural x87 state: it reports new exception flags and restores all
private math-library state, with no guest register/tag/stack mutation or native
C floating computation. Its PC00/10/11 and four RC modes pass1,023,600 actual
x87 comparisons plus8,000 thread cases on Linux64, ELF32 and PE32 Wine.
The same main corpus passes on big-endian MIPS32 under QEMU/UB traps.

Before guest opcodes can use this backend, implement C1, wrapped unmasked
range results, stack/tag exceptions, deferred status/delivery, memory/metadata
and pop/commit semantics. This API does not make masked defaults a substitute
for the guest's exception policy. Full x87 arithmetic, remaining operations,
old/new kernel/thread ABI, complete exception contexts and GamePKG integration
remain unfinished. See the main generator README for reproducible checks.

## Architectural register arithmetic

The generated x87 register forms now use gamepkg_x86_x87_arithmetic.c with the
shared physical80bit state. It implements logical stack operands, reversal,
pop/tag updates, stack-underflow responses, C1, deferred pre/post exceptions
and wrapped unmasked range results. Pending faults are checked before new
metadata or register changes. A deferred fault preserves original FIP/opcode
and is delivered at the next waiting instruction's EIP/local EBP. Full guest
selectors/context/SEH and old/new kernel/thread ABI remain incomplete.

The round-pack observer snapshots normalized unrounded96bit input; local
rerounding computes C1 and wrapped values without recomputing arithmetic or
changing the original numerical algorithm. Its TLS state is restored along
with the library environment.1,023,600 masked/C1 cases,100,920 wrapped range
responses and 8,000 thread cases pass on Linux64/ELF32/PE32 Wine. The v2 corpus
also replays all output metadata exactly on big-endian MIPS32 under QEMU.

1,351,680 actual hardware/generated architectural cases across55 representative
encodings exercise all masks, precision/rounding settings, TOP positions,
empty operands, unsupported MMX-like contents and popping.585,174 queued-fault
checks verify precise next-wait delivery with unchanged prior x87 state.
C0/C2/C3 are undefined and excluded from equality claims. Suites pass53 host/
38 Windows tests. PC01, memory forms, loads/stores/conversions, comparisons,
transcendentals and complete environment/control support remain unfinished.
The earlier non-committing-backend section records its initial stage; C1,
wrapped-range metadata and register instruction wiring are now implemented.


## Exact floating and integer loads

Link gamepkg_x86_x87_load.c alongside x87/fault state for FLD memory/ST(i),
FILD 16/32/64 and FLD constants. Integer-only conversions preserve extended
precision and special encodings. Memory reads stage the complete operand after
the old pending-fault check; failed reads leave all state unchanged. The service
handles TOP/tags, masked/unmasked invalid/stack responses, defined C1 and
metadata. Newly unmasked denormal loads still push and defer the error.
Register/constants preserve FDP.1,736,704 hardware cases,310,177 queued-fault
checks and 8,000 thread cases pass Linux64/ELF32/PE32 Wine; Linux additionally
checks 18 real fault contexts. FIP equivalence is limited to the metadata exposed
by the actual save format, as detailed in the main generator README.

## Additional MMX integer lanes

The shared state service now handles21 further saturation/multiply/pack/unpack
operations, for 37 binary forms total. Signed64 intermediates and unsigned
masks avoid host signed-overflow/shift/endian assumptions. PMADDWD wraps the
minimum-word pair overflow, pack reads precede writes, and low unpack memory
reads only 32 bits. Actual protected-page checks verify that width on Linux64
and ELF32. The expanded1,433,720-case hardware fixture passes on both and on
PE32 Wine, with all prior pending-fault/clear-status/thread checks retained.

The combined16,793-function module compiles and links all instruction services
on ELF64/ELF32/i386 COFF; kernel/MMIO/other host references remain external.
Suites pass56 host/40 Windows-tool tests and strict release lint/formatting.
MMX shifts/shuffle/average/minimum/insert/mask and streaming stores/fences,
x87 stores/memory arithmetic/conversions/comparisons/transcendentals/full
control/environment, full fault/CR0/CR4/context/SEH and kernel/thread/old-new
ABI remain required. The upstream independent double/MMX files are not an
acceptable shared-state bridge. This remains a partial native runtime, with
signed launch integration and full game/platform qualification unfinished.


## MMX immediate and variable shifts

PSLLW/D/Q, PSRLW/D/Q and PSRAW/D use `gamepkg_x86_mmx_shift` in the shared
x87 service. The pure integer helper consumes the full unsigned 64-bit count,
zeroes or sign-fills oversized lanes and avoids C shifts by zero/full width.
The emitter stages counts before commit, validates typed operands against the
encoded instruction, checks old pending faults before reads and performs the
MMX tag/TOP/exponent transition even at count zero. Both memory dwords must be
read; failure preserves state. Existing floating metadata stays unchanged.

The 2,368-form synthetic fixture passes 1,343,488 hardware comparisons,
pending-fault checks for every form, 384 read-failure gates and 8,000 thread
cases on Linux64, actual ELF32 and PE32 Wine. Linux targets add 16 real memory/
pending-fault priority cases. Identical explicit-LE corpora from all three
replay exactly through the production helper on big-endian MIPS32/QEMU with
UB traps. The main generator README records hashes, scope and reproduction.
CI adds this replay; the script is verified locally, not remotely.

The expanded 16,814-function partial module relinks current instruction
services on ELF64/ELF32/i386 COFF. Full suites pass 59 host / 42 Windows-tool
tests and strict lint/formatting. Remaining MMX and mixed SSE conversions,
x87 memory arithmetic/stores/other instructions and full exception/context/
SEH/kernel/thread/old-new ABI and signed GamePKG gameplay remain unfinished.


## SSE-era MMX lane operations and transfers

Link `gamepkg_x86_mmx_lanes.c` for PAVGB/W, PMINUB/PMAXUB, PMINSW/PMAXSW,
PSADBW, PSHUFW, PINSRW, PEXTRW and PMOVMSKB. Integer-only operations preserve
raw bits and apply round-up averages, explicit signed ordering, masked word
selectors and zero-extended GPR results. Read-only MMX forms update tags/TOP
without modifying physical register images; destination forms additionally
set the exponent FFFF. All preserve guest floating metadata and EFLAGS.
PINSRW memory reads exactly two bytes, and failures precede state changes.
Encoded opcode/register/immediate fields are checked against typed metadata.

The 4,200-form oracle passes 1,848,320 hardware comparisons, every-form pending
fault checks, 976 read failures and 8,000 thread cases on Linux64/ELF32/PE32
Wine. Linux adds six real memory-priority/word-width cases. Identical explicit-LE
corpora replay through the production helper on big-endian MIPS32/QEMU with
UB traps. Full suites pass 62 host / 44 Windows-tool tests and strict lint/
formatting. The 16,824-function partial module relinks all instruction services
on ELF64/ELF32/i386 COFF, with other kernel/MMIO/host symbols still external.

Streaming/masked stores, fences, prefetch and mixed MMX/SSE conversions remain,
as do x87 memory arithmetic/stores/other instructions and complete context/
SEH/kernel/thread/old-new ABI and signed GamePKG gameplay. The main generator
README records exact forms, hashes, reproduction and incomplete requirements.


## Mixed MMX/SSE numeric conversions

Link `gamepkg_x86_mmx_convert.c` and the current SSE service for CVTPI2PS,
CVTPS2PI and CVTTPS2PI. A pure scalar conversion primitive stages both lanes;
new invalid conditions precede precision across the pair. The original scalar
wrapper retains its ABI and is regression-qualified on all three x86 targets.

Memory-only CVTPI2PS does not enter MMX mode or check an old x87 exception.
Other forms check before reads, then enter after successful reads but before
possible SIMD exceptions. SIMD faults retain MMX tags/TOP changes and suppress
the numerical destination. Read failures retain the original state. XMM upper
lanes, unrelated physical register images, guest flags and virtual FP metadata
are preserved. This is shared physical state, not independent MMX/double files.

The 96-form fixture passes 417,792 hardware-based comparisons, all mask/RC/FTZ
and sticky-state checks, 96 pending-state cases, 144 read failures and 8,000
thread comparisons on Linux64/ELF32/PE32 Wine. Linux targets additionally compare
3,072 real exception contexts and six memory-priority cases. Numeric corpora
match across all three and replay through the pure primitive on big-endian
MIPS32/QEMU with UB traps. The main README records the memory-form distinction,
primary references, exact scope, hashes and reproduction. Physical Xbox and
full platform/game qualification remain open.

Suites pass 65 host / 46 Windows-tool tests and strict lint/formatting. Current
instruction diagnostics gain all 56 mixed sites, but complete-function selection
and its private C remain unchanged at 16,824. Current conversion/SSE and earlier
instruction services relink on ELF64/ELF32/i386 COFF; other host/kernel/MMIO
symbols and full runtime/ABI/signed launch integration remain unfinished.


## Checked RAM transactions and streaming/cache operations

Link `gamepkg_x86_memory.c` and the current fault/x87 services for generated
MOVNTQ, MOVNTPS, MASKMOVQ, SFENCE and PREFETCH hints. The reviewed host API owns
an immutable, refcounted view of at most 64 nonoverlapping borrowed RAM regions;
all workers must bind/unbind and the backing arena must outlive every binding.
Complete spans admit before any selected-byte store, read outputs stage until
success, and faults unlock before delivery. Serialized access prevents torn
transactions within the shared synchronization domain. Devices/MMIO cannot be
registered as RAM; missing binding aborts as a host integration error.

MOVNTPS checks final 16-byte alignment. MOVNTQ enters MMX only after a successful
write; MASKMOVQ enters before address/store faults even at mask zero. Old x87
errors precede those MMX forms. SFENCE is a real C11 fence; cache hints are
best-effort and publication is stronger than weak WC ordering. Prefetch does
not produce guest reads/faults for unmapped addresses. Exact cache/bus timing
is outside this component qualification. Page-fault results now append the
first denied address/error code; all fault consumers require rebuilding.

425,984 hardware cases pass Linux64/ELF32/PE32 Wine. Linux compares 550/570 real
fault contexts, including every masked fault pattern, alignment/old-error
priority and 32-bit segment bounds. The concrete RAM suite passes 41,280 access
cases, reference lifetime/fault rollback/wrap, 16,000 threaded transactions and
20,000 concurrent writes crossing a 64-byte boundary. ASan/UBSan/leak checks and big-endian
MIPS32 replay pass. Full suites pass 70 host / 49 Windows-tool tests and strict
lint/formatting. The main README records exact scope, counts and reproduction.

Clang MinGW code sharing GCC TLS must use `-femulated-tls`. The reusable Windows
check verifies emitted code plus 8,000 cross-compiler threaded fault contexts;
local Wine execution and CI-style link-only checks pass. The 16,852-function
partial module links on ELF64/ELF32/i386 COFF with all instruction services
resolved. Host engine map/device hooks, all thread bindings, legacy callback/
ordinary-wide-store migration, full paging/segmentation/SEH/kernel/old-new ABI
and signed game launch still require integration. This RAM view resolves host
capabilities, not guest page tables, and does not claim all-memory protection.

x87 floating/integer memory arithmetic now uses complete checked RAM reads and
the register arithmetic commit path. Exact IEEE decode preserves signaling-NaN
selection and source DE after normalization; integer16/32 inputs are exact.
Old pending exceptions precede span checks/reads, and failures preserve all FP
bits/metadata. Binary32/64 and signed16/32 cover 144 encodings with 2,211,840
physical comparisons on Linux64/ELF32 and equivalent PE32 Wine runs, including a
Clang-emulated-TLS/GCC boundary. 624 admission/priority cases, 72 physical Linux
fault contexts and 8,000 worker cases pass. A 737,280-row LE corpus replays on
big-endian MIPS32 with 277,104 deferred faults. New portability/target scripts
and the main README record reproduction and precise scope. Suites pass 73 host /
51 Windows-tool tests. A 16,858-function partial module links all instruction
helpers on ELF64/ELF32/PE32; actual engine bindings, remaining x87 operations and
complete signed game launch remain open.

The new integer-only raw80 comparison service implements ordered/unordered
register, memory floating/integer and zero comparisons, optional EFLAGS, and
zero/one/two pops. Complete checked reads and old pending delivery precede any
new FP metadata/flags. It preserves numerical bits and sticky status, handles
empty/NaN/unsupported/denormal priority, and suppresses pop on unmasked IE/DE.
The measured C1/flag publication profile and differences from SDM wording are
explicit in the main README; physical Xbox/PentiumIII qualification remains.
7,247,872 physical state/flags cases, 2,693,572 deferred faults, 323 priority
cases, 369 Linux kernel FPU contexts and 8,000 worker cases pass on Linux64/
ELF32. Both PE32 Wine compiler profiles pass non-Linux-signal cases. A 423,104-
row explicit-LE corpus replays on big-endian MIPS32 with 151,494 deferred faults.
Suites pass 76 host/53 Windows-tool tests. The 16,862-function partial module
resolves comparison and other instruction helpers on ELF64/ELF32/i386 COFF;
full engine memory/device/thread/ABI and signed playable launch remain open.

x87 stores now cover binary32/64/raw80/register and signed16/32/64 conversion,
optional popping and deferred exceptions. Integer-only conversion uses no host
FP/native128 or new SoftFloat sources. Whole-span admission precedes conversion;
immutable live RAM bindings allow later re-admission before committing memory
and staged FP state. Unmasked IE/OE/UE suppress writes/pops; unmasked PE commits
rounded output/pop before its next-wait fault. Raw80 spans are ten bytes.
4,487,168 hardware state/target-buffer cases, 1,799,975 queued faults, 576
priority cases, 412 Linux FPU contexts and 8,000 workers pass on Linux64/ELF32;
both PE32 Wine compiler profiles pass non-Linux-signal cases. A 383,264-row LE
corpus replays on MIPS32 with 155,631 deferred faults. The RAM suite now checks
74,112 cases, complete ten-byte masks, no-data admission, rollback/wrap and
existing thread/reference lifetime gates across Linux64/32, PE32 Wine and MIPS32.
Suites pass 79 host/55 Windows-tool tests. A 17,056-function partial module links
all instruction helpers on ELF64/ELF32/i386 COFF. Remaining x87 stack/control/
environment/transcendentals, full engine RAM/device/worker/ABI and signed playable
launch remain open. The main README records precise scope and reproduction.

The new integer-only stack service adds sign changes, exchange/free/TOP motion,
classification, no-op, exact integral rounding, exponent extraction and eight
conditional moves. It uses shared physical 80-bit registers and read-only supplied
EFLAGS, including untaken-condition stack faults, C1 preservation/underflow,
source-before-destination FXTRACT faults and exact masked/deferred responses.
6,447,104 hardware state cases, 1,994,542 next-wait faults, 88 pending gates,
243 Linux kernel FPU contexts and 8,000 workers pass on Linux64/ELF32; both PE32
Wine compiler profiles pass non-Linux-signal cases. A 349,184-row full-state LE
corpus replays on MIPS32 with 102,690 deferred faults, independent supplied flags
and unchanged unrelated registers. Suites pass 82 host/57 Windows-tool tests and
strict lint/format. A 17,161-function partial module resolves all instruction
helpers on three targets. Control/environment, scale/remainder/transcendentals,
full engine RAM/device/worker/ABI and signed playable launch still remain.

Protected-mode x87 control/environment is now a separate staged service for
FLDCW/FLDENV/FRSTOR/FNSTENV/FNSAVE. Explicit LE14/28/94/108-byte formats preserve
logical register order and physical tags, with complete RAM rollback before
state commit. No-wait saves mask/reset only after successful writes. Reviewed
host threads bind CS/DS/SS/FS metadata; generated memory instructions retain
logical FDP offsets across FS translation/wrap. This does not bind actual engine
memory/workers or implement segment descriptors. Reserved PC01 follows measured
64-bit-significand arithmetic, with physical legacy-CPU qualification outstanding.
286,720 hardware cases/33,690 deferred faults, ELF32 and both PE32 Wine compiler
profiles match; native/MIPS32 numerical replay is identical.8,000 worker bindings across these hardware profiles and526,336 additional1..512-byte RAM transactions pass. Reproduction,
exact exclusions for modern deprecated metadata, hashes and scope are in the
native-generator README. The fresh17,168-function partial module resolves these
services on three object targets. Real/V86/FXSAVE images, scale/remainder/
transcendentals, full engine/fault/CPU/ABI and signed playable launch remain open.

FSCALE now uses the shared arithmetic service's fixed operation5 (ST0 by ST1,
no reverse/pop). A separate pure raw80 scale entry truncates independently of RC,
ignores PC and applies extended-range rounding. One wrapped exponent response
that still fails range commits infinity/zero with PE independently of RC;
masked range results honor RC.2,089,999 hardware state cases/756,350 queued faults,
8,000 workers and all integer scales−65,537..65,537 pass Linux64/ELF32 and both
PE32 Wine compiler profiles; complete LE before/after state replays on MIPS32.
Host math state and unrelated guest registers are preserved. Exact reproduction,
hash and scope are in the generator README. Every current13 FSCALE site emits,
but the17,168-function closed set is unchanged because other dependencies remain
unsupported. All runtime numerical services relink on three object targets.
Remainder/transcendentals and full engine/CPU/ABI/signed launch remain open.

FPREM/FPREM1 now use fixed arithmetic operations6/7. A pure unsigned64 backend
implements exact truncating/nearest-even quotient and remainder, with no PC/RC
dependence or PE. Large exponent differences perform32+(D modulo32) reduction,
a measured profile within Intel's permitted implementation-dependent range.
C2/quotient flags, invalid/stack/DE priority and exact tiny-result wrapped UE
responses match4,136,989 hardware states/1,183,892 deferred faults and8,000 workers
across Linux64/ELF32 and both PE32 Wine compiler profiles. Complete LE replay
matches on MIPS32. Older FXRSTOR oracle sites also declare all XMM clobbers;
prior numerical corpora remain identical. Scripts, hashes, exact scope and
remaining physical legacy-CPU qualification are in the generator README.
There are no remainder sites in the current backed game inventory; the closed
17,168-function module is unchanged and relinks all services on three targets.
Transcendentals/other x87 formats and full engine/CPU/ABI/signed launch remain open.

The internal gamepkg_x86_f128 helper adds portable113-bit-significand numerical
workspace for upcoming transcendental evaluation, using26 additional pinned
SoftFloat3e memory-arithmetic/conversion sources. All production values are
integer words; compiler __float128/long double exist only in the independent
maintainer fixture.660,646 GCC reference cases and32,000 worker operations pass
Linux64/ELF32; complete value/flag data replays on MIPS32 and both Windows32 Wine
compiler/TLS profiles. Three original-source adaptations fix a signed leading
shift and big-endian common-NaN payload accesses; an original-source MIPS guard
regression fails before the fix and passes after. All78 pins are checked.
The helper is numerical workspace, without source DE/C1/wrapped-fault or guest
instruction commit handling. It is linked into the17,168-function partial module
on three object targets, with no production compiler binary128 references.
Transcendental instruction integration and full engine/CPU/ABI/signed launch
remain open. Exact scope and reproduction are in the native-generator README.


F2XM1 now uses the separate shared-state gamepkg_x86_x87_trans service (op0),
with raw80 final rounding, masked/wrapped responses and deferred delivery.
Its isolated LGPL numerical component and maintainer-only independent C++/
MPFR reference are documented in [transcendentals/README.md](transcendentals/README.md).
146,304 full-state cases,44,064 queued faults,8,000 workers and279,808 monotonic
pairs pass the documented fixed approximation profile/architectural accuracy
checks. Exact full-state replay passes Linux64/ELF32, both Windows32 Wine
compiler/TLS profiles and MIPS32. The full changed17,173-function partial module
compiles/links these services on three object targets. Other transcendental
instructions and complete engine/CPU/SEH/ABI/signed playable integration remain
open; original target CPU bit-for-bit approximation qualification is unverified.


FYL2X is now op1 of gamepkg_x86_x87_trans, with ST1 result/conditional pop,
IE/DE/ZE priority, signed special values and wrapped-range/deferred responses.
Its portable isolated evaluator uses the pinned reduction profile, with exact
integer logs for power-of-two inputs.1,585,260 full-state cases/650,192 queued
faults,8,000 workers and133,116 monotonic pairs pass independent reference,
MPFR512 accuracy and exact physical state checks. Complete state replay passes
ELF32/PE32 Wine/MIPS32; the fresh17,191-function partial module links all math
services on three object targets. See the generator README for precise profile
limits, hashes and reproduction. Remaining transcendentals and full engine/
CPU/SEH/ABI/signed playable integration remain open.


FPATAN is shared trans op2 (ST1 result and conditional pop). The numerical
component supplies table/polynomial ratio reduction and quadrant reconstruction;
the host service owns signed zero/infinity pi responses, source/stack priority,
metadata and wrapped/deferred effects. Exact tiny dyadic ratios retain a lower
tail before final rounding; otherwise an uncorrected ratio can miss directed
predecessor/underflow behavior. Tiny divisions are normalized before computing
the ratio. The physical boundary regression, independent original profile and
MPFR512 accuracy checks pass2,409,580 states/1,085,424 queued faults,8,000 workers
and264,192 quadrant monotonic pairs. Full state replays agree across the tested
width/compiler/endian profiles. Fresh17,199-function partial modules link on
three targets. Exact scope/hashes/reproduction are in the generator README;
remaining instructions and full engine/CPU/SEH/ABI/signed playable integration
remain open.


Standalone FSIN/FCOS are shared trans ops3/4, unary ST0 operations with defined
C2 rejection for finite |x|>=2^63. Binary pop/write behavior is restricted to
ops1/2. The portable standalone evaluator uses integer192-bit reduction with
legacy66-bit pi/2 and distinct sine/cosine reconstruction. Tiny results retain
a lower tail for directed rounding and wrapped underflow.1,077,056 states,
447,552 deferred faults,8,000 workers and8,704 local monotonic pairs pass
physical/pinned-reference and MPFR512 checks. Accuracy separates local reduced
function error from legacy-pi phase drift. Extreme-tiny binade accounting is
also corrected in FPATAN's oracle, with both suites rerun. Complete state replay
matches tested32-bit/compiler/endian profiles; fresh17,231-function modules
partially link on three targets. The name gamepkg_fsincos identifies the
shared numerical source, not combined-instruction support: FSINCOS/FPTAN and
full engine/CPU/SEH/ABI/signed playable integration remain open. Exact scope,
hashes and reproduction are in the generator README.


Combined FSINCOS/FPTAN are trans ops5/6, with a checked push slot and two-result
publication. Empty/overflow priority, NaN duplication, defined range C2,
precondition suppression and post-range/deferred commits match the documented
profile. FSINCOS C1 is selected from sine; FPTAN preserves a wider closing-divide
rounding hint only when narrowing has no fractional decision. Separate original
combined/tangent polynomial paths are used. Tiny FPTAN uses the pinned numerical
profile with independently verified bounds; modern tiny numerical/C1 variations
do not replace exact exception/stack checks.1,077,056 states/452,576 queued
faults,8,000 workers and13,056 local monotonic pairs pass; complete state replays
across tested width/compiler/endian profiles. Tangent mathematical phase drift
is bounded with its pole amplification. Fresh17,233-function partial modules
link all services on three targets. Exact scope/reproduction are in the generator
README. FYL2XP1 and full engine/CPU/SEH/ABI/signed playable integration remain open.


FYL2XP1 is trans op7, binary ST1 result/conditional pop; trig range/C2/push paths
remain restricted to3..6. Its separate log1p evaluation preserves tiny inputs
and delays both operand exponents, with signed special values, source/stack
priority and wrapped/deferred effects. Exact1+x powers avoid table residuals.
The shared adapter also retains a closing113-bit multiply's direction when it
lands on an exact64-bit value, for FYL2XP1/FYL2X and tiny F2XM1, including wrapped
results. Algebraic before-fix regressions demonstrate the missing C1.1,912,940
new-operation states/780,384 queued faults and expanded prior-family corpora
pass independent profile/MPFR and exact exception/stack tests, with complete
width/compiler/endian replay. All source/report/game-body bytes are unchanged
(no current FYL2XP1 game sites); current wrappers relink on three object targets.
Formal-domain accuracy, exact hashes, reproduction and remaining original-CPU
qualification are in the generator README. Packed decimal and full runtime/
CPU-context/SEH/ABI/signed playable integration remain open.

## SSE1 source-bit selection

`gamepkg_x86_select_f32` implements legacy MINSS/MINPS/MAXSS/MAXPS with integer
binary32 ordering, no native floating-point operations or numerical dependency.
Equal signed zeros and NaN operands select the second source's exact bits,
including an unquieted signaling NaN. Both quiet and signaling NaNs raise
invalid; this takes priority over a denormal in that lane. Other packed lanes
contribute their own conditions. Conditions aggregate before any destination
write, so an unmasked exception leaves every lane unchanged. Scalar writes
preserve upper destination lanes. RC and FTZ do not alter source-bit selection.
These rules follow the [Intel instruction reference](https://cdrdv2-public.intel.com/782156/325383-sdm-vol-2abcd.pdf).

The emitter accepts exact SSE1 opcodes, XMM0..7 and reviewed register/memory
forms, stages all memory input before calculation, and uses existing packed
alignment guards and logical little-endian reads. Tests cover aliases,
FS-relative/16-bit addresses, every failed-read position and packed alignment
faults before reads or numerical status changes. The service also supports an
output aliasing either complete source array; scalar calls touch only lane0.

The hardware oracle checks 287,232 direct comparisons and 145,920 generated
forms across zeros, infinities, NaNs, subnormals, random bits, all RC/FTZ choices
and existing sticky flags, plus 1,952 actual Linux exception contexts (828 faults
and 1,124 normal completions) for masks, packed priority and old unmasked status.
There are 8,000 worker iterations per hardware fixture. The explicit LE corpus
of 289,184 full result/status/fault cases replays on actual ELF32, big-endian
MIPS32 QEMU and both PE32 compiler/TLS profiles under Wine. Windows also runs
the hardware masked/generated/thread fixture; its physical signal-context
oracle is Linux-only. These gates do not qualify CR0/CR4 modes, guest SEH or
actual whole-game execution.

## Legacy SSE1 comparison masks

`gamepkg_x86_predicate_f32` implements all eight valid CMPSS/CMPPS predicates
with integer binary32 classification and ordering. Results are raw all-ones or
zero masks, with equal signed zeros. Quiet predicates EQ/UNORD/NEQ/ORD raise
invalid only for signaling NaNs; LT/LE/NLT/NLE also signal for quiet NaNs.
A NaN suppresses same-lane denormal classification; packed lanes independently
contribute conditions. All conditions precede destination commit. Scalar calls
preserve upper output lanes, and in-place source/result arrays are supported.
RC/FTZ and existing sticky flags do not change the comparison result.

The lowerer verifies the opcode, prefix, encoded predicate and canonical alias
or explicit immediate metadata. Reserved immediate values8..255 are rejected,
following the [Intel legacy-compare compiler guidance](https://cdrdv2-public.intel.com/782156/325383-sdm-vol-2abcd.pdf),
rather than enabling AVX predicates in the Pentium III profile. It shares the
reviewed staged memory reader and packed alignment checks with other SSE forms.
There are no native floating comparisons or new numerical dependencies.

`tests/sse_predicate.rs` checks1,148,928 masked hardware cases and583,680
emitted cases, aliases, scalar upper bits, FS/16-bit addressing, all120 failed
source-read positions, alignment-before-numerical status, and source/GPR/flags/
host MXCSR preservation.7,808 actual Linux exception-context cases cover3,128
faults and4,680 normal outcomes, including old unmasked sticky flags that must
not trigger a new exception. Hardware fixtures run8000 worker cases. Tests also
check all496 reserved scalar/packed immediate encodings and equivalence of the
canonical aliases and explicit immediate metadata. All defined bits/status are
exact; no approximation allowance applies.

The LE corpus of1,156,736 result/status/fault records replays under actual ELF32,
big-endian MIPS32 QEMU and both PE32 compiler/TLS profiles under Wine. Windows
also executes the entire masked/generated/thread fixture. Physical signal
contexts are Linux-only. Existing context/SEH, CPU modes, engine/worker/device
bindings and signed-player integration remain separate requirements.

## Restartable data-stack register commits

The data emitter now stages PUSH, POP-to-memory and LEAVE register changes
until their memory access succeeds. PUSH snapshots its input and computes the
new stack address before storing, then commits ESP. POP reads the old stack
slot and computes an ESP-based destination with the advanced pointer without
publishing that pointer; only a successful destination store commits ESP.
Register POP retains its existing ESP/SP overwrite semantics. LEAVE reads from
the old local EBP before changing local EBP or ESP. Normal16/32-bit behavior is
retained, including a word SP write preserving the advanced upper half.

The previous emitters reproducibly changed ESP on a failed store/read. Separate
old-emission regressions fail for PUSH, POP-to-memory and LEAVE. The repaired
fixture passes16 forms, source/store injection, exact post-POP SIB/FS addresses,
subregister carries and unsigned wrap arithmetic. Its modular byte arena tests
address arithmetic only; it is not a segment/page-wrapping implementation.
There are288 base cases plus800 worker iterations,230,688 total cases including
115,344 injected faults per target. The full core fault record is checked at
failed accesses, and successful paths verify normal values and access counts.

A separate actual Linux i386 process uses protected pages and private alternate
signal stacks to observe12 kernel page-fault contexts across word/dword PUSH,
POP and LEAVE. All retain original ESP and the relevant EBP. POP [ESP] faults
at the advanced destination but reports original ESP. Children have a deadline,
core dumps disabled and are reaped. This provides independent instruction
fault-state evidence, not an emulator-derived expected value.

The generated fixture passes host UB checks, actual ELF32 UB traps, big-endian
MIPS32 QEMU and both PE32 compiler/TLS profiles under Wine. The physical-context
oracle is Linux i386-only. Generic MMIO callbacks still need correct instruction
metadata and reviewed engine binding; full segment/SS/CPU fault modes, guest
SEH, handler selection and interior-instruction resumption remain open. This
repairs register commit order, not arbitrary memory/device-side-effect rollback.

## Flag images, status and direction

The integer lowerer now supports LAHF/SAHF, CLC/STC/CMC, CLD/STD and word/dword
PUSHF/POPF under the fixed flat protected-mode CPL0 register profile. Opcode,
prefix, operand count and stack-width metadata are checked. LAHF/SAHF transfer
only the five specified status bits; OF and other flags retain their proper
values. CLD/STD synchronize the direction bridge with the virtual word. The
new eager flags API in `gamepkg_x86_flags.h` is implemented alongside the
thread-local register/fault services, adding no numerical or system dependency.

The PUSH image masks reserved fields, fixes bit1, takes DF from its bridge and
clears RF/VM in the image without changing source registers. POP loads writable
fields, preserves VIF/VIP/VM, clears RF and synchronizes DF. Word loads preserve
upper flags. Stack accesses stage register changes until memory succeeds.
This follows the [Intel flag-transfer reference](https://cdrdv2-public.intel.com/782156/325383-sdm-vol-2abcd.pdf).
These are register data effects: changing an image is not V86/CPL switching,
interrupt scheduling, debug/single-step delivery or legacy lazy-flag recovery.
Those processor-event and engine bridges remain required. Raw-word tests also
exercise field normalization without pretending to enter the modes named by
otherwise opaque cached bits.

Native LAHF/SAHF/carry/direction observations match229,376 generated cases.
The generated logical fixture covers27 opcode/prefix forms,7,997,184 transfer/
transaction cases including1,230,336 injected faults and800 worker iterations.
The independent maintainer-only multiboot kernel runs actual POPF/POPFD/PUSHFD
instructions at CPL0 in QEMU's Pentium III CPU profile. It checks all65536 low
words with permuted upper words, four initial AC/ID states and both operand
sizes:524,288 records. Its IDT handles262,144 single-step events needed to
observe TF correctly; this handler belongs to the oracle, not the player
runtime. The kernel includes no production helper and emits only flags.

The corpus replays exactly with host UB checks, ELF32 UB traps, big-endian
MIPS32 QEMU and both PE32 compiler/TLS profiles under Wine. Native x86 fixtures
also execute on Linux32 and both Windows profiles. QEMU is a maintainer test
requirement, not a player dependency or proof on original Pentium III hardware.
No debug/interrupt/full CPU/SEH/engine/worker/player completion is implied.

## Restartable string RAM operations

Link gamepkg_x86_string.c for MOVS/STOS/LODS/CMPS/SCAS emitted calls. The call
receives operation, byte width, address16, repeat condition, FS-source flag,
EIP and local EBP. Bind checked RAM on each worker before a nonzero operation.
Transfers use logical little-endian words, sequential overlap semantics and
complete-element commits before count/index updates. A repeated zero count
does not touch RAM. Address16 updates retain the high register halves; a
flat32 operand span exceeding its offset limit raises #GP before access.

Comparison flags are staged until normal completion. On repeated compare/scan
faults the documented legacy Intel target retains original EFLAGS, while
indices/count retain completed progress. Actual AMD i386 page-fault observations
show last-comparison flags and are recorded separately, not adopted as the
Pentium III contract. Software tests require original flags on every fault.
Fault repair/retry tests finish from the recorded registers without repeating
completed elements. This is explicit known-entry retry, not arbitrary C-frame
or guest SEH recovery.

The independent protected-mode QEMU producer covers144 forms/2,304 full
register and81,920-byte RAM states. Exact replay and generated boundary/fault/
retry/80-worker checks pass host UB, ELF32, big-endian MIPS32 and both PE32
compiler/TLS profiles under Wine. See the generator README and
check_takedawn_strings.sh for provenance and reproduction. QEMU is a maintainer
oracle, not a player dependency or original physical CPU qualification.

No device/MMIO fallback, interior REP interrupt scheduler, debug-event delivery,
full CPU-mode/segment/SEH/legacy ABI or automatic engine/worker binding is
implemented here. Keep the process time/memory controls; there is no fuel
metering or artificial repetition-count limit. Full playable-package integration
and physical platform qualification remain required.

## Checked RAM exchange transactions

Link the updated gamepkg_x86_memory.c for XCHG/XADD/CMPXCHG emission. The new
memory_rmw entry accepts address,byte width1/2/4,operation0/1/2,value,expected,
EIP and local EBP; it returns the old destination. The lowerer validates exact
encoding/operand metadata and snapshots all register inputs before commit.
Partial-register aliases and identical source/destination are supported. XCHG
preserves flags; XADD/CMPXCHG publish defined arithmetic flags only after memory
succeeds. Flat32 offset-limit crossing raises #GP before any RAM transaction.

The same checked RAM object serializes the complete read/modify/write across
all bound workers. Complete-span read and write permission admission precedes
any access, and a compare miss still writes the old destination. Combined
admission reports a write fault, releases the lock and leaves data/registers/
flags unchanged. Reads after a rejected transaction verify the same object is
unlocked; repair and known-entry retry tests verify no hidden partial changes.
Logical byte staging is unaligned and endian independent. Register forms need
no RAM binding. Unprefixed operations use the same serialized RAM domain.

The independent protected-mode Pentium III QEMU instruction corpus contains
60,416 exact states for59 forms, including FS/address16/LOCK, AL/AH and short
register aliases. Replays check complete recorded registers/flags and RAM.
Logical tests cover1,253,376 outputs,504 faults/repairs,84 region crossings,
6 offset-limit faults and three eight-worker80,000-update contention runs.
A native Linux i386 probe separately checks168 protected-page fault contexts.
Host UB/ELF32 UBtrap/big-endian MIPS32 and both PE32 compiler-TLS profiles under
Wine pass, with native Windows threads and no new DLL dependency. See the
generator README and check_takedawn_atomics.sh for provenance and reproduction.

Every worker must bind the same reviewed RAM/synchronization object. Direct
host arena access, separate objects over shared storage, generic MMIO and
arbitrary device callbacks are outside this transaction contract. No global
CPU bus-lock/device integration, general LOCK ALU/bit operation, CMPXCHG8B,
full CPU/SEH/segment/legacy ABI, engine scheduler or playable launcher is implied.
Those remain integration requirements; retain process time/memory controls.

## SSE reciprocal approximations

Link gamepkg_x86_approx.c and approx/gamepkg_approx.c for the emitted RCPSS/PS
and RSQRTSS/PS calls. The service accepts logical binary32 source/result words,
lanes1/4 and operation0=reciprocal/1=reciprocal square root. Scalar high lanes
are untouched; complete source staging permits in-place/overlapping buffers.
The emitter stages every memory read and packed alignment fault first.

The fixed Bochs profile uses raw integer classification and unchanged tables.
It ignores rounding/FTZ, preserves MXCSR, quiets NaNs without numerical faults,
converts denormals to signed-zero inputs and handles signed infinity/zero,
negative roots and tiny reciprocal outputs explicitly. It does not consume
old pending SIMD status. See approx/README.md for original pins, the separate
LGPL license and future source/relink delivery obligations.

An independent original C++ build matches4,681,772 values exactly;8,192 GMP
integer endpoint certificates prove the documented relative error bound for
normal buckets (RCP tiny-output flushing separate). Native AMD finite-result
variations are recorded, not used to weaken the fixed profile. All32,768 valid
control words preserve full MXCSR, specials and scalar high lanes in native
observations. Exact output replay and20 emitted form/read/alignment/alias/
512-worker checks pass host UB, ELF32, big-endian MIPS32 and both PE32 compiler
profiles under Wine. Only maintainers need the C++/GMP/reference tools.

Original physical Pentium III bit-exact approximation, CPU-mode/segment/event/
SEH/legacy-ABI bridges and full engine/worker/player integration remain open.
These helpers add no instruction metering or player floating-point dependency.

## Instruction-address dispatch and fault reentry

The new dispatcher path uses qualified guest instruction addresses rather than
a native C guest call stack. runtime/gamepkg_x86_dispatch.c consumes a sorted,
unique, immutable trusted-host map from guest PC to a compiled block function.
No guest address is cast into a native function pointer. Each block accepts a
checked PC, switches to an admitted instruction label, publishes current EIP/
local EBP, executes native instructions, and returns on a call/return/external
transfer. Known internal branches remain direct C jumps. An ordinary native
function return without a transfer/stop is a host protocol error.

src/dispatch.rs exposes DispatchProgram::build/write_source for validated units.
It bounds function/instruction/source counts and owned memory, validates unique
entries, identical shared bytes and exact same-PC instruction metadata,
then emits the block source and instruction map. No successful program exists
when any instruction is unsupported, the metadata conflicts or budgets fail.
Writers still require staged publication on I/O failure. This in-memory builder
holds its decoded units/bodies; write_streamed now adds bounded two-pass source
generation and reference-dispatch-source adds diagnostic frontend selection.
Player/source-kit/launcher wiring is still required. The old C-call source
path remains diagnostic and still skips RET target values; it is not upgraded
or made playable by this new API.

CALL snapshots its register/RAM target before pushing the real guest return
PC. Checked RAM admits the complete stack store before ESP changes. RET reads
the actual word/dword return PC, zero-extends word targets, then commits ESP and
immediate cleanup; it can return to a qualified interior instruction. Default
flat32 stack/source segment-limit crossings raise #SS(0)/#GP before RAM; FS
source bases and address16 arithmetic are explicit. RET target-fetch handling
comes after the pop. Missing map targets are reported to the host, not converted
into a false successful return or invented CPU fault. A host must resolve an
import/kernel/compiled entry or establish an actual fetch exception.

The dispatcher returns0 for an explicit trusted host stop,1 for a real guest
fault,2 for an unmapped PC,3 for a host block protocol error, and-1 for invalid
host arguments with unchanged output. Completed guest data/register effects
remain live. The current instruction-site query gives reviewed RAM/MMIO
callbacks correct EIP/local EBP; it does not itself bind generic device code.
Nested dispatch restores its previous host frame/site. Native hooks must return
normally or use the guest fault boundary; unrelated longjmp/C++ unwinding must
not bypass cleanup. Hooks own their guest ABI/register preservation.

Optional explicit FP bindings capture the full raw x87/MMX/XMM bank on faults.
The descriptor is validated against the whole exit output and frozen once
for the entire dispatch run, including across hooks. The existing core64-byte
fault ABI and old run/run_with_fp signatures remain intact. Nonmutating binding
validation is shared through fp_source_compatible. Host-owned valid immutable
maps/banks must outlive execution; pointer span checks do not prove ownership.

After the host repairs/selects guest state, register restoration and dispatch at
the fault PC recreate local EBP at the exact admitted label. Tests retry an
interior DIV after a preceding INC, preserving the INC once and raw FP payloads.
This is generated-code reentry, not a complete CPU/segment/SEH handler, event
scheduler, legacy lazy-flag/double-x87 bridge or automatic RAM/device rollback.

Tests execute120,000 levels of guest calls with a bounded real native stack,
changed return PCs, direct/register/stack-source calls, tail jumps,16-bit returns,
cleanup, unresolved PCs, fault repair, correct callback sites, nested dispatch,
frozen binding descriptors, invalid/overlapping outputs and256 worker runs.
The real stack pointer is sampled; ASan relocated local storage is not used as
an architectural stack-depth measurement. Separate native Linux i386 execution
of public synthetic instruction bytes agrees on direct/indirect/ESP-source calls,
changed returns, tail jumps and120,000 call depth. No game-derived code is used.
Eighteen actual i386 signal contexts independently verify RET load/cleanup and
CALL source/store fault restart state; a RET target-fetch fault has a committed
pop. Modern host fetch error bits are not a Pentium III oracle.

Host ASan/UB, real ELF32 UBtrap, big-endian MIPS32 QEMU and PE32 GCC plus
Clang-emulated-TLS/GCC Wine checks pass, including default and pinned runtime
declarations. tools/check_takedawn_dispatch.sh and CI add these qualification
steps.140 host/96 Windows-tool tests and strict lint/format/shell/YAML pass.
The old17,992-function bodies retain identical bytes and object ABI; updated
fault/dispatch/current numerical services compile/partial-link ELF64/ELF32/
i386 COFF with all instruction/numerical references resolved. This compatibility
check is not whole-game dispatcher generation or executable linking.

No allocation or instruction/depth/fuel quota is added by the dispatcher loop.
Retain the game process time/memory watchdog, including for internal native
loops. Whole-game generation/discovery/repair, kernel/device/worker/legacy ABI,
source-kit/build/signed launch, full gameplay and physical platform qualification
remain open. Readiness remains false; the private prototype/media are unchanged.

The streamed path accepts compatible crossing/overlapping instruction streams,
because distinct x86 PCs can begin inside another instruction's bytes. Same-PC
typed instructions and every shared byte must agree. A bounded 16 KiB rolling
validator, compact sorted map, per-unit input/body hashes and preplanned source
cap preserve fail-closed validation without retaining all decoded bodies. An
independent actual i386 fixture executes both overlapping entries and both
conditional paths. See native-gen/README.md for whole-inventory hashes/counts,
three-target O3 partial compilation and the private real-entry import probe.
That probe stops at PsCreateSystemThreadEx; it supplies no fake kernel success.

gamepkg_x86_capture_registers(eip, local_ebp, output, source) and the pinned
gamepkg_x86_capture_pinned adapter capture the full register image at a normal
dispatcher/import boundary. They use GamePkgX86FpFault as a host context image,
with vector/address/error zero, and the existing restore API for round trips.
No fault is raised to obtain a snapshot. Raw x87 physical/MMX registers and XMM
lanes, pending conditions, MXCSR, GPRs, EFLAGS, DF and FS base are copied without
arithmetic/WAIT or normalization. Local EBP is explicit and capture leaves live
registers unchanged. Output/bindings must be live, aligned and disjoint; invalid
input or nonboolean DF returns -1 before mutation. EIP is data and still needs
host map validation before dispatch. Segment metadata, RAM bindings, scheduler,
handles, guest TIB/stack creation, SEH and legacy lazy flags are not included.
Boundary snapshots match real captured divide-fault images, alternate two raw
contexts and reject malformed/overlapping banks in the existing FP context
fixture. Host UB, real ELF32, big-endian MIPS32 QEMU, PE32 GCC and Clang-emulated-
TLS/GCC Wine profiles pass, including 8,000 worker iterations per profile.

The exact unprefixed CC INT3 form now calls gamepkg_x86_breakpoint_trap with
the next guest EIP and current local EBP. Vector3 is a trap, so dispatcher
result1/out.pc and the saved core EIP identify the following instruction, not
the INT3 for automatic retry. No error code or fault address is synthesized;
all raw FP banks are captured normally. A host debugger/SEH policy must decide
how to handle the breakpoint. This is the flat32 protected execution profile,
not IDT, privilege-transition, virtual8086, generic INT n or debug-register
emulation. Prefixes and IP wrapping remain unqualified. An actual i386 SIGTRAP
oracle verifies next-EIP/stack/register/defined-flag state; generated capture/
restore/reentry executes each surrounding INC once. The dispatcher matrix now
includes this case on every target and 19 actual i386 signal contexts. Full
host/Windows-tool generator suites pass 145/101 tests. The private system-routine
worker probe reaches RtlEnterCriticalSection after INT3 admits its startup unit;
thread/kernel synchronization and signed launch remain open. See the generator
README for exact counts, source hashes and the diagnostic context limitations.

## Checked-RAM Xbox critical sections

gamepkg_x86_critical_section(address, guest_thread, op, eip, ebp) implements
initialize, try-enter, blocking enter and leave on the current RAM object.
The 28-byte little-endian guest ABI follows
[nxdk's RTL_CRITICAL_SECTION declaration](https://github.com/XboxDev/nxdk/blob/master/lib/xboxkrnl/xboxdef.h):
16-byte synchronization event, then signed LockCount/RecursionCount and the
32-bit guest OwningThread token. Initialization writes a synchronization-event
header 0x00040001, zero signal, self-referencing wait-list head, count -1 and no
owner/recursion. The real owned-image lock has exactly that initial image.
Host OS mutex/condition objects never replace or overlay guest fields.

One native metadata mutex/condition belongs to each checked RAM object. Every
transition holds that mutex briefly and the existing RAM transaction lock for
its guest field update. Blocking wait releases both locks; other critical
sections remain usable and no guest-owned lock holds a host mutex across guest
execution. A waiter increments LockCount once, rechecks its predicate after
every wake and consumes one synchronization-event token on handoff. Recursive
acquisition increments both count and depth, and only final leave clears owner
and signals waiters. There is no host address table, per-lock allocation, global
cross-arena lock, entry limit, polling timeout or instruction metering. Native
pthreads are used on Unix/Android and SRW/condition primitives on Windows.

All workers must bind the same RAM object and use distinct stable nonzero guest
KTHREAD tokens. Existing binding references keep the RAM and native condition
alive until blocked workers return/unbind. Host cancellation/unwinding inside
this call is forbidden; thread termination/lifecycle integration remains open.
Time/memory supervision remains external. Invalid host arguments return -1;
inconsistent guest ownership/counts or signed counter overflow return -2 without
committing that transition. They cannot silently acquire a lock or complete an
import. Try contention returns 1 with unchanged fields; successful ops return 0.

Field admission precedes mutation, including signal-field permission checks
before waiter registration or final release. Inner RAM fault capture releases
the native mutex before rethrow to the outer guest boundary, preserving the
original PF address/error/EIP/EBP and optional raw FP capture. Uncontended
operations do not require writing unrelated read-only event bytes. Guest wait
list links remain the initialized empty list: host condition waiters are not
guest KWAIT_BLOCK objects. KeWait/KeSetEvent interoperability, APC/critical-region
variants, hardware fairness/IRQL, thread cleanup and complete kernel objects are
separate qualification requirements; this is not a complete Xbox scheduler.

The concrete RAM fixture checks exact layout, recursion/try/nonowner release,
overflow/malformed states, independent arenas at identical VAs, independent
locks while one worker blocks, deterministic handoff and 16,000 eight-worker
contended recursive updates. Its 216 permission/span cases include 188 faults,
unchanged guest bytes and successful later calls proving native-lock cleanup.
Existing 74,112 RAM access cases, 526,336 bounded image cases and 16,000 ordinary
threaded transactions still pass. Host ASan/UB/leak checks, actual ELF32 UBtrap,
MIPS32 big-endian QEMU and PE32 GCC plus Clang-emulated-TLS/GCC Wine pass.
tools/check_takedawn_memory_windows.sh adds a reproducible Windows gate; CI
also runs the updated RAM fixture on host/MIPS and actual ELF32.

A synthetic generated program calls reviewed initialize/enter/leave imports
through the instruction map, uses the real guest return PC and stdcall stack
cleanup, and verifies recursion returns to unlocked state with caller registers
preserved. This runs in the full dispatcher portability matrix. The first test
version accidentally appended a lower-address test hook after higher entries;
the map validator rejected it. Test hook addresses were moved into sorted order;
validation was not weakened. Full generator suites remain 145 host/101 Windows
tests and strict lint passes.

The private real startup worker now services two actual critical-section calls
through this backend and checked return helper, then reaches unresolved guest
entry 0x001d3f2f without a reported CPU fault. Caller raw state still matches its
saved snapshot. That candidate is rejected for CLI at 0x001d3fce and STI at
0x001d3fd5. Interrupt privilege/virtual CPU state and STI's deferred-delivery
shadow are not replaced by flag-only/no-op stubs. The original parent import
PsCreateSystemThreadEx remains unresolved; diagnostic thread/TIB placements are
still not a production allocator or thread service. All 23,956 unchanged game
blocks remain in three partial links with freshly compiled current runtime
services. Discovery/repairs, kernel/device/legacy/interrupt integration, signed
source-kit/build/launch and actual full gameplay/platform parity remain open.

## Interrupt instruction state and retirement foundation

The standalone gamepkg_x86_interrupt.c/.h component implements the
[Intel CLI](https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-vol-2a-manual.pdf)
and [STI](https://cdrdv2-public.intel.com/782151/253667-sdm-vol-2b.pdf) instruction
state tables. Its explicit host image contains raw flags, PE, CPL, PVI, VME
and a one-bit maskable-interrupt STI shadow. It has no TLS, host flag operations,
native register accesses, allocations, guest pointers or callbacks. This is a
state-transition component, not a virtual CPU scheduler or interrupt controller.

interrupt_step(op0=CLI,1=STI) returns0 for successful retirement,1 when #GP(0)
must be delivered, or-1 for invalid host state/op/pointer. Failure changes no
image fields. Real-mode and permitted protected accesses change physical IF;
restricted protected CPL3/PVI and virtual8086/VME accesses change VIF. A virtual
STI with VIP set requires GP. Other flags and control fields are preserved.
VM/PE/CPL consistency, boolean controls/shadow and pointer span/alignment are
checked. Real IF0-to1 STI installs inhibition at the next boundary; a second
STI with IF already1 does not extend it. CLI removes that inhibition while
clearing its selected flag. VIF-only updates do not enable physical IRQs.

interrupt_eligible is a nonmutating physical maskable-IRQ boundary query.
interrupt_retire_other ends an old STI shadow after another completed
instruction, or when another event/exception is delivered. The host must not
retire STI twice, confuse a failed instruction with successful retirement, or
drop this state from saved thread/fault contexts. The old64-byte core fault ABI
is unchanged and does not capture this new image. A future binding must keep
the image's flags synchronized with g_eflags and preserve all controls/shadow.
No device interrupt is acknowledged or IDT frame built by this API.

The portable fixture checks3,584 declarative mode/privilege/IF/VIF/VIP/shadow
cases, unrelated-bit preservation, unchanged GP/rejections, malformed host
states/pointers, event retirement and independent saved images. A separate
freestanding protected-mode kernel, containing no production service code,
runs on QEMU TCG's Pentium3 model. Its2,048 CPL0 observations cover both ops,
four IOPLs, both initial IF states and all128 arithmetic/direction flag patterns.
Three independent pending-PIT/PIC sequences observe saved IRQ EIP/flags and a
marker: STI;INC delivers after INC, STI;CLI suppresses delivery, and STI;STI
delivers before the subsequent INC. This measures a real virtual interrupt
source and handler rather than inferring shadow behavior from flags alone.

The canonical little-endian corpus is65,644 bytes, SHA-256
`0e0f05dd08eaa3f9a73102721a6e13c912e5b8de8cd29896c54c435add012593`.
Every record replays on host ASan/UB, actual ELF32 UBtrap, MIPS32 big-endian QEMU,
and PE32 GCC/Clang-GCC under Wine. Another actual i386 process executes256 CPL3
CLI/STI fault contexts, independently checking GP13/error0 and unchanged fault
PC/SP/GPRs/flags, accounting separately for hardware RF on fault delivery.
Real/VM86/PVI mode tables beyond the observed CPL0/CPL3 profiles still require
independent hardware/machine qualification; this does not prove original Xbox
CPU or complete virtual8086 execution. All147 host/102 Windows-tool generator
tests and strict lint pass. tools/check_takedawn_interrupts.sh and CI add the
portable machine corpus and mixed-compiler linkage gates.

CLI/STI deliberately remain rejected by generic lowering and dispatch program
construction; a regression enforces this until integration is present. The
missing pieces include instruction-boundary retirement across transfers and
faults, REP interior events, shared single-CPU ownership/preemption across host
workers, disabled-IRQ regions, host blocking boundaries, device IRQ posting/
acceptance, complete handler frames and saved CPU state. MOV/POP SS shadow,
NMI/SMI, APIC/PIC device emulation and debugger behavior are separate work.
This foundation does not admit the0x001d3f2f function or change23,956-function
coverage,43 rejected candidates, source hashes, private startup stopping PC,
native source-kit/launcher state or readiness=false. Fully playable signed
GamePKG and complete game/platform fidelity remain the objective.

## Shared virtual-CPU ownership and host blocking boundaries

gamepkg_x86_cpu.c/.h adds an explicit single-processor domain shared by guest
workers. Opaque host thread contexts retain the interrupt image and a domain
reference; no handle is a guest address or OS thread ID. A FIFO wait queue
controls ownership. One context reserves the CPU across translated execution,
while the native mutex protects only short metadata operations. It is not held
across guest C frames, faults, callbacks or host waits.

enter waits for ownership, or resumes a suspended context's retained reservation.
suspend detaches the native worker while retaining that reservation, including
when IF is clear or STI inhibition is active. Other guest workers cannot steal
it while the host inspects/resolves a fault or import. block releases ownership
for an actual host wait only when physical IF is set and the STI shadow has
ended; inhibition returns1 without mutation. yield similarly respects interrupt
state, lets queued workers run in FIFO order and reacquires behind them. These
are eligibility/ownership rules, not instruction quotas or polling timeouts.

Active handles are tied to the entering native worker. Wrong-worker operations,
duplicate entry and destruction of active/queued/suspended owners reject rather
than silently release CPU state. Full-image restore and instruction/event
retirement require the active owner. Snapshot synchronizes through metadata
locking and rejects output overlapping the opaque domain/context. Thread
destruction requires externally quiescent references; domain ownership references
can be released while live contexts retain it. There is no forced cancellation
or guest thread-termination policy. Host cancellation/unwinding inside a wait
is forbidden. Native waits use pthreads or Windows SRW/condition APIs, with no
fixed context table or guest-native pointer conversion. Resource limits remain
the process supervisor's responsibility.

Tests deterministically queue a second native worker while the owner has IF0,
retain ownership through suspend/resume, reject host blocking/yield while IF0
or under STI shadow, then observe FIFO handoff after retirement. Wrong-worker
operations, owner destruction, snapshot aliasing, full-state restore, GP image
preservation and lifetime after dropping the domain owner reference are checked.
Eight workers perform16,000 serialized entries around ordinary shared memory.
Host ASan/UB, real ELF32 UBtrap, MIPS32 big-endian QEMU and PE32 GCC plus Clang/GCC
Wine pass. tools/check_takedawn_cpu.sh and CI add the portability/linkage gates;
the standard Rust suite includes the ownership fixture. Full suites pass148
host/102 Windows-tool tests and strict lint/format/shell/YAML checks.

The private real-startup probe now binds parent/worker CPU contexts, synchronizes
flags at its checked host-call boundaries, releases/reacquires ownership around
the two real critical-section calls, and preserves the parent's raw FP/GPR plus
CPU image across the diagnostic worker. It stops at the same0x001d3f2f without
a reported CPU fault. Private executable SHA-256:
`7b21f5d5a97f44432b8b39bb42989193aa4dffae4268db884e2eb1b6f3b697e6`.
The probe reports instruction_boundary_integration=false and
irq_delivery_integrated=false. It is still the minimal diagnostic loader/TIB,
and its parent PsCreateSystemThreadEx import never returns fabricated success.

This component does not yet connect retirement to every generated label,
transfer, trap/fault or REP iteration, nor post/accept device IRQs or construct
handler/privilege frames. Binding GPR/FP/RAM/PC, complete CPU context save/restore,
kernel blocking/termination semantics, SS shadow, NMI/SMI and production thread
lifecycle remain required. The generic generator still rejects CLI/STI, and
23,956-function coverage/43 rejected candidates, source hashes and readiness=false
are unchanged. Shared ownership APIs plus selected private host boundaries do
not prove complete single-CPU interrupt scheduling or a playable package.

## Controlled instruction retirement and CPU adapter

DispatchProgram::build_controlled and write_streamed_controlled emit a distinct
gamepkg_cpu_block namespace and gamepkg_x86_cpu_generated instruction table.
Each admitted label calls dispatch_boundary before executing, and every completed
instruction retires before its branch/call/return transfer. Updated local EBP is
published at retirement. A faulting instruction has no normal retirement; the
runtime instead notifies the frozen controller of the detected exception after
capturing the original raw register/fault image. INT3's event PC is its saved
next instruction address. This is not an IDT/privilege handler implementation.

run_controlled freezes trusted host callback pointers/opaque context for the
whole invocation and validates descriptor/output spans and overlap before
execution. Callbacks must preserve virtual registers/bindings and return
normally, and cannot issue their own dispatcher transfer/stop. Protocol misuse
fails before another instruction executes, retaining any already completed
instruction or captured genuine fault. Return4 means a host boundary event
BEFORE the next instruction, distinct from guest exception1, unresolved2 and
protocol3; no CPU fault or IRQ acceptance is fabricated. The caller decides
how to snapshot, handle and resume. Ordinary run on controlled generated code
fails at its first boundary before executing an instruction. The reverse is a
trusted-host profile error: run_controlled requires a reviewed controlled map,
not arbitrary old/native blocks whose machine-code semantics cannot be inferred
from a C function pointer. Future source-kit/cache metadata must select the
profile explicitly. The ordinary source profile and API remain separate.

gamepkg_x86_cpu_dispatch.c/.h binds these hooks to the shared CPU domain. Entry
requires current native-worker ownership and matching raw g_eflags/image flags;
failure leaves output unchanged. Eligible queued workers may run before a label,
while IF0/STI shadow keeps the current owner. Retirement synchronizes flags and
ends old inhibition; detected exceptions end it too. The CPU reservation remains
owned on exit so the host can resolve its actual fault/import/boundary event.
There is no device IRQ posting, acceptance or handler frame in this adapter.
The CPU domain also now rejects entering another context behind the same active
native worker, which would otherwise self-deadlock.

Synthetic tests verify exact callback order through CALL/RET/internal branches,
real fault versus retirement counts, divide repair without replaying INC,
boundary stop/resume without a fabricated fault, INT3 next-PC event, updated EBP,
descriptor freezing, overlap rejection, callback action misuse, and CPU-owner/
raw-flag entry guards. A first-instruction divide fault clears an initial shadow
without losing raw x87/MMX/XMM payloads. Existing120,000-depth,256-worker and19
actual i386 signal-context gates remain. The entire dispatcher fixture passes
host sanitizers/ELF32/MIPS32 big-endian/both PE32 compiler/TLS profiles, including
pinned declarations. Full148 host/102 Windows-tool generator tests and strict
lint pass; streaming controlled/builder output and map hashes agree.

The maintainer reference-controlled-dispatch-source command uses the same
owned-disc verification,192 MiB diagnostic budget,32,768 units/16 KiB extents/
1,048,576 map bounds,512 MiB source cap and private staged output guard. It
produces23,956 functions/772,156 entries,458 overlaps/455 shared tails and43
rejected functions plus one unbacked extent. Source368,019,437 bytes SHA-256
`590af0d41538538b48ba93e7671ec682c113cc8377bc89cb0275a029b3d38225`;
body368,019,411 bytes SHA-256
`717bbe6eedf0cfd7b24bfed3552d84ba8f2b22b294d63994833dabfdb240c43c`;
map SHA-256 `1b1a074a866dce0689438f7438b73bb4cfde230e749e0f97ccf6c637472a33e5`;
report SHA-256 `3b0071406b4af444178b25d5f3d79f34cbe8a013c509b52a327b7cca9ca674f7`.
Linux and Windows tool outputs/reports are byte-identical. Independently
regenerated ordinary source AND report match their previous bytes exactly.

A private verified function-boundary splitter creates81 body units plus map;
maximum function source1,659,852 bytes. Every controlled body is freshly compiled
at O3/strict warnings for Linux64/Linux32/Windows32, with all current services
partial-linked and instruction/numerical/CPU references resolved. Maximum
compiler RSS is668,660/540,296/539,956 KiB. These partial links do not establish
full executable/platform gameplay. The private Linux64 executable enters real
startup through run_cpu on the controlled map, completes both critical-section
imports, preserves parent raw/CPU images and reaches the same0x001d3f2f without
a reported guest CPU fault. Its SHA-256 is
`f321f84389860259e37005783df9b9c20cb48bc753d50b39fcbcd11e99e35e14`.
The report now says instruction_boundary_integration=true, but explicitly
rep_interior_integration=false and irq_delivery_integrated=false.

CLI/STI remain rejected in BOTH generation profiles. REP currently retires only
on whole-instruction completion; partial iteration IRQ/host-event suspension
needs its own checked state protocol. A private pending-PIT experiment on the
Pentium3 QEMU model observes STI;REP MOVSB(32) completing before IRQ delivery
(saved EIP after REP, ECX0, marker not yet incremented), so iteration polling
must preserve an existing shadow rather than clear it unconditionally. This is
not original physical-CPU or ordinary IF1 REP interruption qualification.
Device IRQ posting/acceptance, handler/privilege frames, REP interior semantics,
SS/NMI/SMI, complete CPU contexts, production kernel threads/loader/source-kit/
build/signing/launch and full game/platform fidelity remain required. Readiness
stays false; all game-derived source/chunks/probes remain private and the ROM
is used in place. No assets or private inventory are published.

### Controlled REP checkpoints and comparison rollback

`gamepkg_x86_string_controlled` accepts a trusted host boundary callback. After a
completed element, it offers a checkpoint only when count and repeat predicate
permit another element. A stop returns 1 with partial data/count/indices and
current comparison flags; completion returns 0. No macro retirement occurs on a
stop. A continuing callback restores invocation-entry flags before the next
fault-capable access. A later resumed invocation uses the interrupted comparison
flags as its new entry flags. Continued-versus-resumed CMPS/SCAS fault tests
cover widths, DF, address size, and FS; zero counts never access memory or call
back. Existing ordinary helper behavior is retained.

`gamepkg_x86_cpu_checkpoint` synchronizes current flags without clearing an STI
shadow. The controlled dispatcher exits partial strings before retirement, and
the CPU adapter checkpoints before eligible worker handoff. Synthetic generated
REP stop/resume and old-shadow tests pass across the existing dispatcher matrix.
The new maintainer checkpoint gate replays 17,556 independently measured P3-model
prefix states and resumes against a separate whole-instruction CPU corpus on
host/ELF32/MIPS32-big-endian/both PE32 compiler profiles. All 11,812 interrupted
and 5,744 completed-early cases match full register/RAM images. Actual Linux
ELF32 timer signals add 42 cases with complete shared RAM and RF=1 in saved
contexts. Both gates are asset-free; Wine is not physical Windows qualification.

IRQ acceptance and saved handler frames are not implemented here. In particular,
pre-interrupt logical flags are distinct from saved event flags: Intel SDM 3B
20.3.1.1 requires RF=1 for an interior REP interrupt. PUSHFD in the TF oracle
normalizes RF, and a separate private QEMU P3 PIT experiment omits that flag even
where partial state agrees. The physical signal fixture enforces it separately.
Future delivery must explicitly distinguish REP interiors from instruction
boundaries; do not infer full interrupt-frame fidelity from these checkpoints.
The original startup counter report was later found ineffective and is
withdrawn; the corrected probe below observes four strings/35 REP checkpoints
before its kernel-data fault. Generated support is connected, full game execution and
production thread/device/IRQ integration remain unfinished. See the latest REP
section in the parent README for current source/build hashes and exact scope.

### Scoped boundary snapshots, eligible event sources, and gate flags

Controlled REP sites now use an explicit `dispatch_rep_checkpoint`; the
instruction hook retains its original meaning. `dispatch_checkpoint_kind` and
`dispatch_capture_checkpoint` are available only in a genuine boundary callback.
Capture uses the FP bindings frozen at dispatcher entry and preserves raw
pending x87/MMX/XMM state. It rejects map/result/controller/descriptor/bank/frame
aliases and leaves rejected output unchanged. Nested dispatch restores scope;
retirement and ordinary inner execution cannot capture an outer checkpoint.
Reentrant boundary/retirement calls and faults from a boundary callback produce
protocol failures. These host-only services never expose native callbacks to
unreviewed guest code.

The optional CPU event API polls a frozen source only when physical IF/STI state
allows it, before a queued-worker handoff. A pending notification returns reason
4 at the exact instruction or REP prefix with CPU reservation and host request
retained. It does not invent a fault or acknowledge/deliver an interrupt. The
CPL0 flat32 IRQ flag planner separates saved RF from live handler flags and
interrupt-gate IF clearing from trap-gate preservation; unsupported modes and
inhibition leave source/output unchanged. Gate validation, frame writes,
privilege transitions and IRET are not implemented by this planner.

An independent IRETD/PIT protected-mode kernel captures 8,192 saved/live
interrupt/trap-gate flag images across status/IOPL/TF/NT/RF combinations. Exact
portable replay, 8,192 REP saved-RF plans, and 49,152 inhibited cases pass on host
sanitizers, ELF32, MIPS32 big-endian, and both PE32 compiler profiles. The latter
RF rule uses the prior physical signal evidence and Intel SDM, not QEMU's
unfinished-REP RF omission. The corpus is 196,620 bytes with SHA-256
`ea6bd6453835a77a0c66eeeefb1d8b71bb0659ef4275b0ee97e0616244a7163c`.
`tools/check_takedawn_irq_flags.sh` reproduces the complete gate. Default and
pinned dispatcher matrices, existing CPU/interrupt matrices, full150 host/103
Windows generator tests, strict lint and formatting pass. No physical Windows,
original-P3, full-game endian, performance or playable-release claim follows.

All 23,956 controlled functions were freshly compiled and partial-linked on
Linux64/Linux32/Windows32 after the hook change. A real-entry diagnostic host
request now verifies raw snapshot/CPU ownership preservation before resumption;
actual startup still stops on the unresolved thread/CLI-STI path. No device IRQ
or guest handler is fabricated, and its old zero-string counts are withdrawn in the correction below. The parent README records current source/probe hashes and limitations.
New boundary-kind consumers require regenerated controlled objects; the old REP
callback used the ordinary instruction hook. Production cache/profile pinning,
actual IRQ delivery, kernel/thread services and complete gameplay remain open.

### Controlled virtual CLI/STI execution

Exact unprefixed CLI/STI now lower only in the controlled profile. Their helper
requires the frozen controller's virtual-CPU provider and cannot run a native
host CLI/STI. A missing service is a protocol stop, not an ignored opcode or
invented guest exception. The shared CPU provider performs qualified IF/VIF/VIP
and shadow transitions. A true privilege failure raises GP with the original
raw state. Completed CLI/STI notify retirement events2/3 so a newly established
STI shadow survives; event0 remains ordinary retirement and1 remains exception
notification. All host users of the expanded controller ABI require recompiling.
Existing generated bodies that do not instantiate it retain their helper/map ABI.

Synthetic generated code verifies STI-CLI/STI-STI/STI-REP/STI-fault ordering,
pending notification exclusion, protected32 virtual-IF and GP, raw status/DF,
pending x87/MMX/XMM preservation, provider freezing/rejection and completed-state
retention on controller errors. A real queued native visitor cannot take the CPU
inside the translated CLI region or STI's following instruction; eligible event
polling precedes its eventual FIFO handoff. Default/pinned dispatcher, CPU,
interrupt and IRQ-flag matrices pass across host/ELF32/MIPS32-big-endian/both
PE32 profiles, plus full151 host/104 Windows generator tests and strict lint.
Native object disassembly contains no privileged CLI/STI opcodes. This completes
the explicit controlled instruction/provider/scheduling path; it does not accept
a device IRQ, build a guest handler frame or implement IRET. Pending requests
continue to stop at an eligible boundary for unresolved host delivery.

Whole-game generation admits two additional functions,23,958 total, with41
rejected and one unbacked extent. A private source/hash/used-ABI comparison proves
all23,956 previous function bodies unchanged;81 checked body objects per target
are reused while the new functions/map and runtime/callers are rebuilt. All
functions partial-link on Linux64/Linux32/Windows32. The parent README records
current source/probe hashes and scope. Actual worker entry now advances into the
formerly rejected function, but first faults on the unresolved XboxHardwareInfo
data import at0x80000142, before any live CLI/STI execution. Corrected counters observe four string entries
and35 REP interior checkpoints on the path to that fault. Raw/CPU parent state
is preserved; no export value or successful thread handle is fabricated. A
stopped inhibited worker is never force-enabled just for diagnostic cleanup.
Kernel-data binding, production threads/devices/IRQ handling, full instruction
coverage and playable GamePKG/platform integration remain open.

The old zero-string/zero-REP startup counts in earlier sections are withdrawn:
linker wrapping after partial linking did not intercept already-bound calls.
The corrected private probe substitutes service-definition symbols before final
linking and verifies routing by unresolved symbols/disassembly. CPU counters
are calibrated on a separate host test domain before being reset. The resulting
four string entries/35 REP checkpoints qualify actual startup integration;
zero CLI/STI attempts remain a current observation before the hardware-data
fault. This fixes diagnostic evidence, not nominal runtime behavior. See the
parent README for the corrected executable hash and full limitations.

### Static platform data loader boundary

The Rust host loader now validates the entire original XBE import table before
exposing entries and atomically binds the two reviewed DATA ordinals322/324.
`HardwareInfo` and `KernelVersion` serialize explicit eight-byte LE objects;
these are virtual profiles supplied by the host, not native pointers or host
OS/hardware identity. Binding does not create a memory capability: callers must
register separate read-only spans with storage that outlives all guest workers.
The new fixture validates204 paired profiles, field layout and408 protected
write attempts on host, Linux32, MIPS32 big-endian/QEMU and GCC/mixed-compiler
Windows32/Wine. Full generator suites pass158host/110Windows, lint/format pass,
and generated ordinary/controlled source and reports remain byte-identical.

The private startup diagnostic binds both actual data slots at0x80020000/10,
leaving0x80010000 available for the kernel image. Explicit profile values are
flags0/GPUa1/MCPb1 and declared compatibility target1.0.5838.1; no complete kernel
claim follows. Both read-only writes fault before entry. Startup proceeds to a
real kernel-header read fault at0x001d3ed0/address0x8001003c after two critical
imports, with parent raw/CPU preservation, four strings/35 REP checkpoints and
zero CLI/STI attempts. The next helper uses section metadata/INIT addresses to
construct a descriptor. The old prototype's zero-section header and cache-init
comment do not prove this behavior; that bypass is not adopted. Kernel image
metadata/initialization, production threads/TLS, device/IRQ/handler behavior,
remaining instructions and playable signed GamePKG integration remain open.

### Packed-decimal x87 service extension

The unchanged load/store helper signatures now accept load kind14=FBLD and
store kind7=FBSTP. Explicit integer-only18-digit decoding/encoding shares the
raw x87 bank, existing rounding and deferred-exception behavior. Signed zero,
post-rounding decimal overflow, masked BCD indefinite, invalid/precision
priority and conditional stack pops match the native oracle. FBLD's generated
full-operand segment check precedes staged reads; old pending exceptions take
priority. Failed reads and rejected writes preserve architectural state/RAM.
These are host services with reviewed kinds, not guest-native callbacks.

The new fixture checks344,064 cases including six generated address forms,
all eight raw registers and the complete independently captured x87 tag word.
Its32,686,088-byte LE GPKBCD02 corpus SHA-256 is
`1ea64305bdbc73ba7f329b5696c74da29592d1b2c9585276e41c9ede5f1ed412`.
Linux32 and two Windows32 native profiles reproduce the host corpus exactly;
MIPS32 big-endian/QEMU replay and explicit fault-priority/transaction checks
pass. Original Pentium III and physical Windows remain separate work.
CI adds portability and Windows-link gates. Full generator suites pass161host/
112Windows; strict lint/format pass. Current owned ordinary/controlled source
and reports remain unchanged; fresh Windows32 output agrees with Linux for
both profiles. All23,958 definitions partial-link with
fresh runtime objects on Linux64/Linux32/Windows32 using qualified unchanged
body-object reuse. A fresh startup diagnostic preserves prior CPU/raw state
and reaches the same real kernel-header fault at001d3ed0/address8001003c after
two critical imports, four strings/35 REP checkpoints and zeroCLI/STI attempts.
This does not prove live BCD use or remove the kernel/thread/IRQ/device and
signed-playable-package requirements. The numerical/exception rules follow
[Intel's instruction reference](https://cdrdv2-public.intel.com/671110/325383-sdm-vol-2abcd.pdf);
invalid packed digits have undefined numerical results rather than a newly
invented invalid-operand check.

### Explicit shared GDTR/IDTR component

The CPU domain now stores an explicit, copied `GamePkgX86Tables` configuration
with logical32-bit bases and16-bit limits. Initialization is one-time and must
precede every thread creation; a missing configuration never becomes an
invented zero register. The owner-only snapshot rejects bad alignment/overlap
and leaves rejected outputs unchanged. The shared image is separate from the
old GPR/interrupt context ABI and must accompany future full CPU contexts.

`gamepkg_x86_cpu_store_table` writes a six-byte LE image through checked byte
transactions. The qualified legacy protected32 profile supports full32-bit
bases or the older16-bit operand's24-bit base/zero sixth byte. It validates
op/size/ownership/configuration/mode before touching RAM, preserves CPU images,
and neither retires nor creates memory capabilities. No host SGDT/SIDT/LGDT/
LIDT instruction is used. The independent ring0 Pentium-III-profile QEMU corpus
contains2,448 records,58,764 bytes, SHA-256
`b5a484ba6a28c49037500238cf18981eff1844c001db4188497756f5a7e480fb`.
Host ASan/UB/leak, Linux32, big-endian MIPS32/QEMU and two Windows32/Wine profiles
pass its exact byte/sentinel replay and configuration/ownership/fault tests,
including1,024 eight-worker leases. Existing CPU qualification still passes
16,000 entries per target; full generator suites162host/112Windows and strict
lint/format pass. CI adds portability and Windows-link gates.

The private INIT-path audit reveals SGDT, descriptor exchanges and a far jump
that reloads the code segment, returning the previous descriptor. This is more
specific than the old cache-init comment. At that component milestone guest
table/far-jump lowering remained rejected. The following integration adds
controlled table stores. Far-jump descriptor/cache validation, instruction-fetch
bounds and complete segment context/fault handling remain required. Kernel
metadata and complete processor/thread setup are not bypassed.
The prior startup fault and broader signed-playable/full-game/device/platform
requirements remain open. Original Pentium III and physical Windows behavior
are not established by QEMU/Wine component tests.

### Scoped controlled descriptor-table stores

Controlled SGDT/SIDT lowering now calls `gamepkg_x86_dispatch_store_table`.
A separately scoped/frozen `GamePkgX86DispatchTables` provider supplies only
reviewed logical base/limit values. Existing four-field controller ABI remains
unchanged. CPU dispatch binds the owner-only configured image; missing state,
unqualified mode, bad limits or invalid callbacks stop as host protocol errors
before RAM. The helper leaves callback scope before real segment/page faults,
performs full-span GP/SS validation, commits one six-byte checked transaction
and retires once. The prior STI shadow ends only after successful completion.
No native host table value/instruction is used. Reentrant callbacks are stopped;
completed stores survive retirement callback failure; nested runs need their
own provider. Entry/checkpoint alias checks include original/frozen descriptor
storage and raw FP bindings.

All32 emitted forms agree between builder/streaming and Linux/Windows frontends.
The192-case generated/provider fixture plus malformed/alias/nesting/fault/
retirement gates passes five platform profiles, with pending raw x87 state
preserved. The independent2,448-record legacy ring0 corpus and existing deep
call/fault/REP/CPU dispatcher matrix also pass. Standard suites165host/114Windows
and strict lint/format pass; CI adds portable and Windows-link gates.
Owned ordinary/controlled source and reports remain unchanged, and all23,958
functions partial-link with53 freshly compiled host-runtime objects per target
on Linux64/Linux32/Windows32, reusing verified unchanged bodies/numerical pools.
Fresh startup59c0ebadd73da451b5f4d2820db4b39842dca752651f6c726f26f46786e570b6
preserves prior state and reaches the same kernel-header fault, four strings/
35 REP checkpoints and zeroCLI/STI attempts. No live-game table execution is
claimed. The INIT helper's far jump, descriptor/segment-cache/fetch enforcement,
kernel/processor/thread/IRQ/device/legacy ABI and signed-playable/full-game/
platform requirements remain open.

### Pure far-jump plans and complete code-fetch spans

`gamepkg_x86_far_lookup` validates selector/table bounds before a caller reads
any descriptor bytes. `gamepkg_x86_far_plan_code` then validates a logical LE
code descriptor, privilege/type/presence/offset order and proposes the visible
CS/base/limit/attributes. Return0 is a plan,1 a guest-fault plan and-1 an
unsupported or invalid host input with unchanged output. System descriptors
remain unsupported rather than being assigned fictional direct-code faults.
No CPU, flags, RAM or descriptor is changed. `mark_accessed` requires a future
checked LOCK-semantics descriptor update before cache commit. The separate
`gamepkg_x86_code_fetch_span` checks all1..15 instruction bytes without wrap.
A valid transfer can commit CS before a later crossing-fetch GP; the planner
does not conflate the two faults. Selector-bearing GP/NP/SS errors use the new
explicit segment-fault bridge; raw flags remain distinct from delivered frames.

An independent132,816-record ring0 oracle covers all four privilege levels,
both tables, code/data types,16/32-bit operands/default sizes, base/limit bits
and invalid table/selector cases. GCC11/GCC13 builds reproduce its9,562,764
bytes and SHA25651b8f4b78eb2352e32a721a00331f897ad1c1f711fa2853e6d795ab755e6c24c.
Replay and rejection gates pass host sanitizers, actual Linux32, MIPS32 BE and
two Windows32 compiler profiles under Wine. QEMU omits4,384 clear-A updates and
crossing-fetch faults; its raw bytes stay unchanged and these omissions are
not runtime rules. Native ELF32 confirms committed-CS versus transfer-fault-CS,
one-byte retirement before the next fetch fault and RF1 in delivered frames.
Linux pre-sets A, so clear-to-set writes remain physically unqualified.
The standard suite has five fixed architectural vectors and totals166host/
114Windows passes; strict lint/format pass. Full evidence and reference limits
are in the generator README. Far jumps remain unadmitted pending checked
memory/accessed updates, ownership/cache/context/fetch/retirement integration.
The actual kernel-header startup fault and full playable-runtime work remain.

### Checked far-descriptor resolution

`gamepkg_x86_far_resolve_code` joins selector precheck, checked supervisor
8-byte descriptor read, direct-code validation and conditional locked accessed
update before publishing a proposal. Faults and host rejection preserve the
output; a failed A write preserves RAM. Published `mark_accessed=0` means the
request was serviced, not that a code cache was committed. The caller still
supplies qualified protected32/non-v8086 mode, CPL and real GDTR/LDTR state.
`gamepkg_x86_memory_descriptor_read` stages both LE words, and
`gamepkg_x86_memory_descriptor_accessed` holds the shared RAM lock while
checking/ORing only bit0 of the rights byte. Already set A needs no write in
this P6 profile. Implicit accesses report supervisor PF U/S=0; ordinary memory
retains its previous error bits, and all host capability checks remain.
These are internal processor APIs, not guest source or host GDT access.

The862-case fault/permission/wrap/raw-state fixture and16,000 eight-worker
accessed/CMPXCHG/XADD sequences pass all five platform profiles. Existing RAM/
critical-section fixtures pass host/BE32/two Windows profiles, and standard
suites pass167host/114Windows; strict lint/format pass. All23,958 native
functions partial-link on three targets with55 freshly compiled current
runtime objects per target and verified unchanged body/numerical reuse.
Fresh startup55692364585dce9bb5de5ce5ebcc5df81ba9984fe94c7e6d598dacd565dff14f
reaches the unchanged actual kernel-header fault with prior state preserved.
No live-game descriptor read/update or far-jump execution is claimed.
CPU ownership/cache/context commit, fetch/dispatch/retirement and actual
helper admission remain required alongside the broader playable-runtime work.
Exact reproduction scope, Intel references and prior oracle limitations are
recorded in the generator README.

### Owner-controlled CS/LDTR caches

`gamepkg_x86_cpu_init_code` explicitly configures each thread's logical hidden
CS/LDTR image, with active native ownership and shared table setup required.
There is no default flat cache. `snapshot_execution`/`restore_execution`
coherently capture/restore interrupt state and caches, checking immutable table
identity. Invalid mode/CPL, cache, ownership, pointer or alias input leaves
state/output unchanged. Interrupt-only restore/checkpoints preserve cache
compatibility. Suspension/resumption and worker handoff retain independent
thread images. This partial context excludes PC, other segment caches, current
x87 segment bindings and raw GPR/FP/RAM bindings.

`gamepkg_x86_cpu_far_transfer` now performs owner-context lookup, checked
supervisor descriptor read/locked A resolution and CS-only cache commit without
holding a CPU mutex across memory/fault boundaries. Failures retain the old
cache. The caller must still commit target PC and retire once; flags/LDTR/STI
shadow remain untouched here. `gamepkg_x86_cpu_check_code_span` raises GP0 for
an entire crossing fetch, retaining an already committed new cache. Actual
execute permission/bytes/decode/dispatch and exception frames are separate.

The 141-case synthetic fault/transfer fixture plus configuration/foreign-owner/
alias/mode/commit guards and 1,024 eight-worker context leases pass five
platform profiles with raw pending x87/MMX/XMM state preserved. The prior CPU
16,000-entry matrix also passes all profiles. Full regressions pass 168 host/
114 Windows tests, strict lint/format pass, and CI adds portability/link gates.
Three-target partial modules retain all 23,958 definitions with 56 fresh runtime
objects per target and verified body/numerical reuse. Fresh startup probe
4c7d6ad2ce86d019f07c5e86cd80e74d857dfaa9c8382ede76e5d645fa93356d
preserves prior state and reaches the unchanged kernel-header fault.
No generated far-jump opcode is newly admitted. Scoped code providers, PC/
retirement/fetch integration, current x87-CS binding and actual helper admission
remain next, alongside the broader playable-runtime requirements. Exact scope
and proof boundaries are in the generator README.

### Scoped code-provider dispatch and far JMP

Every newly emitted controlled label calls `gamepkg_x86_dispatch_instruction`
with its qualified byte length. A separate frozen `GamePkgX86DispatchCode`
capability supplies pure CS/CPL/GDTR/LDTR snapshots and a nonyielding checked-CS
commit. The original four-field controller ABI is unchanged. Entry/checkpoint
alias guards cover original/frozen provider storage; nested runs get only their
own capability. Real descriptor faults occur outside callback scope; provider
fault/error/reentry is a host protocol stop, not guest exception retirement.

Exact EA immediate far pointers, with optional unique 66 prefix, resolve
selector/descriptor/A checks before committing CS, target PC and current x87 CS,
then retire once at the source site. Completed state survives retirement
callback rejection. Pending IRQ requests may stop at the target after STI
shadow ends, before target fetch; no IRQ handler is delivered. Newly committed
CS survives a subsequent full-instruction-span fault. Current x87 selector
binding affects future metadata only, preserving past/raw FP state and DS/SS/FS
data selection; nested dispatch restores the outer selector binding.

`gamepkg_x86_dispatch_run_cpu_code_events` opts into explicitly configured,
owner-backed caches. Qualified generated execution remains base-zero/default32;
another valid target profile commits but cannot execute unqualified code.
Legacy entries preserve their prior checkpoint behavior and cannot run a far
transfer without the capability. Fetch-span hooks do not implement executable
RAM/byte integrity, page tables or near CALL/JMP/RET target-limit/stack-commit
fault ordering. Those, production bootstrap and complete contexts remain open.

The generated 130-case CPU fixture and frozen/nested/error/fault/reentry/alias/
retirement gates pass five platform profiles. Existing table/deep-dispatch
matrices pass again; full generator suites pass 171 host/116 Windows tests and
strict lint/format pass. Controlled owned generation admits one actual helper,
reaching 23,959 functions/772,330 entries with 40 rejects and one unbacked extent.
All 808,502 emitted sites are audited for byte-length hooks. All bodies compile
fresh in 83 units plus a map on three targets, with 56 current runtime objects
each and verified numerical-pool reuse. Linux/Windows source/report match.

The unmodified owned helper at 0x001d6aac also executes in an isolated strict
CPU/RAM probe on all five profiles: eleven checked instructions, exact SGDT
bytes, two IF0 descriptor exchanges, CLI/STI, shadow-protected A update and CS
reload, previous-descriptor return/RET8 and pending FP/MMX/XMM preservation.
This synthetic caller setup is not natural game startup or physical CPU proof.
The separate full-map startup regression uses the legacy adapter and still
reaches the same kernel-header fault with prior state/counters preserved.
Kernel/processor/thread/near-transfer/IRQ/device/legacy ABI and signed playable
integration remain required. Full source/probe hashes and exact scope are in
the generator README; readiness remains false.

### Required kernel priority and DPC initialization

`gamepkg_x86_irql.c` connects explicit CPU priority state to a checked supervisor
PCR byte. `cpu_require_irql` opts in before any thread is created; an unbound
thread cannot retire guest code. Initialization requires an active owner with
explicit flat/default32 CPL0 code, writes PASSIVE(0) once, and retains the exact
borrowed memory identity. CPU/thread and RAM lifetimes remain host obligations.
Legacy domains retain their existing behavior. The older partial execution
snapshot/restore does not overwrite priority or make an elevated owner eligible
for an ordinary wait.

The by-value service returns old/current levels, rejects invalid raises/lowers
and levels outside0..31 before mutation, and checks the guest byte against the
trusted value. Neither a guest mirror edit nor EFLAGS.IF determines scheduling
priority. IRQL>=2 inhibits ordinary yield/block; APC1 remains schedulable.
Suspend retains the reservation at any level. Services do not change raw GPR,
x87/MMX/XMM, flags, STI shadow or instruction retirement. A host HLE adapter
handles UCHAR/fastcall/stdcall and the separately checked near return.

`gamepkg_x86_dispatch_run_cpu_kernel_events` requires explicit code/tables and
bound priority state. It activates the retained mirror inside the live fault
boundary, verifies it at subsequent instruction/REP boundaries, and synchronizes
it after scheduling. All CPU metadata locks are released before RAM access or
translated code. Older dispatch entries reject required-priority domains.
Host stop-request polling still follows IF/STI; it does not acknowledge or
deliver a device IRQ, and priority2 does not imply all hardware IRQs are masked.
This activation handles priority only; a production scheduler must also preserve
and activate guest-mutated NT_TIB/SEH/TLS/current-thread/kernel-stack state.

`gamepkg_x86_dpc_initialize` initializes the28-byte logical little-endian KDPC
record, with checked Type/Inserted/routine/context writes. Padding, list links
and system arguments are preserved. Both selected spans are admitted before
any write; read-only untouched fields need no write permission. Wrapping spans,
wrong bindings/profiles and faults cannot partially initialize the record.
Callers must not reinitialize an already queued object. Queue insertion/removal,
priority delivery and actual DPC callback execution remain separate work.

The public interface/ordinal references are [nxdk's CC0 header](https://github.com/XboxDev/nxdk/blob/master/lib/xboxkrnl/xboxkrnl.h)
and [export definitions](https://github.com/XboxDev/nxdk/blob/master/lib/xboxkrnl/xboxkrnl.exe.def).
The implementation uses checked GamePKG memory and logical guest addresses;
there is no guest-to-native function-pointer cast or native IRQL operation.

`tools/check_takedawn_kernel_priority.sh` qualifies generated guest reads,
unexpected mirror edits, priority direction/range, IF-independent inhibition,
suspend/resume, partial-context restore, memory/profile failures, transactional
DPC initialization with untouched read-only fields, and eight-worker shared-PCR
activation. Each of five profiles passes1,104 checked transitions and1,024
worker activations: ASan/UB/leak host, actual Linux32 UB, MIPS32 big-endian/QEMU,
GCC Windows32/Wine and mixed Clang-fixture/GCC-runtime Windows32/Wine. Pending
FP/MMX/XMM banks are preserved. This is component portability evidence, not
physical Windows/original Pentium3/full-game big-endian qualification.

### Transactional event and inactive-timer initialization

`gamepkg_x86_dispatcher_object.c` supplies checked logical little-endian event
and timer initialization for the explicit flat/default32 CPL0 host profile.
Both require an active owner, exact supervisor RAM binding and a live fault
boundary. Each complete logical record must be representable without wrapping.
The host API accepts only notification/synchronization types and a normalized
event Boolean; the import adapter separately normalizes the guest UCHAR.

A16-byte event receives its Type,Size,SignalState and self-linked wait head;
unused Absolute/Inserted bytes are preserved. A40-byte inactive timer receives
Type8/9, Absolute0, Size10, Inserted0, SignalState0, an empty wait head, zero due
time/timer links/period. Its Dpc field is preserved for a subsequent set operation.
Only selected fields need write admission; untouched fields can be read-only or
unmapped. Write-only selected fields also work. All selected spans are checked
before the first store, so an absent or protected final byte cannot leave an
initialized header with an unfinished tail. RAM maps/permissions are immutable
and participating guest access retains exclusive CPU ownership; unrelated host
writers are outside this transaction guarantee.

Initialization changes no GPR, x87/MMX/XMM, priority, IF/shadow, instruction count
or native handle. It neither arms a timer nor signals a waiting worker. Live
object reinitialization with waiters or a queued timer is invalid. Set/reset/
pulse/cancel/wait/expiry/DPC queue delivery remain separate services. In
particular these initializers must not be used to claim functioning timer expiry
or event wait semantics.

`tools/check_takedawn_dispatcher_object.sh` passes752 transactional cases and
eight generated header reads on each of five profiles: sanitized host, actual
Linux32, MIPS32 big-endian/QEMU, GCC Windows32/Wine and mixed Clang-fixture/
GCC-runtime Windows32/Wine. Cases deny each byte separately, preserve unused
fields, admit write-only destinations, check high addresses/whole-record wrap,
reject invalid types/states and wrong bindings/profiles, and preserve pending
FP/MMX/XMM plus priority at levels0/1/2/31. The original invalid fixture tried to
register a zero-permission RAM region; the RAM API correctly rejected it.
Denied bytes are now actual gaps or read-only regions. Linux and Windows fixture
bytes match SHA256
`17a95c033cffafc0705c1cee66d722040fad9f60c0aaf6b16de327521a0a755b`.
Public layout/API references remain [nxdk's CC0 kernel header](https://github.com/XboxDev/nxdk/blob/master/lib/xboxkrnl/xboxkrnl.h)
and its [ordinal definitions](https://github.com/XboxDev/nxdk/blob/master/lib/xboxkrnl/xboxkrnl.exe.def).
This is component qualification, not full-game endianness, physical Windows or
original-console execution proof.

### Frozen virtual-console settings queries

`gamepkg_x86_nv.c` provides the checked `ExQueryNonVolatileSetting` service for
an explicit logical virtual-console profile. Creation copies reviewed host
values into owned immutable storage. Duplicate/unknown indices, malformed
native spans, unsupported registry types and invalid payload lengths are
rejected. Each payload fits the physical 256-byte EEPROM format; that bound is
not a query-output or execution quota. Empty profiles are explicit, with no
implicit host-language/timezone/identity lookup or default values.

The query requires active ownership in its exact borrowed CPU domain, explicit
flat/default32 CPL0 code, the exact bound supervisor RAM and a live fault
boundary. A known but unconfigured public setting returns host status -2 and
must stop integration. An unknown index returns guest OBJECT_NAME_NOT_FOUND
without output access. A short buffer returns BUFFER_TOO_SMALL and writes only
optional ResultLength. Success writes ResultLength, Type, then a zero-filled
ValueLength buffer overlaid with the frozen payload. Output aliases follow
that order. All selected writes are admitted before any mutation, so a late
fault cannot leave partial metadata or data. Whole selected pointer spans may
not wrap; invalid spans stop as host protocol errors. Tail clearing uses
512-byte chunks without a total byte/iteration quota. Participating guest work
retains exclusive CPU ownership; unrelated host arena writers are outside this
transaction guarantee. No native lock remains held over a fault.

The service changes no GPR, FP/MMX/XMM, flags, priority, retirement or native
handle. HLE separately handles the five stack arguments, NTSTATUS, checked
return and retirement. CPU/RAM are borrowed and must outlive the store and all
queries; release requires quiescent host references. This is a logical profile,
not raw EEPROM checksum/decryption emulation, settings persistence or a
completed `ExSaveNonVolatileSetting` implementation. Those remain required.

`tools/check_takedawn_nv.sh` passes 3,686 state/fault/alias cases, 11 constructor
rejections and a generated guest output read on each of five profiles:
ASan/UB/leak host, actual Linux32 UB, MIPS32 big-endian/QEMU, GCC Windows32/Wine,
and mixed Clang-fixture/GCC-runtime Windows32/Wine. Cases cover frozen source
mutation, DWORD/binary sizes, short/exact/oversized buffers, multi-chunk clearing,
512-byte boundaries, all 512 small output-overlap combinations, each selected
byte absent/read-only/write-only, high addresses, wrap rejection, required-null
faults, optional ResultLength, missing known versus unknown indices, ownership,
profile/binding guards and priority 0/1/2/31. Pending FP/MMX/XMM state is retained.
Linux and Windows generated fixture bytes match SHA256
`b0a3c9d8d39b79c1827e194fa18cda63ca2772be58b01f7c685dbbcbd8410635`.

The interface/indices come from [nxdk's CC0 header](https://github.com/XboxDev/nxdk/blob/master/lib/xboxkrnl/xboxkrnl.h).
The query's status and output-order compatibility reference is
[Cxbx-Reloaded's implementation](https://github.com/Cxbx-Reloaded/Cxbx-Reloaded/blob/master/src/core/kernel/exports/EmuKrnlEx.cpp).
This is GamePKG's checked virtual-kernel behavior, not a physical retail-kernel
oracle for every invalid pointer, overlong buffer or console EEPROM revision.

### Portable NTSTATUS conversion

`gamepkg_x86_status_to_dos` implements scalar error conversion with the complete
pinned Wine 9.0 mapping profile: 1,586 entries in 43 ranges, customer-code
preservation, HRESULT-from-NT normalization, encoded Win32 codes and an explicit
unknown-code result. Runtime code uses logical 32-bit integers and generated
constants; it does not depend on Windows DLLs or host TEB state. HLE still owns
the checked stdcall-one import/return. See [status/README.md](status/README.md)
for original source provenance, regeneration, LGPL source/relink obligations,
reference/oracle separation and retail-kernel qualification limits.

The native ntdll oracle and independently compiled pinned source agree for all
730,816 cases. The native-generated corpus replays on five profiles with
pending guest FP/MMX/XMM/GPR state preserved. Native GCC/mixed Clang corpora and
the original-source corpus match SHA256
`8fee9d8937127372b77ee40bc66fc20d31ae7619cddefe3825b54a33f3d63f7e`.
The opcode/generator runtime remains separate from the maintainer oracle; no
native ntdll call or original reference routine is linked into the player.
