#ifndef GAMEPKG_X86_IRQL_H
#define GAMEPKG_X86_IRQL_H
#include "gamepkg_x86_cpu.h"
/* Trusted host service, not guest pointer ABI. Call in a live fault boundary
 * with active flat/default32 CPL0 owner and exact supervisor RAM binding.
 * Initialize checks writable PCR mirror then commits PASSIVE(0) once.
 * op0=get,1=raise,2=lower; levels0..31. Invalid direction/range rejects before
 * RAM access. A mismatched PCR byte returns-2 without mutation. Wrong host
 * domain/profile/op returns-1. RAM faults propagate without a metadata lock.
 * No GPR/FP/IF/shadow/retirement or device request is changed; HLE handles
 * fastcall/UCHAR/stdcall and checked return separately. Mutable guest mirror
 * is checked against trusted CPU state; it never controls scheduler priority.
 * CPU/memory are borrowed and must outlive the initialized thread. */
typedef struct GamePkgX86IrqlResult {
  int status;
  uint32_t previous, level;
} GamePkgX86IrqlResult;
int gamepkg_x86_irql_initialize(GamePkgX86CpuThread *thread,
                                GamePkgX86Memory *memory, uint32_t address,
                                uint32_t eip, uint32_t ebp);
GamePkgX86IrqlResult gamepkg_x86_irql(GamePkgX86CpuThread *thread, uint32_t op,
                                      uint32_t level, uint32_t eip,
                                      uint32_t ebp);
#endif
