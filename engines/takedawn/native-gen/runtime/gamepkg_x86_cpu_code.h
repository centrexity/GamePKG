#ifndef GAMEPKG_X86_CPU_CODE_H
#define GAMEPKG_X86_CPU_CODE_H
#include "gamepkg_x86_cpu.h"
/* Active-owner legacy direct-code far transfer: snapshot explicitly configured
 * mode/CPL/GDTR/LDTR, resolve the descriptor and locked A update, then commit
 * only CS. No mutex is held over a guest memory/fault boundary. No callback,
 * CPU handoff, implicit initialization, native segment load or quota occurs.
 * Return0=CS committed,-1=invalid/unconfigured/unsupported host operation;
 * selector/page faults leave via the mandatory live boundary before commit.
 * Caller MUST commit the truncated target PC and retire exactly once before
 * the next instruction/event. This service does neither and does not claim
 * target fetch, gate/task/real/v8086/long-mode execution or handler delivery.
 */
int gamepkg_x86_cpu_far_transfer(GamePkgX86CpuThread *thread, uint32_t selector,
                                 uint32_t offset, uint32_t operand16,
                                 uint32_t eip, uint32_t ebp);
/* Check the entire1..15-byte instruction against the current hidden CS cache.
 * offset is architectural EIP, not base+EIP.
 * Return0=allowed,-1=bad/unconfigured host state. A crossing span raises GP0 at
 * offset with newly committed CS retained. This does NOT check RAM execute
 * capability/bytes/decode/map or retire anything. Active ownership and a live
 * fault boundary are required. */
int gamepkg_x86_cpu_check_code_span(GamePkgX86CpuThread *thread,
                                    uint32_t offset, uint32_t bytes,
                                    uint32_t ebp);
/* Checked flat/default32 near return for trusted host import adapters outside
 * a dispatcher frame. No register, PC, memory, CPU or retirement commit occurs.
 * An active owner and explicitly configured code/tables are mandatory. Invalid
 * host arguments/profile return status=-1 BEFORE RAM; real SS/PF/target GP
 * exits the caller's live fault boundary. Stack read precedes target check.
 * On status0 the caller must commit offset/esp without rereading the stack,
 * then apply its host-call retirement contract before dispatch resumes.
 * This does not qualify general SS caches, privilege returns or RETF/IRET. */
typedef struct GamePkgX86NearReturn {
  int status;
  uint32_t offset, esp;
} GamePkgX86NearReturn;
GamePkgX86NearReturn gamepkg_x86_cpu_plan_return(
    GamePkgX86CpuThread *thread, uint32_t esp, uint32_t bytes, uint32_t extra,
    uint32_t eip, uint32_t ebp);
#endif
