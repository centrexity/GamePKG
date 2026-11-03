#include "gamepkg_x86_timer.h"
#include <stdlib.h>
#include <string.h>
typedef struct Node {
  struct Node *prev, *next;
  uint32_t address, type, absolute, dpc, period;
  uint64_t due, origin, distance, target;
} Node;
struct GamePkgX86Timers {
  GamePkgX86Cpu *cpu;
  GamePkgX86Memory *memory;
  GamePkgX86DpcQueue *dpcs;
  uint32_t head, initialized;
  GamePkgX86Clock now;
  Node *first, *last;
};
static int overlap(uint32_t a, uint32_t an, uint32_t b, uint32_t bn) {
  return (uint64_t)a + an > b && (uint64_t)b + bn > a;
}
static uint32_t get(const unsigned char *p) {
  return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) |
         ((uint32_t)p[3] << 24);
}
static uint64_t wide(const unsigned char *p) {
  return (uint64_t)get(p) | ((uint64_t)get(p + 4) << 32);
}
static void put(unsigned char *p, uint32_t v) {
  for (unsigned i = 0; i < 4; ++i)
    p[i] = (unsigned char)(v >> (8 * i));
}
static void putwide(unsigned char *p, uint64_t v) {
  put(p, (uint32_t)v);
  put(p + 4, (uint32_t)(v >> 32));
}
static int domain(GamePkgX86Timers *s, GamePkgX86CpuThread *t) {
  return !s || gamepkg_x86_cpu_owner_in_domain(t, s->cpu) ||
                 !gamepkg_x86_memory_is_bound(s->memory, 0) ||
                 gamepkg_x86_dpc_queue_in_domain(s->dpcs, s->cpu, s->memory)
             ? -1
             : 0;
}
static int owner(GamePkgX86Timers *s, GamePkgX86CpuThread *t,
                 GamePkgX86CpuIrql *p) {
  GamePkgX86CpuExecution e;
  return domain(s, t) || gamepkg_x86_cpu_snapshot_execution(t, &e) ||
                 e.interrupt.cpl || e.code.cs_base ||
                 !(e.code.cs_attributes & 0x400) ||
                 gamepkg_x86_cpu_snapshot_irql(t, p) || !p->required ||
                 !p->configured || p->memory != s->memory
             ? -1
             : 0;
}
static uint32_t link(GamePkgX86Timers *s, Node *n) {
  return n ? n->address + 24 : s->head;
}
static void word(uint32_t a, uint32_t v, uint32_t ip, uint32_t bp) {
  uint32_t w[4] = {v, 0, 0, 0};
  gamepkg_x86_memory_write(a, w, 4, 15, 0, ip, bp);
}
static int links(GamePkgX86Timers *s, Node *n, uint32_t ip, uint32_t bp) {
  uint32_t w[4];
  gamepkg_x86_memory_read(n ? n->address + 24 : s->head, w, 8, ip, bp);
  return w[0] != link(s, n ? n->next : s->first) ||
                 w[1] != link(s, n ? n->prev : s->last)
             ? -2
             : 0;
}
static Node *find(GamePkgX86Timers *s, uint32_t a) {
  for (Node *n = s->first; n; n = n->next)
    if (n->address == a)
      return n;
  return NULL;
}
static int admit(GamePkgX86Timers *s, uint32_t a, GamePkgX86CpuIrql p) {
  if (a > UINT32_MAX - 39 || overlap(a, 40, s->head, 8) ||
      overlap(a, 40, p.address, 1) ||
      gamepkg_x86_dpc_queue_disjoint(s->dpcs, a, 40))
    return -2;
  for (Node *n = s->first; n; n = n->next)
    if (n->address != a && overlap(a, 40, n->address, 40))
      return -2;
  return 0;
}
static int body(GamePkgX86Timers *s, uint32_t a, Node *n, unsigned char b[40],
                uint32_t ip, uint32_t bp) {
  gamepkg_x86_memory_bytes(a, b, 40, 0, ip, bp);
  if ((b[0] != 8 && b[0] != 9) || b[1] > 1 || b[2] != 10 ||
      b[3] != (uint32_t)(n != NULL) || get(b + 4) > 1)
    return -2;
  if (get(b + 8) != a + 8 || get(b + 12) != a + 8)
    return -3;
  if (n && (b[0] != n->type || b[1] != n->absolute || wide(b + 16) != n->due ||
            get(b + 32) != n->dpc || get(b + 36) != n->period ||
            links(s, n, ip, bp)))
    return -2;
  return 0;
}
static int unlink_check(GamePkgX86Timers *s, Node *n, uint32_t ip,
                        uint32_t bp) {
  if (links(s, NULL, ip, bp) || links(s, n, ip, bp) ||
      (n->prev && links(s, n->prev, ip, bp)) ||
      (n->next && links(s, n->next, ip, bp)))
    return -2;
  gamepkg_x86_memory_check(link(s, n->prev), 4, 1, ip, bp);
  gamepkg_x86_memory_check(link(s, n->next) + 4, 4, 1, ip, bp);
  return 0;
}
static void unlink_commit(GamePkgX86Timers *s, Node *n, uint32_t ip,
                          uint32_t bp) {
  word(link(s, n->prev), link(s, n->next), ip, bp);
  word(link(s, n->next) + 4, link(s, n->prev), ip, bp);
  if (n->prev)
    n->prev->next = n->next;
  else
    s->first = n->next;
  if (n->next)
    n->next->prev = n->prev;
  else
    s->last = n->prev;
}
static void append(GamePkgX86Timers *s, Node *n, uint32_t ip, uint32_t bp) {
  n->prev = s->last;
  n->next = NULL;
  word(n->address + 24, s->head, ip, bp);
  word(n->address + 28, link(s, s->last), ip, bp);
  word(link(s, s->last), n->address + 24, ip, bp);
  word(s->head + 4, n->address + 24, ip, bp);
  if (s->last)
    s->last->next = n;
  else
    s->first = n;
  s->last = n;
}
static int dpc_admit(GamePkgX86Timers *s, GamePkgX86CpuThread *t, uint32_t a,
                     uint32_t dpc, uint32_t ip, uint32_t bp) {
  if (!dpc)
    return 0;
  if (dpc > UINT32_MAX - 27 || overlap(a, 40, dpc, 28) ||
      overlap(s->head, 8, dpc, 28))
    return -2;
  for (Node *n = s->first; n; n = n->next)
    if (overlap(n->address, 40, dpc, 28))
      return -2;
  int checked = gamepkg_x86_dpc_queue_check_insert(s->dpcs, t, dpc, ip, bp);
  return checked < 0 ? checked : 0;
}
GamePkgX86Timers *gamepkg_x86_timers_create(GamePkgX86Cpu *cpu,
                                            GamePkgX86Memory *memory,
                                            GamePkgX86DpcQueue *dpcs,
                                            uint32_t head,
                                            GamePkgX86Clock initial) {
  if (!cpu || !gamepkg_x86_memory_is_bound(memory, 0) ||
      gamepkg_x86_dpc_queue_in_domain(dpcs, cpu, memory) ||
      head > UINT32_MAX - 7 || gamepkg_x86_dpc_queue_disjoint(dpcs, head, 8))
    return NULL;
  GamePkgX86Timers *s = calloc(1, sizeof(*s));
  if (s) {
    s->cpu = cpu;
    s->memory = memory;
    s->dpcs = dpcs;
    s->head = head;
    s->now = initial;
  }
  return s;
}
int gamepkg_x86_timers_init(GamePkgX86Timers *s, GamePkgX86CpuThread *t,
                            uint32_t ip, uint32_t bp) {
  GamePkgX86CpuIrql p;
  if (owner(s, t, &p) || s->initialized || overlap(s->head, 8, p.address, 1))
    return -1;
  uint32_t w[4] = {s->head, s->head, 0, 0};
  gamepkg_x86_memory_write(s->head, w, 8, 255, 0, ip, bp);
  s->initialized = 1;
  return 0;
}
int gamepkg_x86_timers_clock(GamePkgX86Timers *s, GamePkgX86CpuThread *t,
                             GamePkgX86Clock now) {
  GamePkgX86CpuIrql p;
  if (owner(s, t, &p) || !s->initialized ||
      now.interrupt_time < s->now.interrupt_time)
    return -1;
  s->now = now;
  return 0;
}
GamePkgX86TimerResult gamepkg_x86_timer_set(GamePkgX86Timers *s,
                                            GamePkgX86CpuThread *t, uint32_t a,
                                            uint64_t due, uint32_t period,
                                            uint32_t dpc, uint32_t ip,
                                            uint32_t bp) {
  GamePkgX86CpuIrql p;
  if (owner(s, t, &p) || !s->initialized || period > INT32_MAX)
    return (GamePkgX86TimerResult){-1, 0};
  if (admit(s, a, p) || links(s, NULL, ip, bp))
    return (GamePkgX86TimerResult){-2, 0};
  Node *old = find(s, a);
  unsigned char b[40];
  int checked = body(s, a, old, b, ip, bp);
  if (checked)
    return (GamePkgX86TimerResult){checked, 0};
  uint32_t previous = old != NULL, absolute = !(due >> 63),
           ready = absolute && due <= s->now.system_time;
  uint64_t distance = UINT64_C(0) - due,
           deadline = absolute
                          ? s->now.interrupt_time + due - s->now.system_time
                          : s->now.interrupt_time + distance;
  uint32_t armed = !ready || period;
  Node proposed = {.address = a,
                   .type = b[0],
                   .absolute = absolute,
                   .dpc = dpc,
                   .period = period,
                   .due = deadline,
                   .origin = s->now.interrupt_time,
                   .distance = distance,
                   .target = due};
  if (ready && period) {
    proposed.absolute = 0;
    proposed.distance = (uint64_t)period * 10000;
    proposed.due = s->now.interrupt_time + proposed.distance;
  }
  if (old && unlink_check(s, old, ip, bp))
    return (GamePkgX86TimerResult){-2, 0};
  if (s->last && links(s, s->last, ip, bp))
    return (GamePkgX86TimerResult){-2, 0};
  gamepkg_x86_memory_check(a, 40, 1, ip, bp);
  Node *tail = old == s->last ? old ? old->prev : NULL : s->last;
  if (armed) {
    gamepkg_x86_memory_check(link(s, tail), 4, 1, ip, bp);
    gamepkg_x86_memory_check(s->head + 4, 4, 1, ip, bp);
  }
  checked = dpc_admit(s, t, a, dpc, ip, bp);
  if (checked)
    return (GamePkgX86TimerResult){checked, 0};
  Node *allocated = NULL;
  if (armed && !old) {
    allocated = malloc(sizeof(*allocated));
    if (!allocated)
      return (GamePkgX86TimerResult){-1, 0};
  }
  if (ready && dpc) {
    checked = gamepkg_x86_dpc_queue_insert(
        s->dpcs, t, dpc, (uint32_t)s->now.system_time,
        (uint32_t)(s->now.system_time >> 32), ip, bp);
    if (checked < 0) {
      free(allocated);
      return (GamePkgX86TimerResult){checked, 0};
    }
  }
  if (old)
    unlink_commit(s, old, ip, bp);
  b[1] = (unsigned char)proposed.absolute;
  b[3] = (unsigned char)(armed != 0);
  put(b + 4, ready);
  putwide(b + 16, proposed.due);
  put(b + 32, dpc);
  put(b + 36, period);
  gamepkg_x86_memory_bytes(a, b, 40, 1, ip, bp);
  if (armed) {
    Node *n = old ? old : allocated;
    *n = proposed;
    append(s, n, ip, bp);
  } else
    free(old);
  return (GamePkgX86TimerResult){0, previous};
}
GamePkgX86TimerResult gamepkg_x86_timer_cancel(GamePkgX86Timers *s,
                                               GamePkgX86CpuThread *t,
                                               uint32_t a, uint32_t ip,
                                               uint32_t bp) {
  GamePkgX86CpuIrql p;
  if (owner(s, t, &p) || !s->initialized)
    return (GamePkgX86TimerResult){-1, 0};
  if (admit(s, a, p) || links(s, NULL, ip, bp))
    return (GamePkgX86TimerResult){-2, 0};
  Node *n = find(s, a);
  unsigned char b[40];
  int checked = body(s, a, n, b, ip, bp);
  if (checked)
    return (GamePkgX86TimerResult){checked, 0};
  if (!n)
    return (GamePkgX86TimerResult){0, 0};
  if (unlink_check(s, n, ip, bp))
    return (GamePkgX86TimerResult){-2, 0};
  gamepkg_x86_memory_check(a + 3, 1, 1, ip, bp);
  unlink_commit(s, n, ip, bp);
  b[3] = 0;
  gamepkg_x86_memory_bytes(a + 3, b + 3, 1, 1, ip, bp);
  free(n);
  return (GamePkgX86TimerResult){0, 1};
}
GamePkgX86TimerPump gamepkg_x86_timers_pump(GamePkgX86Timers *s,
                                            GamePkgX86CpuThread *t, uint32_t ip,
                                            uint32_t bp) {
  GamePkgX86CpuIrql p;
  if (owner(s, t, &p) || !s->initialized)
    return (GamePkgX86TimerPump){.status = -1};
  if (links(s, NULL, ip, bp))
    return (GamePkgX86TimerPump){.status = -2};
  for (Node *n = s->first; n; n = n->next) {
    unsigned char b[40];
    int checked = body(s, n->address, n, b, ip, bp);
    if (checked)
      return (GamePkgX86TimerPump){.status = checked};
    uint64_t elapsed = s->now.interrupt_time - n->origin;
    uint32_t ready =
        n->absolute ? s->now.system_time >= n->target : elapsed >= n->distance;
    if (!ready) {
      if (n->absolute) {
        uint64_t updated =
            s->now.interrupt_time + n->target - s->now.system_time;
        if (updated != n->due) {
          gamepkg_x86_memory_check(n->address + 16, 8, 1, ip, bp);
          putwide(b, updated);
          gamepkg_x86_memory_bytes(n->address + 16, b, 8, 1, ip, bp);
          n->due = updated;
          return (GamePkgX86TimerPump){2, n->address, 0, 0};
        }
      }
      continue;
    }
    uint64_t next_distance = 0;
    if (n->period) {
      uint64_t period = (uint64_t)n->period * 10000;
      uint64_t excess =
          n->absolute ? s->now.system_time - n->target : elapsed - n->distance;
      next_distance = period - excess % period;
    }
    if (!n->period && unlink_check(s, n, ip, bp))
      return (GamePkgX86TimerPump){.status = -2};
    gamepkg_x86_memory_check(n->address, 4, 1, ip, bp);
    gamepkg_x86_memory_check(n->address + 4, 4, 1, ip, bp);
    if (n->period)
      gamepkg_x86_memory_check(n->address + 16, 8, 1, ip, bp);
    checked = dpc_admit(s, t, n->address, n->dpc, ip, bp);
    if (checked)
      return (GamePkgX86TimerPump){.status = checked};
    uint32_t queued = 0;
    if (n->dpc) {
      checked = gamepkg_x86_dpc_queue_insert(
          s->dpcs, t, n->dpc, (uint32_t)s->now.system_time,
          (uint32_t)(s->now.system_time >> 32), ip, bp);
      if (checked < 0)
        return (GamePkgX86TimerPump){.status = checked};
      queued = (uint32_t)checked;
    }
    GamePkgX86TimerPump result = {1, n->address, n->dpc, queued};
    b[1] = n->period ? 0 : b[1];
    b[3] = (unsigned char)(n->period != 0);
    put(b + 4, 1);
    gamepkg_x86_memory_bytes(n->address, b, 8, 1, ip, bp);
    if (n->period) {
      n->absolute = 0;
      n->origin = s->now.interrupt_time;
      n->distance = next_distance;
      n->due = s->now.interrupt_time + next_distance;
      putwide(b, n->due);
      gamepkg_x86_memory_bytes(n->address + 16, b, 8, 1, ip, bp);
    } else {
      unlink_commit(s, n, ip, bp);
      free(n);
    }
    return result;
  }
  return (GamePkgX86TimerPump){0};
}
int gamepkg_x86_timers_release(GamePkgX86Timers *s, GamePkgX86CpuThread *t,
                               uint32_t abandon) {
  if (domain(s, t) || abandon > 1 || (!abandon && s->first))
    return -1;
  while (s->first) {
    Node *n = s->first;
    s->first = n->next;
    free(n);
  }
  free(s);
  return 0;
}
