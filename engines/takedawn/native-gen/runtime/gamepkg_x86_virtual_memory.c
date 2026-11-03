#include "gamepkg_x86_virtual_memory.h"
#include <stdlib.h>
struct GamePkgX86VirtualMemory {
  GamePkgX86Cpu *cpu;
  GamePkgX86Memory *memory;
};
GamePkgX86VirtualMemory *
gamepkg_x86_virtual_memory_create(GamePkgX86Cpu *cpu,
                                  GamePkgX86Memory *memory) {
  if (!cpu || !gamepkg_x86_memory_is_bound(memory, 0) ||
      gamepkg_x86_virtual_is_configured(memory))
    return NULL;
  GamePkgX86VirtualMemory *s = malloc(sizeof(*s));
  if (s)
    *s = (GamePkgX86VirtualMemory){cpu, memory};
  return s;
}
void gamepkg_x86_virtual_memory_release(GamePkgX86VirtualMemory *s) { free(s); }
static int owner(GamePkgX86VirtualMemory *s, GamePkgX86CpuThread *t,
                 uint32_t ip, uint32_t bp) {
  GamePkgX86CpuExecution e;
  GamePkgX86CpuIrql p;
  if (!s || gamepkg_x86_cpu_owner_in_domain(t, s->cpu) ||
      !gamepkg_x86_memory_is_bound(s->memory, 0) ||
      gamepkg_x86_virtual_is_configured(s->memory) ||
      gamepkg_x86_cpu_snapshot_execution(t, &e) || e.interrupt.cpl ||
      e.code.cs_base || !(e.code.cs_attributes & 0x400) ||
      gamepkg_x86_cpu_snapshot_irql(t, &p) || !p.required || !p.configured ||
      p.level || p.memory != s->memory)
    return -1;
  uint32_t w[4];
  gamepkg_x86_memory_read(p.address, w, 1, ip, bp);
  return w[0] ? -1 : 0;
}
static GamePkgX86VirtualMemoryResult result(int host, uint32_t status) {
  return (GamePkgX86VirtualMemoryResult){host, status, 0, 0};
}
static int protection(uint32_t p) {
  switch (p) {
  case 1:
  case 2:
  case 4:
  case 0x10:
  case 0x20:
  case 0x40:
    return 0;
  }
  if (p & ~0xfffu)
    return 1;
  uint32_t basic = p & 255u;
  if (!basic || (basic & (basic - 1)))
    return 1;
  return -2;
}
static int read_parameters(uint32_t a, uint32_t n, uint32_t *base,
                           uint32_t *size, uint32_t ip, uint32_t bp) {
  if (a > UINT32_MAX - 3 || n > UINT32_MAX - 3)
    return -1;
  uint32_t w[4];
  gamepkg_x86_memory_read(a, w, 4, ip, bp);
  *base = w[0];
  gamepkg_x86_memory_read(n, w, 4, ip, bp);
  *size = w[0];
  return 0;
}
static int overlap(uint32_t a, uint32_t n, uint32_t p) {
  return (uint64_t)a + n > p && (uint64_t)p + 4 > a;
}
static void admit_outputs(uint32_t a, uint32_t n, uint32_t ip, uint32_t bp) {
  gamepkg_x86_memory_check(a, 4, 1, ip, bp);
  gamepkg_x86_memory_check(n, 4, 1, ip, bp);
}
static void outputs(uint32_t a, uint32_t n, uint32_t base, uint32_t size,
                    uint32_t ip, uint32_t bp) {
  uint32_t w[4] = {base, 0, 0, 0};
  gamepkg_x86_memory_write(a, w, 4, 15, 0, ip, bp);
  w[0] = size;
  gamepkg_x86_memory_write(n, w, 4, 15, 0, ip, bp);
}
static GamePkgX86VirtualMemoryResult finish(GamePkgX86VirtualResult r,
                                            uint32_t a, uint32_t n, uint32_t ip,
                                            uint32_t bp) {
  if (r.status < 0)
    return result(r.status, 0);
  if (r.status)
    return result(0, r.status == 2   ? 0xc0000017u
                     : r.status == 3 ? 0xc00000a0u
                                     : 0xc0000018u);
  outputs(a, n, r.base, r.length, ip, bp);
  return (GamePkgX86VirtualMemoryResult){0, 0, r.base, r.length};
}
GamePkgX86VirtualMemoryResult gamepkg_x86_virtual_memory_allocate(
    GamePkgX86VirtualMemory *s, GamePkgX86CpuThread *t, uint32_t a,
    uint32_t zero, uint32_t n, uint32_t flags, uint32_t protect, uint32_t ip,
    uint32_t bp) {
  if (owner(s, t, ip, bp))
    return result(-1, 0);
  uint32_t base, size;
  if (read_parameters(a, n, &base, &size, ip, bp))
    return result(-1, 0);
  if (!size || zero > 21 || base > 0x7ffeffffu ||
      (uint64_t)base + size > UINT64_C(0x7fff0000) || flags & ~0x983000u ||
      !(flags & 0x83000u) || ((flags & 0x80000u) && flags != 0x80000u))
    return result(0, 0xc000000du);
  int p = protection(protect);
  if (p > 0)
    return result(0, 0xc0000045u);
  if (p < 0)
    return result(-2, 0);
  if (flags & (0x80000u | 0x800000u))
    return result(-2, 0);
  uint32_t reserve = (flags & 0x2000u) != 0 || !base;
  uint32_t normalized = base & ~(reserve ? 65535u : 4095u);
  if (base && normalized < 0x10000)
    return result(0, 0xc0000018u);
  uint64_t span = base ? (uint64_t)base + size - normalized : size;
  span = (span + 4095) & ~UINT64_C(4095);
  if (span > UINT32_MAX || (uint64_t)normalized + span > UINT64_C(0x7fff0000))
    return result(0, 0xc000000du);
  uint32_t length = (uint32_t)span;
  if (base &&
      (overlap(normalized, length, a) || overlap(normalized, length, n)))
    return result(-2, 0);
  admit_outputs(a, n, ip, bp);
  GamePkgX86VirtualResult r;
  if (reserve) {
    uint32_t maximum = zero && !base ? UINT32_MAX >> zero : 0x7ffeffffu;
    if (maximum > 0x7ffeffffu)
      maximum = 0x7ffeffffu;
    r = gamepkg_x86_virtual_reserve(s->memory, normalized, length, maximum,
                                    (flags & 0x100000u) != 0, protect,
                                    (flags & 0x1000u) != 0);
  } else
    r = gamepkg_x86_virtual_commit(s->memory, normalized, length, protect);
  if (r.status == 3)
    r.status = 1;
  return finish(r, a, n, ip, bp);
}
GamePkgX86VirtualMemoryResult
gamepkg_x86_virtual_memory_free(GamePkgX86VirtualMemory *s,
                                GamePkgX86CpuThread *t, uint32_t a, uint32_t n,
                                uint32_t flags, uint32_t ip, uint32_t bp) {
  if (owner(s, t, ip, bp))
    return result(-1, 0);
  uint32_t base, size;
  if (read_parameters(a, n, &base, &size, ip, bp))
    return result(-1, 0);
  if ((flags != 0x4000 && flags != 0x8000) || base > 0x7ffeffffu ||
      (uint64_t)base + size > UINT64_C(0x7fff0000))
    return result(0, 0xc000000du);
  uint32_t normalized = base & ~4095u;
  uint64_t span =
      size ? ((uint64_t)(base & 4095u) + size + 4095) & ~UINT64_C(4095) : 0;
  if (span > UINT32_MAX)
    return result(0, 0xc000000du);
  uint32_t length = (uint32_t)span;
  if (!length) {
    GamePkgX86VirtualInfo q = gamepkg_x86_virtual_query(s->memory, normalized);
    if (q.status < 0)
      return result(q.status, 0);
    if (q.state == 0x10000)
      return result(0, 0xc00000a0u);
    if (q.allocation_base != normalized)
      return result(0, 0xc000009fu);
    // Query run lengths can stop at protection/state boundaries. Determine the
    // entire reservation without modifying it, even across mixed page states.
    uint64_t end = normalized;
    do {
      q = gamepkg_x86_virtual_query(s->memory, (uint32_t)end);
      if (q.status || q.allocation_base != normalized)
        break;
      end += q.length;
    } while (end < UINT64_C(0x7fff0000));
    length = (uint32_t)(end - normalized);
  }
  if (overlap(normalized, length, a) || overlap(normalized, length, n))
    return result(-2, 0);
  admit_outputs(a, n, ip, bp);
  GamePkgX86VirtualResult r =
      gamepkg_x86_virtual_free(s->memory, normalized, length, flags == 0x8000);
  return finish(r, a, n, ip, bp);
}
