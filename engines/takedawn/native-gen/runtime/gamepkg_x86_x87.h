#ifndef GAMEPKG_X86_X87_H
#define GAMEPKG_X86_X87_H
#include <stdint.h>
#include <limits.h>
#if CHAR_BIT != 8
#error GamePKG x86 services require eight-bit bytes
#endif
/* Host-owned architectural bits. Physical registers, not ST-relative lanes.
 * Padding/layout is not a guest save-image format. Never expose a host pointer
 * to guest code. Loads, arithmetic, comparisons and stores have separate
 * services. Legacy protected-mode environment save/restore has its own service. */
typedef struct GamePkgX87State {
    uint64_t significand[8];
    uint16_t sign_exponent[8];
    uint16_t control,status,tags;
    uint32_t instruction_pointer,data_pointer;
    uint16_t opcode,instruction_selector,data_selector;
} GamePkgX87State;
GamePkgX87State *gamepkg_x86_x87_state(void);
/* op0=F2XM1,1=FYL2X,2=FPATAN,3=FSIN,4=FCOS,5=FSINCOS,6=FPTAN,7=FYL2XP1. Binary ops1/2/7 store ST1 then pop;5/6 push. Shared state, old pending before metadata, deferred commit rules. */
void gamepkg_x86_x87_trans(uint32_t op,uint32_t ip,uint32_t opcode,uint32_t ebp);
/* Protected-mode control/images: op0=FLDCW,1=FLDENV,2=FRSTOR,
 * 3=FNSTENV,4=FNSAVE. word16 chooses14/94 rather than28/108 byte images.
 * No-wait stores preserve pending flags in the image, then mask/reset. */
void gamepkg_x86_x87_environment(uint32_t op,uint32_t word16,uint32_t address,
    uint32_t ip,uint32_t ebp);
/* Reviewed host thread binding. DS/SS are flat; the emitter supplies the
 * current FS base when selecting a memory operand. These are selectors and
 * metadata inputs, not segment-descriptor or permission bypasses. */
void gamepkg_x86_x87_bind_segments(uint32_t cs,uint32_t ds,uint32_t ss,uint32_t fs);
/* Current code-selector binding for future x87 instruction metadata only.
 * Never changes an already recorded FIP/FCS/FDP/FDS or physical FP/MMX bank,
 * data-segment selection or native host segment. Trusted host inputs only. */
void gamepkg_x86_x87_code_segment(uint32_t cs);
uint32_t gamepkg_x86_x87_current_code_segment(void);
void gamepkg_x86_x87_data_segment(uint32_t kind,uint32_t base);
void gamepkg_x86_x87_record(GamePkgX87State *state,uint32_t ip,uint32_t opcode);
void gamepkg_x86_x87_record_data(GamePkgX87State *state,uint32_t address);
/* op0=FABS,1=FCHS,2=FXCH,3=FFREE,4=FINCSTP,5=FDECSTP,6=FXAM,
 * 7=FNOP,8=FRNDINT,9=FXTRACT,10..17=FCMOV B/E/BE/U/NB/NE/NBE/NU.
 * Logical index for exchange/free/move. Flags are read-only guest EFLAGS. */
void gamepkg_x86_x87_stack(uint32_t op,uint32_t index,uint32_t eflags,
    uint32_t ip,uint32_t opcode,uint32_t ebp);
/* Stores: kind0=binary32,1=binary64,2=raw80,3=signed16,4=signed32,
 * 5=signed64,6=ST(destination),7=packed BCD80 (FBSTP).
 * Whole memory admission precedes new FP
 * status/metadata. Unmasked IE/OE/UE suppress stores/pops; PE does not.
 * Old pending delivery and logical segment-span checks precede this call. */
void gamepkg_x86_x87_store(uint32_t kind,uint32_t destination,uint32_t pop,
    uint32_t ip,uint32_t opcode,uint32_t address,uint32_t ebp);
uint32_t gamepkg_x86_x87_control(void);
uint32_t gamepkg_x86_x87_status(void);
uint64_t gamepkg_x86_mmx_read(uint32_t reg);
/* Register arithmetic under the measured four-PC host profile. Logical
 * stack operands, optional reversal and pop; op5 is FSCALE with fixed
 * destination0/source1, no reversal/pop, ignoring PC. op6/7=FPREM/FPREM1
 * share fixed operands and exact quotient flags, independently of PC/RC.
 * Old pending faults precede new
 * metadata. Masks select deferred pre/post responses and wrapped ranges. */
void gamepkg_x86_x87_arithmetic(uint32_t op,uint32_t destination,uint32_t source,
    uint32_t reverse,uint32_t pop,uint32_t ip,uint32_t opcode,uint32_t ebp);
/* Memory arithmetic: op0..3 and optional reversal, ST0 destination, no pop.
 * kind0=binary32,1=binary64,2=signed16,3=signed32. Emitter waits on old
 * exceptions before admitting the complete RAM read; low/high are staged
 * little-endian words. IEEE source denormals retain DE after normalization. */
void gamepkg_x86_x87_memory_arithmetic(uint32_t op,uint32_t reverse,uint32_t kind,
    uint32_t low,uint32_t high,uint32_t ip,uint32_t opcode,uint32_t address,uint32_t ebp);
/* Compare ST0 with kind0=ST(source),1=zero,2=binary32,3=binary64,
 * 4=signed16,5=signed32. pop0..2; quiet1 permits quiet NaNs. integer_flags
 * selects EFLAGS instead of C0/C2/C3. Returns the updated supplied flags.
 * Memory inputs must be completely staged after the initial pending check. */
uint32_t gamepkg_x86_x87_compare(uint32_t kind,uint32_t source,uint32_t pop,
    uint32_t quiet,uint32_t integer_flags,uint32_t flags,uint32_t low,uint32_t high,
    uint32_t ip,uint32_t opcode,uint32_t address,uint32_t ebp);
/* kind0/1/2: binary32/binary64/raw80;3/4/5: signed16/32/64;6: ST(low);
 * 7..13: FLD1/L2T/L2E/PI/LG2/LN2/Z;14: packed BCD80 (FBLD).
 * Memory inputs are fully staged before
 * this call. The emitter checks old pending faults before its first read. */
void gamepkg_x86_x87_load(uint32_t kind,uint32_t low,uint32_t high,uint32_t upper,
    uint32_t ip,uint32_t opcode,uint32_t address,uint32_t ebp);
void gamepkg_x86_x87_init(void);
void gamepkg_x86_x87_clear(void);
void gamepkg_x86_x87_wait(uint32_t eip,uint32_t ebp);
void gamepkg_x86_mmx_commit(uint32_t reg,uint64_t value);
void gamepkg_x86_mmx_read_commit(void);
void gamepkg_x86_mmx_empty(void);
/* Pure lane shifts: op0=left logical,1=right logical,2=right arithmetic.
 * Width16/32/64, except arithmetic64 is not an MMX instruction. Counts are
 * full unsigned64 values; oversized counts zero or sign-fill each lane. */
uint64_t gamepkg_x86_mmx_shift(uint64_t value,uint64_t count,uint32_t op,uint32_t width);
/* Pure SSE-era MMX lanes:0/1=rounded unsigned average8/16,2/3=min/max
 * unsigned8,4/5=min/max signed16,6=sum absolute byte differences,7=shuffle,
 * 8=insert word,9=extract word,10=byte sign mask. Control is the encoded
 * imm8 for7..9 (insert/extract ignore its upper six bits), zero otherwise. */
uint64_t gamepkg_x86_mmx_lanes(uint64_t a,uint64_t b,uint32_t op,uint32_t control);
/* Paired signed32/binary32 conversion, operation0/1/2 as scalar conversion.
 * enter_mmx=0 only for CVTPI2PS m64. The emitter checks pending x87 errors
 * before staged reads in MMX forms; the service enters before SIMD faults. */
uint64_t gamepkg_x86_convert_mmx(uint64_t value,uint32_t operation,
    uint32_t enter_mmx,uint32_t eip,uint32_t ebp);
/* op0=AND,1=ANDN,2=OR,3=XOR,4=ADD,5=SUB,6=EQ,7=signed GT.
 * 8/9=signed/unsigned saturating ADD,10/11=signed/unsigned saturating SUB;
 * 12/13/14=low/signed-high/unsigned-high word multiply,15=word multiply-add;
 * 16=signed pack,17=unsigned-byte pack,18/19=low/high unpack.
 * Lane width is8/16/32 (input width for pack); saturation uses8/16, multiply
 * uses16 and signed pack uses16/32. Bitwise operators ignore lane width. */
uint64_t gamepkg_x86_mmx_binary(uint64_t left,uint64_t right,uint32_t op,uint32_t width);
#endif
