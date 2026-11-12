#include "gamepkg_x86_handles.h"
#include "gamepkg_x86_memory.h"
#include <stdlib.h>
struct GamePkgX86HandleTicket {
  struct GamePkgX86HandleTicket *next;
  uint32_t id;
  GamePkgX86HandleValue value;
};
struct GamePkgX86Handles {
  GamePkgX86Cpu *cpu;
  GamePkgX86Memory *memory;
  uint32_t next;
  uint32_t callback;
  GamePkgX86HandleTicket *first, *pending;
};
GamePkgX86Handles *gamepkg_x86_handles_create(GamePkgX86Cpu *c,
                                              GamePkgX86Memory *m) {
  if (!c || !gamepkg_x86_memory_is_bound(m, 0))
    return NULL;
  GamePkgX86Handles *h = calloc(1, sizeof(*h));
  if (h) {
    h->cpu = c;
    h->memory = m;
    h->next = 4;
  }
  return h;
}
int gamepkg_x86_handles_in_domain(GamePkgX86Handles *h, GamePkgX86Cpu *c,
                                  GamePkgX86Memory *m) {
  return h && h->cpu == c && h->memory == m ? 0 : -1;
}
static int metadata_owner(GamePkgX86Handles *h, GamePkgX86CpuThread *t) {
  return !h || h->callback || gamepkg_x86_cpu_owner_in_domain(t, h->cpu) ||
                 !gamepkg_x86_memory_is_bound(h->memory, 0)
             ? -1
             : 0;
}
int gamepkg_x86_handles_owner(GamePkgX86Handles *h, GamePkgX86CpuThread *t,
                              uint32_t ip, uint32_t bp) {
  if (metadata_owner(h, t))
    return -1;
  GamePkgX86CpuExecution e;
  GamePkgX86CpuIrql p;
  if (gamepkg_x86_cpu_snapshot_execution(t, &e) || e.interrupt.cpl ||
      e.code.cs_base || !(e.code.cs_attributes & 0x400) ||
      gamepkg_x86_cpu_snapshot_irql(t, &p) || !p.required || !p.configured ||
      p.level || p.memory != h->memory)
    return -1;
  uint32_t w[4];
  gamepkg_x86_memory_read(p.address, w, 1, ip, bp);
  return w[0] ? -1 : 0;
}
GamePkgX86HandleReservation gamepkg_x86_handles_prepare(GamePkgX86Handles *h,
                                                        GamePkgX86CpuThread *t,
                                                        uint32_t ip,
                                                        uint32_t bp) {
  if (gamepkg_x86_handles_owner(h, t, ip, bp) || h->pending)
    return (GamePkgX86HandleReservation){-1, NULL};
  uint32_t id = h->next;
  for (;;) {
    GamePkgX86HandleTicket *live = h->first;
    while (live && live->id != id)
      live = live->next;
    if (!live)
      break;
    id = id == 0xfffffffcu ? 4 : id + 4;
    if (id == h->next)
      return (GamePkgX86HandleReservation){1, NULL};
  }
  GamePkgX86HandleTicket *n = calloc(1, sizeof(*n));
  if (!n)
    return (GamePkgX86HandleReservation){1, NULL};
  n->id = id;
  h->pending = n;
  return (GamePkgX86HandleReservation){0, n};
}
int gamepkg_x86_handles_commit(GamePkgX86Handles *h, GamePkgX86CpuThread *t,
                               GamePkgX86HandleTicket *n,
                               GamePkgX86HandleValue v, uint32_t *out) {
  if (metadata_owner(h, t) || !n || h->pending != n || !v.kind || !v.owner ||
      !v.close || !out)
    return -1;
  n->value = v;
  n->next = h->first;
  h->first = n;
  h->pending = NULL;
  h->next = n->id == 0xfffffffcu ? 4 : n->id + 4;
  *out = n->id;
  return 0;
}
int gamepkg_x86_handles_cancel(GamePkgX86Handles *h, GamePkgX86CpuThread *t,
                               GamePkgX86HandleTicket *n) {
  if (metadata_owner(h, t) || !n || h->pending != n)
    return -1;
  h->pending = NULL;
  free(n);
  return 0;
}
GamePkgX86HandleLookup gamepkg_x86_handles_lookup(GamePkgX86Handles *h,
                                                  GamePkgX86CpuThread *t,
                                                  uint32_t id, uint32_t ip,
                                                  uint32_t bp) {
  if (gamepkg_x86_handles_owner(h, t, ip, bp) || h->pending)
    return (GamePkgX86HandleLookup){.status = -1};
  for (GamePkgX86HandleTicket *n = h->first; n; n = n->next)
    if (n->id == id)
      return (GamePkgX86HandleLookup){0, 0, n->value};
  return (GamePkgX86HandleLookup){.ntstatus = 0xc0000008u};
}
GamePkgX86HandleResult gamepkg_x86_handles_close(GamePkgX86Handles *h,
                                                 GamePkgX86CpuThread *t,
                                                 uint32_t id, uint32_t ip,
                                                 uint32_t bp) {
  if (gamepkg_x86_handles_owner(h, t, ip, bp) || h->pending)
    return (GamePkgX86HandleResult){-1, 0};
  GamePkgX86HandleTicket **slot = &h->first;
  while (*slot && (*slot)->id != id)
    slot = &(*slot)->next;
  if (!*slot)
    return (GamePkgX86HandleResult){0, 0xc0000008u};
  GamePkgX86HandleTicket *n = *slot;
  uint32_t ntstatus = 0;
  h->callback = 1;
  int status = n->value.close(n->value.owner, n->value.data, 0, &ntstatus);
  h->callback = 0;
  if (status || ntstatus)
    return (GamePkgX86HandleResult){status, ntstatus};
  *slot = n->next;
  free(n);
  return (GamePkgX86HandleResult){0, 0};
}
size_t gamepkg_x86_handles_owned(GamePkgX86Handles *h, void *owner) {
  if (!h || h->callback)
    return SIZE_MAX;
  size_t count = 0;
  for (GamePkgX86HandleTicket *n = h->first; n; n = n->next)
    count += n->value.owner == owner;
  return count;
}
int gamepkg_x86_handles_drain(GamePkgX86Handles *h) {
  if (!h || h->callback || h->pending)
    return -1;
  while (h->first) {
    GamePkgX86HandleTicket *n = h->first;
    uint32_t ntstatus = 0;
    h->callback = 1;
    int status = n->value.close(n->value.owner, n->value.data, 1, &ntstatus);
    h->callback = 0;
    if (status || ntstatus)
      return -1;
    h->first = n->next;
    free(n);
  }
  return 0;
}
int gamepkg_x86_handles_release(GamePkgX86Handles *h, uint32_t terminal) {
  if (!h || terminal > 1 || h->callback || h->pending ||
      (!terminal && h->first))
    return -1;
  if (terminal && gamepkg_x86_handles_drain(h))
    return -1;
  free(h);
  return 0;
}
