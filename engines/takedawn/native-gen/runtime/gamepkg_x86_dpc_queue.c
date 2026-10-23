#include "gamepkg_x86_dpc_queue.h"
#include "gamepkg_x86_fault.h"
#include <stdlib.h>
typedef struct Node {
  struct Node *prev, *next;
  uint32_t address, arg1, arg2;
} Node;
struct GamePkgX86DpcQueue {
  GamePkgX86Cpu *cpu;
  GamePkgX86Memory *memory;
  GamePkgX86CpuThread *active_thread;
  uint32_t head, initialized, prepared, return_pc, return_esp;
  uint64_t sequence;
  Node *first, *last;
  GamePkgX86DpcCall active;
};
static int overlap(uint32_t a, uint32_t an, uint32_t b, uint32_t bn) {
  return (uint64_t)a + an > b && (uint64_t)b + bn > a;
}
static int domain(GamePkgX86DpcQueue *q, GamePkgX86CpuThread *t) {
  return !q || gamepkg_x86_cpu_owner_in_domain(t, q->cpu) ||
                 !gamepkg_x86_memory_is_bound(q->memory, 0)
             ? -1
             : 0;
}
static int owner(GamePkgX86DpcQueue *q, GamePkgX86CpuThread *t,
                 GamePkgX86CpuIrql *p) {
  GamePkgX86CpuExecution e;
  return domain(q, t) || gamepkg_x86_cpu_snapshot_execution(t, &e) ||
                 e.interrupt.cpl || e.code.cs_base ||
                 !(e.code.cs_attributes & 0x400) ||
                 gamepkg_x86_cpu_snapshot_irql(t, p) || !p->required ||
                 !p->configured || p->memory != q->memory
             ? -1
             : 0;
}
static uint32_t link(GamePkgX86DpcQueue *q, Node *n) {
  return n ? n->address + 4 : q->head;
}
static void word(uint32_t a, uint32_t v, uint32_t ip, uint32_t bp) {
  uint32_t w[4] = {v, 0, 0, 0};
  gamepkg_x86_memory_write(a, w, 4, 15, 0, ip, bp);
}
static void byte(uint32_t a, uint32_t v, uint32_t ip, uint32_t bp) {
  uint32_t w[4] = {v, 0, 0, 0};
  gamepkg_x86_memory_write(a, w, 1, 1, 0, ip, bp);
}
static int header(uint32_t a, uint32_t inserted, uint32_t ip, uint32_t bp) {
  unsigned char b[3];
  gamepkg_x86_memory_bytes(a, b, 3, 0, ip, bp);
  return b[0] != 19 || b[1] || b[2] != inserted ? -2 : 0;
}
static int check(GamePkgX86DpcQueue *q, Node *n, uint32_t ip, uint32_t bp) {
  uint32_t w[4];
  gamepkg_x86_memory_read(n ? n->address + 4 : q->head, w, 8, ip, bp);
  if (w[0] != link(q, n ? n->next : q->first) ||
      w[1] != link(q, n ? n->prev : q->last))
    return -2;
  if (!n) {
    gamepkg_x86_memory_read(q->head + 8, w, 4, ip, bp);
    return w[0] != (uint32_t)(q->active.status == 1) ? -2 : 0;
  }
  if (header(n->address, 1, ip, bp))
    return -2;
  gamepkg_x86_memory_read(n->address + 20, w, 8, ip, bp);
  return w[0] != n->arg1 || w[1] != n->arg2 ? -2 : 0;
}
static Node *find(GamePkgX86DpcQueue *q, uint32_t a) {
  for (Node *n = q->first; n; n = n->next)
    if (n->address == a)
      return n;
  return NULL;
}
static int admitted(GamePkgX86DpcQueue *q, uint32_t a, GamePkgX86CpuIrql p) {
  if (a > UINT32_MAX - 27 || overlap(a, 28, q->head, 12) ||
      overlap(a, 28, p.address, 1))
    return -2;
  for (Node *n = q->first; n; n = n->next)
    if (n->address != a && overlap(a, 28, n->address, 28))
      return -2;
  if (q->active.status == 1 && q->active.dpc != a &&
      overlap(a, 28, q->active.dpc, 28))
    return -2;
  return 0;
}
static int remove_check(GamePkgX86DpcQueue *q, Node *n, uint32_t ip,
                        uint32_t bp) {
  if (check(q, NULL, ip, bp) || check(q, n, ip, bp) ||
      (n->prev && check(q, n->prev, ip, bp)) ||
      (n->next && check(q, n->next, ip, bp)))
    return -2;
  gamepkg_x86_memory_check(n->address + 2, 1, 1, ip, bp);
  gamepkg_x86_memory_check(link(q, n->prev), 4, 1, ip, bp);
  gamepkg_x86_memory_check(link(q, n->next) + 4, 4, 1, ip, bp);
  return 0;
}
static void remove_commit(GamePkgX86DpcQueue *q, Node *n, uint32_t ip,
                          uint32_t bp) {
  word(link(q, n->prev), link(q, n->next), ip, bp);
  word(link(q, n->next) + 4, link(q, n->prev), ip, bp);
  byte(n->address + 2, 0, ip, bp);
  if (n->prev)
    n->prev->next = n->next;
  else
    q->first = n->next;
  if (n->next)
    n->next->prev = n->prev;
  else
    q->last = n->prev;
}
GamePkgX86DpcQueue *gamepkg_x86_dpc_queue_create(GamePkgX86Cpu *cpu,
                                                 GamePkgX86Memory *memory,
                                                 uint32_t head) {
  if (!cpu || !gamepkg_x86_memory_is_bound(memory, 0) || head > UINT32_MAX - 11)
    return NULL;
  GamePkgX86DpcQueue *q = calloc(1, sizeof(*q));
  if (q) {
    q->cpu = cpu;
    q->memory = memory;
    q->head = head;
  }
  return q;
}
int gamepkg_x86_dpc_queue_init(GamePkgX86DpcQueue *q, GamePkgX86CpuThread *t,
                               uint32_t ip, uint32_t bp) {
  GamePkgX86CpuIrql p;
  if (owner(q, t, &p) || q->initialized || overlap(q->head, 12, p.address, 1))
    return -1;
  unsigned char b[12];
  for (unsigned i = 0; i < 12; ++i)
    b[i] = i < 8 ? (unsigned char)(q->head >> (8 * (i % 4))) : 0;
  gamepkg_x86_memory_bytes(q->head, b, 12, 1, ip, bp);
  q->initialized = 1;
  return 0;
}
int gamepkg_x86_dpc_queue_in_domain(GamePkgX86DpcQueue *q, GamePkgX86Cpu *c,
                                    GamePkgX86Memory *m) {
  return q && q->initialized && q->cpu == c && q->memory == m ? 0 : -1;
}
int gamepkg_x86_dpc_queue_disjoint(GamePkgX86DpcQueue *q, uint32_t a,
                                   uint32_t bytes) {
  if (!q || !bytes || (uint64_t)a + bytes > (UINT64_C(1) << 32) ||
      overlap(a, bytes, q->head, 12))
    return -1;
  for (Node *n = q->first; n; n = n->next)
    if (overlap(a, bytes, n->address, 28))
      return -1;
  return q->active.status == 1 && overlap(a, bytes, q->active.dpc, 28) ? -1 : 0;
}
int gamepkg_x86_dpc_queue_check_insert(GamePkgX86DpcQueue *q,
                                       GamePkgX86CpuThread *t, uint32_t a,
                                       uint32_t ip, uint32_t bp) {
  GamePkgX86CpuIrql p;
  if (owner(q, t, &p) || !q->initialized)
    return -1;
  if (admitted(q, a, p) || check(q, NULL, ip, bp))
    return -2;
  Node *n = find(q, a);
  if (n)
    return check(q, n, ip, bp) ? -2 : 0;
  if (header(a, 0, ip, bp) || (q->last && check(q, q->last, ip, bp)))
    return -2;
  gamepkg_x86_memory_check(a + 2, 1, 1, ip, bp);
  gamepkg_x86_memory_check(a + 4, 8, 1, ip, bp);
  gamepkg_x86_memory_check(a + 20, 8, 1, ip, bp);
  gamepkg_x86_memory_check(link(q, q->last), 4, 1, ip, bp);
  gamepkg_x86_memory_check(q->head + 4, 4, 1, ip, bp);
  return 1;
}
int gamepkg_x86_dpc_queue_insert(GamePkgX86DpcQueue *q, GamePkgX86CpuThread *t,
                                 uint32_t a, uint32_t arg1, uint32_t arg2,
                                 uint32_t ip, uint32_t bp) {
  int ready = gamepkg_x86_dpc_queue_check_insert(q, t, a, ip, bp);
  if (ready != 1)
    return ready;
  Node *n = malloc(sizeof(*n));
  if (!n)
    return -1;
  *n = (Node){q->last, NULL, a, arg1, arg2};
  uint32_t w[4] = {q->head, link(q, q->last), arg1, arg2};
  gamepkg_x86_memory_write(a + 4, w, 8, 255, 0, ip, bp);
  uint32_t arguments[4] = {arg1, arg2, 0, 0};
  gamepkg_x86_memory_write(a + 20, arguments, 8, 255, 0, ip, bp);
  byte(a + 2, 1, ip, bp);
  word(link(q, q->last), a + 4, ip, bp);
  word(q->head + 4, a + 4, ip, bp);
  if (q->last)
    q->last->next = n;
  else
    q->first = n;
  q->last = n;
  return 1;
}
int gamepkg_x86_dpc_queue_remove(GamePkgX86DpcQueue *q, GamePkgX86CpuThread *t,
                                 uint32_t a, uint32_t ip, uint32_t bp) {
  GamePkgX86CpuIrql p;
  if (owner(q, t, &p) || !q->initialized)
    return -1;
  if (admitted(q, a, p) || check(q, NULL, ip, bp))
    return -2;
  Node *n = find(q, a);
  if (!n)
    return header(a, 0, ip, bp) ? -2 : 0;
  if (remove_check(q, n, ip, bp))
    return -2;
  remove_commit(q, n, ip, bp);
  free(n);
  return 1;
}
static int matches(GamePkgX86DpcQueue *q, GamePkgX86CpuThread *t,
                   GamePkgX86DpcCall c) {
  GamePkgX86DpcCall a = q->active;
  return a.status == 1 && t == q->active_thread && c.status == 1 &&
         c.dpc == a.dpc && c.pc == a.pc && c.context == a.context &&
         c.arg1 == a.arg1 && c.arg2 == a.arg2 &&
         c.previous_irql == a.previous_irql &&
         c.sequence_low == a.sequence_low && c.sequence_high == a.sequence_high;
}
GamePkgX86DpcCall gamepkg_x86_dpc_queue_claim(GamePkgX86DpcQueue *q,
                                              GamePkgX86CpuThread *t,
                                              uint32_t ip, uint32_t bp) {
  GamePkgX86CpuIrql p;
  GamePkgX86InterruptState state;
  if (owner(q, t, &p) || !q->initialized || q->active.status == 1 ||
      gamepkg_x86_cpu_snapshot(t, &state))
    return (GamePkgX86DpcCall){.status = -1};
  int eligible = gamepkg_x86_interrupt_eligible(&state);
  if (eligible < 0)
    return (GamePkgX86DpcCall){.status = -1};
  if (!eligible || p.level >= 2)
    return (GamePkgX86DpcCall){0};
  if (check(q, NULL, ip, bp))
    return (GamePkgX86DpcCall){.status = -2};
  Node *n = q->first;
  if (!n)
    return (GamePkgX86DpcCall){0};
  uint32_t w[4], priority[4];
  gamepkg_x86_memory_read(n->address + 12, w, 8, ip, bp);
  gamepkg_x86_memory_read(p.address, priority, 1, ip, bp);
  if (priority[0] != p.level || remove_check(q, n, ip, bp))
    return (GamePkgX86DpcCall){.status = -2};
  gamepkg_x86_memory_check(q->head + 8, 4, 1, ip, bp);
  gamepkg_x86_memory_check(p.address, 1, 1, ip, bp);
  uint64_t sequence = q->sequence + 1;
  GamePkgX86DpcCall c = {1,
                         n->address,
                         w[0],
                         w[1],
                         n->arg1,
                         n->arg2,
                         p.level,
                         (uint32_t)sequence,
                         (uint32_t)(sequence >> 32)};
  remove_commit(q, n, ip, bp);
  word(q->head + 8, 1, ip, bp);
  byte(p.address, 2, ip, bp);
  if (gamepkg_x86_cpu_commit_irql(t, p.level, 2))
    abort();
  free(n);
  q->active = c;
  q->active_thread = t;
  q->prepared = 0;
  q->sequence = sequence;
  return c;
}
GamePkgX86DpcFrame gamepkg_x86_dpc_queue_prepare(GamePkgX86DpcQueue *q,
                                                 GamePkgX86CpuThread *t,
                                                 GamePkgX86DpcCall c,
                                                 uint32_t esp, uint32_t trap,
                                                 uint32_t ip, uint32_t bp) {
  GamePkgX86CpuIrql p;
  GamePkgX86CpuExecution e;
  if (owner(q, t, &p) || !matches(q, t, c) || q->prepared || p.level != 2 ||
      gamepkg_x86_cpu_snapshot_execution(t, &e) || !trap ||
      trap > e.code.cs_limit)
    return (GamePkgX86DpcFrame){-1, 0, 0};
  uint32_t mirror[4];
  gamepkg_x86_memory_read(p.address, mirror, 1, ip, bp);
  if (mirror[0] != 2 || check(q, NULL, ip, bp))
    return (GamePkgX86DpcFrame){-2, 0, 0};
  uint32_t at = esp - 20;
  if (at > UINT32_MAX - 19)
    gamepkg_x86_stack_segment(ip, bp);
  gamepkg_x86_memory_check(at, 20, 1, ip, bp);
  if (c.pc > e.code.cs_limit)
    gamepkg_x86_general_protection(ip, bp);
  uint32_t words[5] = {trap, c.dpc, c.context, c.arg1, c.arg2};
  unsigned char b[20];
  for (unsigned i = 0; i < 20; ++i)
    b[i] = (unsigned char)(words[i / 4] >> (8 * (i % 4)));
  gamepkg_x86_memory_bytes(at, b, 20, 1, ip, bp);
  q->return_pc = trap;
  q->return_esp = esp;
  q->prepared = 1;
  return (GamePkgX86DpcFrame){0, c.pc, at};
}
int gamepkg_x86_dpc_queue_complete(GamePkgX86DpcQueue *q,
                                   GamePkgX86CpuThread *t, GamePkgX86DpcCall c,
                                   uint32_t pc, uint32_t esp, uint32_t ip,
                                   uint32_t bp) {
  GamePkgX86CpuIrql p;
  if (owner(q, t, &p) || !matches(q, t, c) || !q->prepared ||
      pc != q->return_pc || esp != q->return_esp || p.level != 2)
    return -1;
  uint32_t w[4];
  gamepkg_x86_memory_read(p.address, w, 1, ip, bp);
  if (w[0] != 2 || check(q, NULL, ip, bp))
    return -2;
  gamepkg_x86_memory_check(q->head + 8, 4, 1, ip, bp);
  gamepkg_x86_memory_check(p.address, 1, 1, ip, bp);
  word(q->head + 8, 0, ip, bp);
  byte(p.address, c.previous_irql, ip, bp);
  if (gamepkg_x86_cpu_commit_irql(t, 2, c.previous_irql))
    abort();
  q->active = (GamePkgX86DpcCall){0};
  q->active_thread = NULL;
  q->prepared = 0;
  return 0;
}
int gamepkg_x86_dpc_queue_release(GamePkgX86DpcQueue *q, GamePkgX86CpuThread *t,
                                  uint32_t abandon) {
  if (domain(q, t) || abandon > 1 ||
      (!abandon && (q->first || q->active.status == 1)))
    return -1;
  while (q->first) {
    Node *n = q->first;
    q->first = n->next;
    free(n);
  }
  free(q);
  return 0;
}
