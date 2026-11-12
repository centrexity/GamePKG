#include "gamepkg_x86_shutdown.h"
#include "gamepkg_x86_fault.h"
#include <stdlib.h>
typedef struct Node {
  struct Node *prev, *next;
  uint32_t address, priority;
} Node;
struct GamePkgX86Shutdown {
  GamePkgX86Cpu *cpu;
  GamePkgX86Memory *memory;
  uint32_t head, initialized, phase, prepared, return_pc, return_esp, active_pc;
  uint64_t sequence;
  Node *first, *last, *active;
};
static int domain(GamePkgX86Shutdown *s, GamePkgX86CpuThread *t) {
  return !s || gamepkg_x86_cpu_owner_in_domain(t, s->cpu) ||
                 !gamepkg_x86_memory_is_bound(s->memory, 0)
             ? -1
             : 0;
}
static int owner(GamePkgX86Shutdown *s, GamePkgX86CpuThread *t) {
  GamePkgX86CpuExecution e;
  return domain(s, t) || gamepkg_x86_cpu_snapshot_execution(t, &e) ||
                 e.interrupt.cpl || e.code.cs_base ||
                 !(e.code.cs_attributes & 0x400u)
             ? -1
             : 0;
}
static uint32_t link(GamePkgX86Shutdown *s, Node *n) {
  return n ? n->address + 8u : s->head;
}
static int check(GamePkgX86Shutdown *s, Node *n, uint32_t ip, uint32_t bp) {
  uint32_t w[4];
  gamepkg_x86_memory_read(n ? n->address + 8 : s->head, w, 8, ip, bp);
  return w[0] != link(s, n ? n->next : s->first) ||
                 w[1] != link(s, n ? n->prev : s->last)
             ? -2
             : 0;
}
static void store(uint32_t a, uint32_t value, uint32_t ip, uint32_t bp) {
  uint32_t w[4] = {value, 0, 0, 0};
  gamepkg_x86_memory_write(a, w, 4, 15, 0, ip, bp);
}
static int overlaps(uint32_t a, uint32_t an, uint32_t b, uint32_t bn) {
  return (uint64_t)a + an > b && (uint64_t)b + bn > a;
}
static int remove_prepare(GamePkgX86Shutdown *s, Node *n, uint32_t ip,
                          uint32_t bp) {
  if (check(s, NULL, ip, bp) || check(s, n, ip, bp) ||
      (n->prev && check(s, n->prev, ip, bp)) ||
      (n->next && check(s, n->next, ip, bp)))
    return -2;
  gamepkg_x86_memory_check(link(s, n->prev), 4, 1, ip, bp);
  gamepkg_x86_memory_check(link(s, n->next) + 4, 4, 1, ip, bp);
  return 0;
}
static void remove_commit(GamePkgX86Shutdown *s, Node *n, uint32_t ip,
                          uint32_t bp) {
  store(link(s, n->prev), link(s, n->next), ip, bp);
  store(link(s, n->next) + 4, link(s, n->prev), ip, bp);
  if (n->prev)
    n->prev->next = n->next;
  else
    s->first = n->next;
  if (n->next)
    n->next->prev = n->prev;
  else
    s->last = n->prev;
  n->next = n->prev = NULL;
}
GamePkgX86Shutdown *gamepkg_x86_shutdown_create(GamePkgX86Cpu *cpu,
                                                GamePkgX86Memory *memory,
                                                uint32_t head) {
  if (!cpu || !gamepkg_x86_memory_is_bound(memory, 0) || head > UINT32_MAX - 7)
    return NULL;
  GamePkgX86Shutdown *s = calloc(1, sizeof(*s));
  if (s) {
    s->cpu = cpu;
    s->memory = memory;
    s->head = head;
  }
  return s;
}
int gamepkg_x86_shutdown_init(GamePkgX86Shutdown *s, GamePkgX86CpuThread *t,
                              uint32_t ip, uint32_t bp) {
  if (owner(s, t) || s->initialized)
    return -1;
  uint32_t w[4] = {s->head, s->head, 0, 0};
  gamepkg_x86_memory_write(s->head, w, 8, 255, 0, ip, bp);
  s->initialized = 1;
  return 0;
}
int gamepkg_x86_shutdown_register(GamePkgX86Shutdown *s, GamePkgX86CpuThread *t,
                                  uint32_t a, uint32_t add, uint32_t ip,
                                  uint32_t bp) {
  if (owner(s, t) || !s->initialized || add > 1 || (add && s->phase == 2))
    return -1;
  if (a > UINT32_MAX - 15 || overlaps(a, 16, s->head, 8))
    return -2;
  Node *found = NULL;
  for (Node *p = s->first; p; p = p->next) {
    if (p->address == a)
      found = p;
    else if (overlaps(a, 16, p->address, 16))
      return -2;
  }
  if (!add) {
    if (!found)
      return 0;
    if (remove_prepare(s, found, ip, bp))
      return -2;
    remove_commit(s, found, ip, bp);
    free(found);
    return 0;
  }
  if (found)
    return -2;
  uint32_t w[4];
  gamepkg_x86_memory_read(a, w, 16, ip, bp);
  Node *prev = NULL, *next = s->first;
  while (next && (next->priority ^ 0x80000000u) >= (w[1] ^ 0x80000000u)) {
    prev = next;
    next = next->next;
  }
  if (check(s, NULL, ip, bp) || (prev && check(s, prev, ip, bp)) ||
      (next && check(s, next, ip, bp)))
    return -2;
  gamepkg_x86_memory_check(a + 8, 8, 1, ip, bp);
  gamepkg_x86_memory_check(link(s, prev), 4, 1, ip, bp);
  gamepkg_x86_memory_check(link(s, next) + 4, 4, 1, ip, bp);
  Node *n = malloc(sizeof(*n));
  if (!n)
    return -1;
  *n = (Node){prev, next, a, w[1]};
  uint32_t links[4] = {link(s, next), link(s, prev), 0, 0};
  gamepkg_x86_memory_write(a + 8, links, 8, 255, 0, ip, bp);
  store(link(s, prev), a + 8, ip, bp);
  store(link(s, next) + 4, a + 8, ip, bp);
  if (prev)
    prev->next = n;
  else
    s->first = n;
  if (next)
    next->prev = n;
  else
    s->last = n;
  return 0;
}
int gamepkg_x86_shutdown_begin(GamePkgX86Shutdown *s, GamePkgX86CpuThread *t) {
  if (owner(s, t) || !s->initialized)
    return -1;
  if (!s->phase)
    s->phase = 1;
  return 0;
}
static GamePkgX86ShutdownCall ticket(GamePkgX86Shutdown *s) {
  return (GamePkgX86ShutdownCall){1, s->active->address, s->active_pc,
                                  (uint32_t)s->sequence,
                                  (uint32_t)(s->sequence >> 32)};
}
static int matches(GamePkgX86Shutdown *s, GamePkgX86ShutdownCall c) {
  return s->active && c.status == 1 && c.registration == s->active->address &&
         c.pc == s->active_pc && c.sequence_low == (uint32_t)s->sequence &&
         c.sequence_high == (uint32_t)(s->sequence >> 32);
}
GamePkgX86ShutdownCall gamepkg_x86_shutdown_next(GamePkgX86Shutdown *s,
                                                 GamePkgX86CpuThread *t,
                                                 uint32_t ip, uint32_t bp) {
  if (owner(s, t) || !s->initialized || !s->phase || s->active)
    return (GamePkgX86ShutdownCall){-1, 0, 0, 0, 0};
  if (check(s, NULL, ip, bp))
    return (GamePkgX86ShutdownCall){-2, 0, 0, 0, 0};
  if (!s->first) {
    s->phase = 2;
    return (GamePkgX86ShutdownCall){0, 0, 0, 0, 0};
  }
  Node *n = s->first;
  uint32_t w[4];
  gamepkg_x86_memory_read(n->address, w, 16, ip, bp);
  if (remove_prepare(s, n, ip, bp))
    return (GamePkgX86ShutdownCall){-2, 0, 0, 0, 0};
  remove_commit(s, n, ip, bp);
  s->active = n;
  s->active_pc = w[0];
  s->prepared = 0;
  ++s->sequence;
  return ticket(s);
}
GamePkgX86ShutdownFrame
gamepkg_x86_shutdown_prepare(GamePkgX86Shutdown *s, GamePkgX86CpuThread *t,
                             GamePkgX86ShutdownCall c, uint32_t esp,
                             uint32_t trap, uint32_t ip, uint32_t bp) {
  GamePkgX86CpuExecution e;
  if (owner(s, t) || !matches(s, c) || s->prepared ||
      gamepkg_x86_cpu_snapshot_execution(t, &e) || !trap ||
      trap > e.code.cs_limit)
    return (GamePkgX86ShutdownFrame){-1, 0, 0};
  uint32_t at = esp - 8u;
  if (at > UINT32_MAX - 7)
    gamepkg_x86_stack_segment(ip, bp);
  gamepkg_x86_memory_check(at, 8, 1, ip, bp);
  if (c.pc > e.code.cs_limit)
    gamepkg_x86_general_protection(ip, bp);
  uint32_t w[4] = {trap, c.registration, 0, 0};
  gamepkg_x86_memory_write(at, w, 8, 255, 0, ip, bp);
  s->return_pc = trap;
  s->return_esp = esp;
  s->prepared = 1;
  return (GamePkgX86ShutdownFrame){0, c.pc, at};
}
int gamepkg_x86_shutdown_complete(GamePkgX86Shutdown *s, GamePkgX86CpuThread *t,
                                  GamePkgX86ShutdownCall c, uint32_t pc,
                                  uint32_t esp) {
  if (owner(s, t) || !matches(s, c) || !s->prepared || pc != s->return_pc ||
      esp != s->return_esp)
    return -1;
  free(s->active);
  s->active = NULL;
  s->prepared = 0;
  return 0;
}
int gamepkg_x86_shutdown_release(GamePkgX86Shutdown *s, GamePkgX86CpuThread *t,
                                 uint32_t abandon) {
  if (domain(s, t) || abandon > 1 ||
      (!abandon &&
       (s->active || s->first || (s->initialized && s->phase != 2))))
    return -1;
  while (s->first) {
    Node *n = s->first;
    s->first = n->next;
    free(n);
  }
  free(s->active);
  free(s);
  return 0;
}
