#ifndef GAMEPKG_X86_TABLE_STORE_H
#define GAMEPKG_X86_TABLE_STORE_H
#include "gamepkg_x86_cpu.h"
/* Same checked legacy image transaction, using an already-reviewed value.
 * Invalid size/limit returns-1 before RAM; no CPU state is selected here. */
int gamepkg_x86_store_table_image(GamePkgX86TableRegister value,
                                  uint32_t operand16, uint32_t address,
                                  uint32_t eip, uint32_t ebp);
/* Host service for legacy protected32, pre-UMIP SGDT/SIDT semantics. op0=GDTR,
 * op1=IDTR; operand16 is boolean. 32-bit form stores all base bits; the legacy
 * 16-bit form stores base[23:0] and clears the sixth byte. Always six LE bytes.
 * The active owner and explicit shared CPU configuration are mandatory.
 * Returns-1 before RAM access for invalid host arguments, unconfigured CPU or
 * unqualified real/v8086 mode; no invented guest fault. Under a live guest
 * fault boundary, complete checked RAM admission precedes any store. This
 * linear-address service does NOT do the emitter's segment-span check, retire
 * an instruction, admit an opcode, select a segment or change table registers.
 * The caller still owes exact encoding, segment validation and retirement.
 * No native SGDT/SIDT/LGDT/LIDT or host table address is used. */
int gamepkg_x86_cpu_store_table(GamePkgX86CpuThread *thread, uint32_t op,
                                uint32_t operand16, uint32_t address,
                                uint32_t eip, uint32_t ebp);
#endif
