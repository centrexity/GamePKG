#include "gamepkg_x86_dispatch.h"
#include "gamepkg_x86_memory.h"
#include "gamepkg_x86_table_store.h"
#include <stdlib.h>
#include <string.h>
extern _Thread_local uint32_t g_ebp, g_esp, g_fs_base, g_eflags;
_Thread_local uint32_t gamepkg_x86_dispatch_pc;
typedef struct DispatchFrame {
  struct DispatchFrame *previous;
  void (*block)(uint32_t);
  uint32_t entry, target, action;
  const GamePkgX86DispatchControl *control, *original_control;
  const GamePkgX86FpSource *source, *original_source;
  const GamePkgX86DispatchTables *tables, *original_tables;
  const GamePkgX86DispatchCode *code, *original_code;
  const GamePkgX86DispatchEntry *map;
  GamePkgX86DispatchExit *output;
  size_t map_bytes;
  uint32_t callback_stage, checkpoint_kind;
} DispatchFrame;
static _Thread_local DispatchFrame *active;
static int span(const void *p, size_t n, size_t alignment) {
  return n && p && (uintptr_t)p % alignment == 0 &&
         (uintptr_t)p <= UINTPTR_MAX - (n - 1);
}
static int overlap(const void *a, size_t an, const void *b, size_t bn) {
  uintptr_t av = (uintptr_t)a, bv = (uintptr_t)b;
  return av <= bv ? bv - av < an : av - bv < bn;
}
static void invoke(void) { active->block(active->entry); }
int gamepkg_x86_dispatch_site(uint32_t *pc, uint32_t *ebp) {
  if (!active || !span(pc, 4, _Alignof(uint32_t)) ||
      !span(ebp, 4, _Alignof(uint32_t)) || overlap(pc, 4, ebp, 4))
    return -1;
  *pc = gamepkg_x86_dispatch_pc;
  *ebp = g_ebp;
  return 0;
}
void gamepkg_x86_dispatch_transfer(uint32_t target, uint32_t ebp) {
  if (!active)
    abort();
  if (active->action) {
    active->action = 3;
    return;
  }
  g_ebp = ebp;
  active->target = target;
  active->action = 1;
}
void gamepkg_x86_dispatch_stop(uint32_t ebp) {
  if (!active)
    abort();
  if (active->action) {
    active->action = 3;
    return;
  }
  g_ebp = ebp;
  active->action = 2;
}
void gamepkg_x86_dispatch_invalid(void) {
  if (!active)
    abort();
  active->action = 3;
}
static int run(const GamePkgX86DispatchEntry *map, size_t count, uint32_t pc,
               GamePkgX86DispatchExit *result, const GamePkgX86FpSource *source,
               const GamePkgX86DispatchControl *control,
               const GamePkgX86DispatchTables *tables,
               const GamePkgX86DispatchCode *code) {
  if (!count || count > 1048576 ||
      !span(map, count * sizeof(*map), _Alignof(GamePkgX86DispatchEntry)) ||
      !span(result, sizeof(*result), _Alignof(GamePkgX86DispatchExit)) ||
      overlap(map, count * sizeof(*map), result, sizeof(*result)) ||
      (source &&
       !gamepkg_x86_fp_source_compatible(source, result, sizeof(*result))))
    return -1;
  for (size_t i = 0; i < count; ++i)
    if (!map[i].block || (i && map[i - 1].address >= map[i].address))
      return -1;
  if (control &&
      (!span(control, sizeof(*control), _Alignof(GamePkgX86DispatchControl)) ||
       overlap(control, sizeof(*control), result, sizeof(*result)) ||
       !control->boundary || !control->retire))
    return -1;
  if (tables &&
      (!control ||
       !span(tables, sizeof(*tables), _Alignof(GamePkgX86DispatchTables)) ||
       overlap(tables, sizeof(*tables), result, sizeof(*result)) ||
       overlap(tables, sizeof(*tables), map, count * sizeof(*map)) ||
       overlap(tables, sizeof(*tables), control, sizeof(*control)) ||
       (source &&
        !gamepkg_x86_fp_source_compatible(source, tables, sizeof(*tables))) ||
       !tables->read))
    return -1;
  if (code && (!control || !tables ||
               !span(code, sizeof(*code), _Alignof(GamePkgX86DispatchCode)) ||
               overlap(code, sizeof(*code), result, sizeof(*result)) ||
               overlap(code, sizeof(*code), map, count * sizeof(*map)) ||
               overlap(code, sizeof(*code), control, sizeof(*control)) ||
               overlap(code, sizeof(*code), tables, sizeof(*tables)) ||
               (source && !gamepkg_x86_fp_source_compatible(source, code,
                                                            sizeof(*code))) ||
               !code->read || !code->commit))
    return -1;
  const GamePkgX86DispatchCode frozen_code =
      code ? *code : (GamePkgX86DispatchCode){0};
  const GamePkgX86DispatchTables frozen_tables =
      tables ? *tables : (GamePkgX86DispatchTables){0};
  const GamePkgX86DispatchControl frozen =
      control ? *control : (GamePkgX86DispatchControl){0};
  const GamePkgX86FpSource *original_source = source;
  GamePkgX86FpSource bindings = source ? *source : (GamePkgX86FpSource){0};
  if (source)
    source = &bindings;
  DispatchFrame frame = {.previous = active,
                         .control = control ? &frozen : NULL,
                         .original_control = control,
                         .tables = tables ? &frozen_tables : NULL,
                         .original_tables = tables,
                         .code = code ? &frozen_code : NULL,
                         .original_code = code,
                         .source = source,
                         .original_source = original_source,
                         .map = map,
                         .output = result,
                         .map_bytes = count * sizeof(*map)};
  uint32_t previous_pc = gamepkg_x86_dispatch_pc;
  uint32_t previous_cs = gamepkg_x86_x87_current_code_segment();
  memset(result, 0, sizeof(*result));
  active = &frame;
  int reason;
  for (;;) {
    result->pc = pc;
    size_t lo = 0, hi = count;
    while (lo < hi) {
      size_t mid = lo + (hi - lo) / 2;
      if (map[mid].address < pc)
        lo = mid + 1;
      else
        hi = mid;
    }
    if (lo == count || map[lo].address != pc) {
      reason = 2;
      break;
    }
    frame.block = map[lo].block;
    frame.entry = pc;
    frame.action = 0;
    gamepkg_x86_dispatch_pc = pc;
    int fault = source ? gamepkg_x86_run_with_fp(invoke, &result->fault, source)
                       : gamepkg_x86_run(invoke, &result->fault.core);
    if (fault) {
      if (fault < 0) {
        reason = 3;
        break;
      }
      result->pc = result->fault.core.eip;
      if (frame.callback_stage) {
        reason = 3;
        break;
      }
      frame.callback_stage = 2;
      reason = frame.control &&
                       (frame.control->retire(frame.control->opaque, result->pc,
                                              result->fault.core.ebp,
                                              GAMEPKG_X86_RETIRE_FAULT) ||
                        frame.action)
                   ? 3
                   : 1;
      break;
    }
    if (frame.action == 1) {
      pc = frame.target;
      continue;
    }
    result->pc = gamepkg_x86_dispatch_pc;
    reason = frame.action == 2 ? 0 : frame.action == 4 ? 4 : 3;
    break;
  }
  active = frame.previous;
  gamepkg_x86_dispatch_pc = previous_pc;
  if (frame.previous)
    gamepkg_x86_x87_code_segment(previous_cs);
  return reason;
}
int gamepkg_x86_dispatch_run(const GamePkgX86DispatchEntry *map, size_t count,
                             uint32_t pc, GamePkgX86DispatchExit *result,
                             const GamePkgX86FpSource *source) {
  return run(map, count, pc, result, source, NULL, NULL, NULL);
}
int gamepkg_x86_dispatch_run_controlled(
    const GamePkgX86DispatchEntry *map, size_t count, uint32_t pc,
    GamePkgX86DispatchExit *result, const GamePkgX86FpSource *source,
    const GamePkgX86DispatchControl *control) {
  if (!control)
    return -1;
  return run(map, count, pc, result, source, control, NULL, NULL);
}
int gamepkg_x86_dispatch_run_controlled_tables(
    const GamePkgX86DispatchEntry *map, size_t count, uint32_t pc,
    GamePkgX86DispatchExit *result, const GamePkgX86FpSource *source,
    const GamePkgX86DispatchControl *control,
    const GamePkgX86DispatchTables *tables) {
  if (!control || !tables)
    return -1;
  return run(map, count, pc, result, source, control, tables, NULL);
}
int gamepkg_x86_dispatch_run_controlled_code(
    const GamePkgX86DispatchEntry *map, size_t count, uint32_t pc,
    GamePkgX86DispatchExit *result, const GamePkgX86FpSource *source,
    const GamePkgX86DispatchControl *control,
    const GamePkgX86DispatchTables *tables,
    const GamePkgX86DispatchCode *code) {
  if (!control || !tables || !code)
    return -1;
  return run(map, count, pc, result, source, control, tables, code);
}
static int checkpoint(uint32_t pc, uint32_t ebp, uint32_t kind) {
  if (!active)
    abort();
  if (active->action || !active->control || active->callback_stage) {
    active->action = 3;
    return 1;
  }
  gamepkg_x86_dispatch_pc = pc;
  g_ebp = ebp;
  active->callback_stage = 1;
  active->checkpoint_kind = kind;
  int result = active->control->boundary(active->control->opaque, pc, ebp);
  active->callback_stage = 0;
  if (active->action) {
    active->action = 3;
    return 1;
  }
  if (!result)
    return 0;
  active->action = result == 1 ? 4 : 3;
  return 1;
}
int gamepkg_x86_dispatch_boundary(uint32_t pc, uint32_t ebp) {
  return checkpoint(pc, ebp, GAMEPKG_X86_CHECKPOINT_INSTRUCTION);
}
int gamepkg_x86_dispatch_rep_checkpoint(uint32_t pc, uint32_t ebp) {
  return checkpoint(pc, ebp, GAMEPKG_X86_CHECKPOINT_REP);
}
int gamepkg_x86_dispatch_checkpoint_kind(void) {
  return active && active->callback_stage == 1 && !active->action
             ? (int)active->checkpoint_kind
             : -1;
}
int gamepkg_x86_dispatch_capture_checkpoint(GamePkgX86FpFault *out) {
  if (gamepkg_x86_dispatch_checkpoint_kind() < 0 || !active->source ||
      !span(out, sizeof(*out), _Alignof(GamePkgX86FpFault)) ||
      overlap(out, sizeof(*out), active, sizeof(*active)) ||
      overlap(out, sizeof(*out), active->map, active->map_bytes) ||
      overlap(out, sizeof(*out), active->output, sizeof(*active->output)) ||
      overlap(out, sizeof(*out), active->control, sizeof(*active->control)) ||
      overlap(out, sizeof(*out), active->original_source,
              sizeof(*active->original_source)) ||
      overlap(out, sizeof(*out), active->original_control,
              sizeof(*active->original_control)))
    return -1;
  if (active->tables &&
      (overlap(out, sizeof(*out), active->tables, sizeof(*active->tables)) ||
       overlap(out, sizeof(*out), active->original_tables,
               sizeof(*active->original_tables))))
    return -1;
  if (active->code &&
      (overlap(out, sizeof(*out), active->code, sizeof(*active->code)) ||
       overlap(out, sizeof(*out), active->original_code,
               sizeof(*active->original_code))))
    return -1;
  return gamepkg_x86_capture_registers(gamepkg_x86_dispatch_pc, g_ebp, out,
                                       active->source);
}
static int retire_at(uint32_t ebp, uint32_t event, uint32_t site) {
  if (!active)
    abort();
  if (active->action || !active->control || active->callback_stage) {
    active->action = 3;
    return 1;
  }
  g_ebp = ebp;
  active->callback_stage = 2;
  int result =
      active->control->retire(active->control->opaque, site, ebp, event);
  active->callback_stage = 0;
  if (result || active->action) {
    active->action = 3;
    return 1;
  }
  return 0;
}
static int retire_event(uint32_t ebp, uint32_t event) {
  return retire_at(ebp, event, gamepkg_x86_dispatch_pc);
}
int gamepkg_x86_dispatch_retire(uint32_t ebp) {
  return retire_event(ebp, GAMEPKG_X86_RETIRE_OTHER);
}
int gamepkg_x86_dispatch_interrupt(uint32_t op, uint32_t ebp) {
  if (!active)
    abort();
  if (active->action || !active->control || active->callback_stage || op > 1 ||
      !active->control->interrupt_step) {
    active->action = 3;
    return 1;
  }
  g_ebp = ebp;
  uint32_t flags = g_eflags;
  active->callback_stage = 3;
  int result =
      active->control->interrupt_step(active->control->opaque, op, &flags);
  active->callback_stage = 0;
  if (active->action || (result != 0 && result != 1)) {
    active->action = 3;
    return 1;
  }
  if (result == 1)
    gamepkg_x86_general_protection(gamepkg_x86_dispatch_pc, ebp);
  g_eflags = flags;
  return retire_event(ebp, GAMEPKG_X86_RETIRE_CLI + op);
}
int gamepkg_x86_dispatch_store_table(uint32_t op, uint32_t operand16,
                                     uint32_t offset, uint32_t segment,
                                     uint32_t ebp) {
  if (!active)
    abort();
  if (active->action || !active->control || !active->tables ||
      active->callback_stage || op > 1 || operand16 > 1 || segment > 2) {
    active->action = 3;
    return 1;
  }
  g_ebp = ebp;
  GamePkgX86TableRegister value = {0};
  active->callback_stage = 3;
  int status = active->tables->read(active->tables->opaque, op, &value.base,
                                    &value.limit);
  active->callback_stage = 0;
  if (active->action || status || value.limit > 65535) {
    active->action = 3;
    return 1;
  }
  if (offset > UINT32_MAX - 5u) {
    if (segment == 1)
      gamepkg_x86_stack_segment(gamepkg_x86_dispatch_pc, ebp);
    gamepkg_x86_general_protection(gamepkg_x86_dispatch_pc, ebp);
  }
  if (gamepkg_x86_store_table_image(value, operand16,
                                    offset + (segment == 2 ? g_fs_base : 0u),
                                    gamepkg_x86_dispatch_pc, ebp)) {
    active->action = 3;
    return 1;
  }
  return retire_event(ebp, GAMEPKG_X86_RETIRE_OTHER);
}
static int read_code(GamePkgX86DispatchCodeImage *image) {
  if (active->action || !active->control || !active->code ||
      active->callback_stage) {
    active->action = 3;
    return 1;
  }
  *image = (GamePkgX86DispatchCodeImage){0};
  active->callback_stage = 3;
  int status = active->code->read(active->code->opaque, image);
  active->callback_stage = 0;
  if (status || active->action || image->tables.gdt_limit > 65535 ||
      image->tables.ldt_usable > 1 || image->cpl > 3 ||
      image->cpl != (image->code.selector & 3) || image->code.offset ||
      image->code.mark_accessed || !(image->code.attributes & 1) ||
      gamepkg_x86_code_fetch_span(image->code, 0, 1) != 0) {
    active->action = 3;
    return 1;
  }
  return 0;
}
int gamepkg_x86_dispatch_check_target(uint32_t target, uint32_t eip,
                                      uint32_t ebp) {
  if (!active)
    abort();
  if (active->action || active->callback_stage) {
    active->action = 3;
    return 1;
  }
  if (!active->code)
    return 0;
  GamePkgX86DispatchCodeImage image;
  if (read_code(&image))
    return 1;
  if (image.code.base || !(image.code.attributes & 0x400u)) {
    active->action = 3;
    return 1;
  }
  if (target > image.code.limit)
    gamepkg_x86_general_protection(eip, ebp);
  return 0;
}
int gamepkg_x86_dispatch_instruction(uint32_t pc, uint32_t bytes,
                                     uint32_t ebp) {
  if (!active)
    abort();
  if (!bytes || bytes > 15) {
    active->action = 3;
    return 1;
  }
  if (checkpoint(pc, ebp, GAMEPKG_X86_CHECKPOINT_INSTRUCTION))
    return 1;
  if (!active->code)
    return 0;
  GamePkgX86DispatchCodeImage image;
  if (read_code(&image))
    return 1;
  if (image.code.base || !(image.code.attributes & 0x400u)) {
    active->action = 3; /* Generated decoder/profile is flat default32. */
    return 1;
  }
  gamepkg_x86_x87_code_segment(image.code.selector);
  if (gamepkg_x86_code_fetch_span(image.code, pc, bytes) == 1)
    gamepkg_x86_general_protection(pc, ebp);
  return 0;
}
void gamepkg_x86_dispatch_far_jump(uint32_t selector, uint32_t offset,
                                   uint32_t operand16, uint32_t ebp) {
  if (!active)
    abort();
  if (selector > 65535 || operand16 > 1) {
    active->action = 3;
    return;
  }
  GamePkgX86DispatchCodeImage image;
  if (read_code(&image))
    return;
  if (image.code.base || !(image.code.attributes & 0x400u)) {
    active->action = 3;
    return;
  }
  uint32_t site = gamepkg_x86_dispatch_pc;
  g_ebp = ebp;
  GamePkgX86CodeTarget next;
  if (gamepkg_x86_far_resolve_code(image.tables, selector, offset, image.cpl,
                                   operand16, site, ebp, &next)) {
    active->action = 3;
    return;
  }
  active->callback_stage = 3;
  int status = active->code->commit(active->code->opaque, &next);
  active->callback_stage = 0;
  if (status || active->action) {
    active->action = 3;
    return;
  }
  uint32_t target = next.base + next.offset;
  gamepkg_x86_dispatch_pc = target;
  gamepkg_x86_x87_code_segment(next.selector);
  int stopped = retire_at(ebp, GAMEPKG_X86_RETIRE_OTHER, site);
  gamepkg_x86_x87_code_segment(next.selector);
  if (!stopped)
    gamepkg_x86_dispatch_transfer(target, ebp);
}
uint32_t gamepkg_x86_dispatch_load(uint32_t offset, uint32_t segment,
                                   uint32_t eip, uint32_t ebp) {
  if (segment > 2)
    abort();
  if (offset > UINT32_MAX - 3u) {
    if (segment == 1)
      gamepkg_x86_stack_segment(eip, ebp);
    gamepkg_x86_general_protection(eip, ebp);
  }
  uint32_t words[4];
  gamepkg_x86_memory_read(offset + (segment == 2 ? g_fs_base : 0u), words, 4,
                          eip, ebp);
  return words[0];
}

void gamepkg_x86_dispatch_call(uint32_t return_pc, uint32_t eip, uint32_t ebp) {
  uint32_t at = g_esp - 4u;
  if (at > UINT32_MAX - 3u)
    gamepkg_x86_stack_segment(eip, ebp);
  uint32_t words[4] = {return_pc, 0, 0, 0};
  gamepkg_x86_memory_write(at, words, 4, 15, 0, eip, ebp);
  g_esp = at;
}
int gamepkg_x86_dispatch_call_to(uint32_t target, uint32_t return_pc,
                                 uint32_t eip, uint32_t ebp) {
  if (!active)
    abort();
  if (active->action || active->callback_stage) {
    active->action = 3;
    return 1;
  }
  /* Native near CALL admits the stack write before rejecting the target,
   * but neither the return slot nor ESP changes on that rejection. */
  uint32_t at = g_esp - 4u;
  if (at > UINT32_MAX - 3u)
    gamepkg_x86_stack_segment(eip, ebp);
  gamepkg_x86_memory_check(at, 4, 1, eip, ebp);
  if (gamepkg_x86_dispatch_check_target(target, eip, ebp))
    return 1;
  gamepkg_x86_dispatch_call(return_pc, eip, ebp);
  return 0;
}
uint32_t gamepkg_x86_dispatch_ret(uint32_t bytes, uint32_t extra, uint32_t eip,
                                  uint32_t ebp) {
  if ((bytes != 2 && bytes != 4) || extra > 65535)
    abort();
  if (g_esp > UINT32_MAX - (bytes - 1))
    gamepkg_x86_stack_segment(eip, ebp);
  uint32_t words[4];
  gamepkg_x86_memory_read(g_esp, words, bytes, eip, ebp);
  if (active && active->code &&
      gamepkg_x86_dispatch_check_target(words[0], eip, ebp))
    return 0;
  g_esp += bytes + extra;
  return words[0];
}
