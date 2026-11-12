#include "gamepkg_x86_cpu_code.h"
#include "gamepkg_x86_far_memory.h"
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_memory.h"
GamePkgX86NearReturn gamepkg_x86_cpu_plan_return(
    GamePkgX86CpuThread *thread, uint32_t esp, uint32_t bytes, uint32_t extra,
    uint32_t eip, uint32_t ebp) {
  GamePkgX86CpuExecution state;
  if ((bytes != 2 && bytes != 4) || extra > 65535 ||
      gamepkg_x86_cpu_snapshot_execution(thread, &state) || state.code.cs_base ||
      !(state.code.cs_attributes & 0x400u))
    return (GamePkgX86NearReturn){-1, 0, 0};
  if (esp > UINT32_MAX - (bytes - 1))
    gamepkg_x86_stack_segment(eip, ebp);
  uint32_t words[4];
  gamepkg_x86_memory_read(esp, words, bytes, eip, ebp);
  if (words[0] > state.code.cs_limit)
    gamepkg_x86_general_protection(eip, ebp);
  return (GamePkgX86NearReturn){0, words[0], esp + bytes + extra};
}
int gamepkg_x86_cpu_far_transfer(GamePkgX86CpuThread *thread, uint32_t selector,
                                 uint32_t offset, uint32_t operand16,
                                 uint32_t eip, uint32_t ebp) {
  GamePkgX86CpuExecution state;
  if (gamepkg_x86_cpu_snapshot_execution(thread, &state))
    return -1;
  const GamePkgX86FarTables tables = {
      state.tables.gdtr.base, state.tables.gdtr.limit, state.code.ldtr_base,
      state.code.ldtr_limit, state.code.ldtr_usable};
  GamePkgX86CodeTarget target;
  if (gamepkg_x86_far_resolve_code(tables, selector, offset,
                                   state.interrupt.cpl, operand16, eip, ebp,
                                   &target))
    return -1;
  return gamepkg_x86_cpu_commit_code(thread, &target);
}
int gamepkg_x86_cpu_check_code_span(GamePkgX86CpuThread *thread,
                                    uint32_t offset, uint32_t bytes,
                                    uint32_t ebp) {
  GamePkgX86CpuExecution state;
  if (gamepkg_x86_cpu_snapshot_execution(thread, &state))
    return -1;
  const GamePkgX86CodeTarget code = {.selector = state.code.cs_selector,
                                     .base = state.code.cs_base,
                                     .limit = state.code.cs_limit,
                                     .attributes = state.code.cs_attributes};
  int status = gamepkg_x86_code_fetch_span(code, offset, bytes);
  if (status == 1)
    gamepkg_x86_segment_fault(13, 0, offset, ebp);
  return status;
}
