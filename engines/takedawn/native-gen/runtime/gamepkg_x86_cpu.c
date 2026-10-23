#include "gamepkg_x86_cpu.h"
#include <limits.h>
#include <stdlib.h>
#ifdef _WIN32
#include <windows.h>
typedef DWORD Worker;
static Worker self(void) { return GetCurrentThreadId(); }
static int same(Worker a, Worker b) { return a == b; }
#else
#include <pthread.h>
typedef pthread_t Worker;
static Worker self(void) { return pthread_self(); }
static int same(Worker a, Worker b) { return pthread_equal(a, b); }
#endif
struct GamePkgX86CpuThread {
  GamePkgX86Cpu *cpu;
  struct GamePkgX86CpuThread *next;
  GamePkgX86InterruptState state;
  GamePkgX86CpuCodeState code;
  Worker worker;
  unsigned active, code_ready, irql_ready;
  uint32_t irql, irql_address;
  GamePkgX86Memory *irql_memory;
};
struct GamePkgX86Cpu {
#ifdef _WIN32
  SRWLOCK mutex;
  CONDITION_VARIABLE changed;
#else
  pthread_mutex_t mutex;
  pthread_cond_t changed;
#endif
  unsigned references;
  uint32_t waiting;
  GamePkgX86CpuThread *owner, *head, *tail;
  GamePkgX86Tables tables;
  unsigned tables_ready, threads_created, irql_required;
};
static int code_valid(const GamePkgX86CpuCodeState *code,
                      const GamePkgX86InterruptState *state) {
  uint32_t a = code->cs_attributes, cpl = state->cpl, dpl = (a >> 5) & 3;
  if (!state->protected_mode || (state->flags & 0x20000u) ||
      code->cs_selector > 65535 || !(code->cs_selector & 0xfffcu) ||
      (code->cs_selector & 3) != cpl || (a & ~0xfffu) || (a & 0x99u) != 0x99u ||
      ((a & 4) ? dpl > cpl : dpl != cpl) ||
      ((a & 0x800u) ? ((code->cs_limit & 4095) != 4095)
                    : code->cs_limit > 0xfffff) ||
      code->ldtr_selector > 65535 || code->ldtr_usable > 1 ||
      (code->ldtr_attributes & ~0xfffu))
    return 0;
  if (!code->ldtr_usable)
    return !(code->ldtr_selector & 0xfffcu);
  return (code->ldtr_selector & 0xfff8u) && !(code->ldtr_selector & 4) &&
         (code->ldtr_attributes & 0x9fu) == 0x82u &&
         ((code->ldtr_attributes & 0x800u) ? ((code->ldtr_limit & 4095) == 4095)
                                           : code->ldtr_limit <= 0xfffff);
}
int gamepkg_x86_cpu_owner_in_domain(GamePkgX86CpuThread *t, GamePkgX86Cpu *c) {
  return !t || !c || t->cpu != c ? -1 : gamepkg_x86_cpu_check_owner(t);
}
static void lock(GamePkgX86Cpu *c) {
#ifdef _WIN32
  AcquireSRWLockExclusive(&c->mutex);
#else
  if (pthread_mutex_lock(&c->mutex))
    abort();
#endif
}
static void unlock(GamePkgX86Cpu *c) {
#ifdef _WIN32
  ReleaseSRWLockExclusive(&c->mutex);
#else
  if (pthread_mutex_unlock(&c->mutex))
    abort();
#endif
}
static void changed(GamePkgX86Cpu *c) {
#ifdef _WIN32
  WakeAllConditionVariable(&c->changed);
#else
  if (pthread_cond_broadcast(&c->changed))
    abort();
#endif
}
static void wait(GamePkgX86Cpu *c) {
#ifdef _WIN32
  if (!SleepConditionVariableSRW(&c->changed, &c->mutex, INFINITE, 0))
    abort();
#else
  if (pthread_cond_wait(&c->changed, &c->mutex))
    abort();
#endif
}
GamePkgX86Cpu *gamepkg_x86_cpu_create(void) {
  GamePkgX86Cpu *c = calloc(1, sizeof(*c));
  if (!c)
    return NULL;
#ifdef _WIN32
  InitializeSRWLock(&c->mutex);
  InitializeConditionVariable(&c->changed);
#else
  if (pthread_mutex_init(&c->mutex, NULL)) {
    free(c);
    return NULL;
  }
  if (pthread_cond_init(&c->changed, NULL)) {
    pthread_mutex_destroy(&c->mutex);
    free(c);
    return NULL;
  }
#endif
  c->references = 1;
  return c;
}
void gamepkg_x86_cpu_release(GamePkgX86Cpu *c) {
  if (!c)
    return;
  lock(c);
  if (!c->references)
    abort();
  unsigned remaining = --c->references;
  if (!remaining && (c->owner || c->head || c->waiting))
    abort();
  unlock(c);
  if (!remaining) {
#ifndef _WIN32
    if (pthread_cond_destroy(&c->changed) || pthread_mutex_destroy(&c->mutex))
      abort();
#endif
    free(c);
  }
}
GamePkgX86CpuThread *
gamepkg_x86_cpu_thread_create(GamePkgX86Cpu *c,
                              const GamePkgX86InterruptState *initial) {
  if (!c || gamepkg_x86_interrupt_eligible(initial) < 0)
    return NULL;
  const GamePkgX86InterruptState image = *initial;
  GamePkgX86CpuThread *t = calloc(1, sizeof(*t));
  if (!t)
    return NULL;
  lock(c);
  if (c->references == UINT_MAX) {
    unlock(c);
    free(t);
    return NULL;
  }
  ++c->references;
  c->threads_created = 1;
  t->cpu = c;
  t->state = image;
  unlock(c);
  return t;
}
int gamepkg_x86_cpu_thread_destroy(GamePkgX86CpuThread *t) {
  if (!t)
    return -1;
  GamePkgX86Cpu *c = t->cpu;
  lock(c);
  if (t->active || c->owner == t) {
    unlock(c);
    return -1;
  }
  unlock(c);
  free(t);
  gamepkg_x86_cpu_release(c);
  return 0;
}
static int owns(GamePkgX86CpuThread *t) {
  return t->active && t->cpu->owner == t && same(t->worker, self());
}
static void acquire(GamePkgX86CpuThread *t) {
  GamePkgX86Cpu *c = t->cpu;
  if (c->waiting == UINT32_MAX)
    abort();
  t->next = NULL;
  if (c->tail)
    c->tail->next = t;
  else
    c->head = t;
  c->tail = t;
  ++c->waiting;
  while (c->owner || c->head != t)
    wait(c);
  c->head = t->next;
  if (!c->head)
    c->tail = NULL;
  t->next = NULL;
  --c->waiting;
  c->owner = t;
}
int gamepkg_x86_cpu_enter(GamePkgX86CpuThread *t) {
  if (!t)
    return -1;
  GamePkgX86Cpu *c = t->cpu;
  lock(c);
  if (t->active || (c->owner && c->owner != t && owns(c->owner))) {
    unlock(c);
    return -1;
  }
  t->active = 1;
  t->worker = self();
  if (c->owner != t)
    acquire(t);
  unlock(c);
  return 0;
}
static int detach(GamePkgX86CpuThread *t, int block) {
  if (!t)
    return -1;
  GamePkgX86Cpu *c = t->cpu;
  lock(c);
  if (!owns(t)) {
    unlock(c);
    return -1;
  }
  if (block && (!gamepkg_x86_interrupt_eligible(&t->state) ||
                (t->irql_ready && t->irql >= 2))) {
    unlock(c);
    return 1;
  }
  t->active = 0;
  if (block) {
    c->owner = NULL;
    changed(c);
  }
  unlock(c);
  return 0;
}
int gamepkg_x86_cpu_suspend(GamePkgX86CpuThread *t) { return detach(t, 0); }
int gamepkg_x86_cpu_block(GamePkgX86CpuThread *t) { return detach(t, 1); }
int gamepkg_x86_cpu_yield(GamePkgX86CpuThread *t) {
  if (!t)
    return -1;
  GamePkgX86Cpu *c = t->cpu;
  lock(c);
  if (!owns(t)) {
    unlock(c);
    return -1;
  }
  if (!gamepkg_x86_interrupt_eligible(&t->state) ||
      (t->irql_ready && t->irql >= 2)) {
    unlock(c);
    return 1;
  }
  if (c->head) {
    c->owner = NULL;
    changed(c);
    acquire(t);
  }
  unlock(c);
  return 0;
}
static int transition(GamePkgX86CpuThread *t, uint32_t flags, uint32_t op) {
  if (!t)
    return -1;
  GamePkgX86Cpu *c = t->cpu;
  lock(c);
  if (!owns(t)) {
    unlock(c);
    return -1;
  }
  GamePkgX86InterruptState next = t->state;
  if (c->irql_required && !t->irql_ready) {
    unlock(c);
    return -1;
  }
  next.flags = flags;
  if (t->code_ready && !code_valid(&t->code, &next)) {
    unlock(c);
    return -1;
  }
  int result = op == 3   ? (gamepkg_x86_interrupt_eligible(&next) < 0 ? -1 : 0)
               : op == 2 ? gamepkg_x86_interrupt_retire_other(&next)
                         : gamepkg_x86_interrupt_step(&next, op);
  if (!result)
    t->state = next;
  unlock(c);
  return result;
}
int gamepkg_x86_cpu_step(GamePkgX86CpuThread *t, uint32_t flags, uint32_t op) {
  if (op > 1)
    return -1;
  return transition(t, flags, op);
}
int gamepkg_x86_cpu_retire_other(GamePkgX86CpuThread *t, uint32_t flags) {
  return transition(t, flags, 2);
}
int gamepkg_x86_cpu_checkpoint(GamePkgX86CpuThread *t, uint32_t flags) {
  return transition(t, flags, 3);
}
static int overlaps(const void *a, size_t an, const void *b, size_t bn) {
  uintptr_t av = (uintptr_t)a, bv = (uintptr_t)b;
  return av <= bv ? bv - av < an : av - bv < bn;
}
int gamepkg_x86_cpu_snapshot(GamePkgX86CpuThread *t,
                             GamePkgX86InterruptState *out) {
  if (!t || !out || (uintptr_t)out % _Alignof(GamePkgX86InterruptState) ||
      (uintptr_t)out > UINTPTR_MAX - (sizeof(*out) - 1))
    return -1;
  if (overlaps(out, sizeof(*out), t, sizeof(*t)) ||
      overlaps(out, sizeof(*out), t->cpu, sizeof(*t->cpu)))
    return -1;
  lock(t->cpu);
  *out = t->state;
  unlock(t->cpu);
  return 0;
}
int gamepkg_x86_cpu_restore(GamePkgX86CpuThread *t,
                            const GamePkgX86InterruptState *image) {
  if (!t || gamepkg_x86_interrupt_eligible(image) < 0)
    return -1;
  const GamePkgX86InterruptState next = *image;
  GamePkgX86Cpu *c = t->cpu;
  lock(c);
  if (!owns(t) || (t->code_ready && !code_valid(&t->code, &next))) {
    unlock(c);
    return -1;
  }
  t->state = next;
  unlock(c);
  return 0;
}
uint32_t gamepkg_x86_cpu_waiters(GamePkgX86Cpu *c) {
  if (!c)
    return 0;
  lock(c);
  uint32_t result = c->waiting;
  unlock(c);
  return result;
}
int gamepkg_x86_cpu_check_owner(GamePkgX86CpuThread *t) {
  if (!t)
    return -1;
  lock(t->cpu);
  int result = owns(t) ? 0 : -1;
  unlock(t->cpu);
  return result;
}

int gamepkg_x86_cpu_init_tables(GamePkgX86Cpu *c,
                                const GamePkgX86Tables *tables) {
  if (!c || !tables || (uintptr_t)tables % _Alignof(GamePkgX86Tables) ||
      (uintptr_t)tables > UINTPTR_MAX - (sizeof(*tables) - 1) ||
      overlaps(tables, sizeof(*tables), c, sizeof(*c)))
    return -1;
  const GamePkgX86Tables image = *tables;
  if (image.gdtr.limit > 65535 || image.idtr.limit > 65535)
    return -1;
  lock(c);
  if (c->tables_ready || c->threads_created) {
    unlock(c);
    return -1;
  }
  c->tables = image;
  c->tables_ready = 1;
  unlock(c);
  return 0;
}
int gamepkg_x86_cpu_snapshot_tables(GamePkgX86CpuThread *t,
                                    GamePkgX86Tables *out) {
  if (!t || !out || (uintptr_t)out % _Alignof(GamePkgX86Tables) ||
      (uintptr_t)out > UINTPTR_MAX - (sizeof(*out) - 1) ||
      overlaps(out, sizeof(*out), t, sizeof(*t)) ||
      overlaps(out, sizeof(*out), t->cpu, sizeof(*t->cpu)))
    return -1;
  GamePkgX86Cpu *c = t->cpu;
  lock(c);
  if (!owns(t) || !c->tables_ready) {
    unlock(c);
    return -1;
  }
  *out = c->tables;
  unlock(c);
  return 0;
}
static int host_span(GamePkgX86CpuThread *t, const void *p, size_t bytes) {
  return t && p && (uintptr_t)p % _Alignof(uint32_t) == 0 &&
         (uintptr_t)p <= UINTPTR_MAX - (bytes - 1) &&
         !overlaps(p, bytes, t, sizeof(*t)) &&
         !overlaps(p, bytes, t->cpu, sizeof(*t->cpu));
}
int gamepkg_x86_cpu_require_irql(GamePkgX86Cpu *c) {
  if (!c)
    return -1;
  lock(c);
  if (c->threads_created || c->irql_required) {
    unlock(c);
    return -1;
  }
  c->irql_required = 1;
  unlock(c);
  return 0;
}
int gamepkg_x86_cpu_snapshot_irql(GamePkgX86CpuThread *t,
                                  GamePkgX86CpuIrql *out) {
  if (!host_span(t, out, sizeof(*out)) ||
      (uintptr_t)out % _Alignof(GamePkgX86CpuIrql))
    return -1;
  GamePkgX86Cpu *c = t->cpu;
  lock(c);
  if (!owns(t)) {
    unlock(c);
    return -1;
  }
  *out = (GamePkgX86CpuIrql){c->irql_required, t->irql_ready, t->irql,
                             t->irql_address, t->irql_memory};
  unlock(c);
  return 0;
}
static int kernel_profile(GamePkgX86CpuThread *t) {
  return t->cpu->irql_required && t->code_ready && !t->state.cpl &&
         t->state.protected_mode && !(t->state.flags & 0x20000u) &&
         !t->code.cs_base && (t->code.cs_attributes & 0x400u);
}
int gamepkg_x86_cpu_bind_irql(GamePkgX86CpuThread *t, GamePkgX86Memory *m,
                              uint32_t address) {
  if (!t || !m)
    return -1;
  GamePkgX86Cpu *c = t->cpu;
  lock(c);
  if (!owns(t) || !kernel_profile(t) || t->irql_ready) {
    unlock(c);
    return -1;
  }
  t->irql_memory = m;
  t->irql_address = address;
  t->irql = 0;
  t->irql_ready = 1;
  unlock(c);
  return 0;
}
int gamepkg_x86_cpu_commit_irql(GamePkgX86CpuThread *t, uint32_t expected,
                                uint32_t next) {
  if (!t || next > 31)
    return -1;
  GamePkgX86Cpu *c = t->cpu;
  lock(c);
  if (!owns(t) || !kernel_profile(t) || !t->irql_ready || t->irql != expected) {
    unlock(c);
    return -1;
  }
  t->irql = next;
  unlock(c);
  return 0;
}
int gamepkg_x86_cpu_init_code(GamePkgX86CpuThread *t,
                              const GamePkgX86CpuCodeState *code) {
  if (!host_span(t, code, sizeof(*code)))
    return -1;
  const GamePkgX86CpuCodeState image = *code;
  GamePkgX86Cpu *c = t->cpu;
  lock(c);
  if (!owns(t) || !c->tables_ready || t->code_ready ||
      !code_valid(&image, &t->state)) {
    unlock(c);
    return -1;
  }
  t->code = image;
  t->code_ready = 1;
  unlock(c);
  return 0;
}
int gamepkg_x86_cpu_snapshot_execution(GamePkgX86CpuThread *t,
                                       GamePkgX86CpuExecution *out) {
  if (!host_span(t, out, sizeof(*out)))
    return -1;
  GamePkgX86Cpu *c = t->cpu;
  lock(c);
  if (!owns(t) || !c->tables_ready || !t->code_ready) {
    unlock(c);
    return -1;
  }
  *out = (GamePkgX86CpuExecution){t->state, c->tables, t->code};
  unlock(c);
  return 0;
}
int gamepkg_x86_cpu_restore_execution(GamePkgX86CpuThread *t,
                                      const GamePkgX86CpuExecution *input) {
  if (!host_span(t, input, sizeof(*input)))
    return -1;
  const GamePkgX86CpuExecution image = *input;
  if (gamepkg_x86_interrupt_eligible(&image.interrupt) < 0 ||
      !code_valid(&image.code, &image.interrupt))
    return -1;
  GamePkgX86Cpu *c = t->cpu;
  lock(c);
  if (!owns(t) || !c->tables_ready || !t->code_ready ||
      image.tables.gdtr.base != c->tables.gdtr.base ||
      image.tables.gdtr.limit != c->tables.gdtr.limit ||
      image.tables.idtr.base != c->tables.idtr.base ||
      image.tables.idtr.limit != c->tables.idtr.limit) {
    unlock(c);
    return -1;
  }
  t->state = image.interrupt;
  t->code = image.code;
  unlock(c);
  return 0;
}
int gamepkg_x86_cpu_commit_code(GamePkgX86CpuThread *t,
                                const GamePkgX86CodeTarget *target) {
  if (!host_span(t, target, sizeof(*target)))
    return -1;
  const GamePkgX86CodeTarget next = *target;
  if (next.vector || next.error || next.mark_accessed ||
      next.offset > next.limit)
    return -1;
  GamePkgX86Cpu *c = t->cpu;
  lock(c);
  GamePkgX86CpuCodeState code = t->code;
  code.cs_selector = next.selector;
  code.cs_base = next.base;
  code.cs_limit = next.limit;
  code.cs_attributes = next.attributes;
  if (!owns(t) || !c->tables_ready || !t->code_ready ||
      !code_valid(&code, &t->state)) {
    unlock(c);
    return -1;
  }
  t->code = code;
  unlock(c);
  return 0;
}
