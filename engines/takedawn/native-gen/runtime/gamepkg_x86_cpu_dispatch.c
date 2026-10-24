#include "gamepkg_x86_cpu_dispatch.h"
#include "gamepkg_x86_memory.h"
extern _Thread_local uint32_t g_eflags;
typedef struct CpuDispatch {
  GamePkgX86CpuThread *thread;
  const GamePkgX86CpuEventSource *events;
  int with_irql, first;
} CpuDispatch;
static int priority(CpuDispatch *run, uint32_t pc, uint32_t ebp, int sync) {
  if (!run->with_irql)
    return 0;
  GamePkgX86CpuIrql image;
  if (gamepkg_x86_cpu_snapshot_irql(run->thread, &image) || !image.required ||
      !image.configured || !gamepkg_x86_memory_is_bound(image.memory, 0))
    return -1;
  uint32_t word[4] = {image.level, 0, 0, 0};
  if (sync)
    gamepkg_x86_memory_write(image.address, word, 1, 1, 0, pc, ebp);
  else {
    gamepkg_x86_memory_read(image.address, word, 1, pc, ebp);
    if (word[0] != image.level)
      return -1;
  }
  return 0;
}
static int boundary(void *opaque, uint32_t pc, uint32_t ebp) {
  CpuDispatch *run = opaque;
  if (priority(run, pc, ebp, run->first) ||
      gamepkg_x86_cpu_checkpoint(run->thread, g_eflags))
    return -1;
  run->first = 0;
  if (run->events) {
    GamePkgX86InterruptState image;
    int kind = gamepkg_x86_dispatch_checkpoint_kind();
    if (kind < 0 || gamepkg_x86_cpu_snapshot(run->thread, &image))
      return -1;
    int eligible = gamepkg_x86_interrupt_eligible(&image);
    if (eligible < 0)
      return -1;
    if (eligible) {
      int pending = run->events->poll(run->events->opaque, &image,
                                      (uint32_t)kind, pc, ebp);
      if (pending)
        return pending == 1 ? 1 : -1;
    }
  }
  int result = gamepkg_x86_cpu_yield(run->thread);
  return result < 0 ? -1 : priority(run, pc, ebp, result == 0);
  /* Inhibited CPU handoff continues this owner. */
}
static int retire(void *opaque, uint32_t pc, uint32_t ebp, uint32_t event) {
  const CpuDispatch *run = opaque;
  (void)pc;
  (void)ebp;
  if (event == GAMEPKG_X86_RETIRE_CLI || event == GAMEPKG_X86_RETIRE_STI) {
    GamePkgX86InterruptState image;
    return gamepkg_x86_cpu_check_owner(run->thread) ||
                   gamepkg_x86_cpu_snapshot(run->thread, &image) ||
                   image.flags != g_eflags
               ? -1
               : 0;
  }
  if (event > GAMEPKG_X86_RETIRE_STI)
    return -1;
  return gamepkg_x86_cpu_retire_other(run->thread, g_eflags);
}
static int interrupt_step(void *opaque, uint32_t op, uint32_t *flags) {
  const CpuDispatch *run = opaque;
  int result = gamepkg_x86_cpu_step(run->thread, g_eflags, op);
  if (!result) {
    GamePkgX86InterruptState image;
    if (gamepkg_x86_cpu_snapshot(run->thread, &image))
      return -1;
    *flags = image.flags;
  }
  return result;
}
static int table_register(void *opaque, uint32_t op, uint32_t *base,
                          uint32_t *limit) {
  const CpuDispatch *run = opaque;
  GamePkgX86Tables tables;
  GamePkgX86InterruptState state;
  if (op > 1 || gamepkg_x86_cpu_snapshot_tables(run->thread, &tables) ||
      gamepkg_x86_cpu_snapshot(run->thread, &state) || !state.protected_mode ||
      (state.flags & 0x20000u))
    return -1;
  const GamePkgX86TableRegister value = op ? tables.idtr : tables.gdtr;
  *base = value.base;
  *limit = value.limit;
  return 0;
}
static int code_read(void *opaque, GamePkgX86DispatchCodeImage *out) {
  const CpuDispatch *run = opaque;
  GamePkgX86CpuExecution state;
  if (gamepkg_x86_cpu_snapshot_execution(run->thread, &state))
    return -1;
  *out = (GamePkgX86DispatchCodeImage){
      .tables = {state.tables.gdtr.base, state.tables.gdtr.limit,
                 state.code.ldtr_base, state.code.ldtr_limit,
                 state.code.ldtr_usable},
      .code = {.selector = state.code.cs_selector,
               .base = state.code.cs_base,
               .limit = state.code.cs_limit,
               .attributes = state.code.cs_attributes},
      .cpl = state.interrupt.cpl};
  return 0;
}
static int code_commit(void *opaque, const GamePkgX86CodeTarget *target) {
  const CpuDispatch *run = opaque;
  return gamepkg_x86_cpu_commit_code(run->thread, target);
}
static int overlap(const void *a, size_t an, const void *b, size_t bn) {
  uintptr_t av = (uintptr_t)a, bv = (uintptr_t)b;
  return av <= bv ? bv - av < an : av - bv < bn;
}
static int run_cpu_events(const GamePkgX86DispatchEntry *map, size_t count,
                          uint32_t pc, GamePkgX86DispatchExit *result,
                          const GamePkgX86FpSource *source,
                          GamePkgX86CpuThread *thread,
                          const GamePkgX86CpuEventSource *events, int with_code,
                          int with_irql) {
  GamePkgX86InterruptState state;
  GamePkgX86CpuIrql irql;
  if (gamepkg_x86_cpu_check_owner(thread) ||
      gamepkg_x86_cpu_snapshot(thread, &state) || state.flags != g_eflags ||
      gamepkg_x86_cpu_snapshot_irql(thread, &irql) ||
      (irql.required && !with_irql) ||
      (with_irql && (!irql.required || !irql.configured ||
                     !gamepkg_x86_memory_is_bound(irql.memory, 0))))
    return -1;
  if (events && ((uintptr_t)events % _Alignof(GamePkgX86CpuEventSource) ||
                 (uintptr_t)events > UINTPTR_MAX - (sizeof(*events) - 1) ||
                 !events->poll ||
                 overlap(events, sizeof(*events), result, sizeof(*result)) ||
                 (source && !gamepkg_x86_fp_source_compatible(
                                source, events, sizeof(*events)))))
    return -1;
  const GamePkgX86CpuEventSource frozen =
      events ? *events : (GamePkgX86CpuEventSource){0};
  CpuDispatch run = {thread, events ? &frozen : NULL, with_irql, 1};
  const GamePkgX86DispatchControl control = {(void *)&run, boundary, retire,
                                             interrupt_step};
  const GamePkgX86DispatchTables tables = {(void *)&run, table_register};
  const GamePkgX86DispatchCode code = {(void *)&run, code_read, code_commit};
  if (with_code) {
    GamePkgX86CpuExecution configured;
    if (gamepkg_x86_cpu_snapshot_execution(thread, &configured))
      return -1;
    return gamepkg_x86_dispatch_run_controlled_code(
        map, count, pc, result, source, &control, &tables, &code);
  }
  return gamepkg_x86_dispatch_run_controlled_tables(map, count, pc, result,
                                                    source, &control, &tables);
}
int gamepkg_x86_dispatch_run_cpu_events(
    const GamePkgX86DispatchEntry *map, size_t count, uint32_t pc,
    GamePkgX86DispatchExit *result, const GamePkgX86FpSource *source,
    GamePkgX86CpuThread *thread, const GamePkgX86CpuEventSource *events) {
  return run_cpu_events(map, count, pc, result, source, thread, events, 0, 0);
}
int gamepkg_x86_dispatch_run_cpu_code_events(
    const GamePkgX86DispatchEntry *map, size_t count, uint32_t pc,
    GamePkgX86DispatchExit *result, const GamePkgX86FpSource *source,
    GamePkgX86CpuThread *thread, const GamePkgX86CpuEventSource *events) {
  return run_cpu_events(map, count, pc, result, source, thread, events, 1, 0);
}
int gamepkg_x86_dispatch_run_cpu_kernel_events(
    const GamePkgX86DispatchEntry *map, size_t count, uint32_t pc,
    GamePkgX86DispatchExit *result, const GamePkgX86FpSource *source,
    GamePkgX86CpuThread *thread, const GamePkgX86CpuEventSource *events) {
  return run_cpu_events(map, count, pc, result, source, thread, events, 1, 1);
}
int gamepkg_x86_dispatch_run_cpu(const GamePkgX86DispatchEntry *map,
                                 size_t count, uint32_t pc,
                                 GamePkgX86DispatchExit *result,
                                 const GamePkgX86FpSource *source,
                                 GamePkgX86CpuThread *thread) {
  return gamepkg_x86_dispatch_run_cpu_events(map, count, pc, result, source,
                                             thread, NULL);
}
