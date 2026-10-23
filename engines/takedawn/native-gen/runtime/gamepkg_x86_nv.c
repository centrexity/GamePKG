#include "gamepkg_x86_nv.h"
#include "gamepkg_x86_memory.h"
#include <stdlib.h>
#include <string.h>
typedef struct Entry {
  uint32_t index, type, length;
  unsigned char bytes[256];
} Entry;
struct GamePkgX86Nv {
  GamePkgX86Cpu *cpu;
  GamePkgX86Memory *memory;
  size_t count;
  Entry entries[];
};
static int known(uint32_t i) {
  return i <= 0x12 || i == 0xff || (i >= 0x100 && i <= 0x104) || i == 0x1ff ||
         i == 0xfffe || i == 0xffff;
}
GamePkgX86Nv *gamepkg_x86_nv_create(GamePkgX86Cpu *c, GamePkgX86Memory *m,
                                    const GamePkgX86NvValue *v, size_t count) {
  if (!c || !gamepkg_x86_memory_is_bound(m, 0) ||
      count > (SIZE_MAX - sizeof(GamePkgX86Nv)) / sizeof(Entry) ||
      (count && (!v || (uintptr_t)v % _Alignof(GamePkgX86NvValue) ||
                 count > SIZE_MAX / sizeof(*v) ||
                 (uintptr_t)v > UINTPTR_MAX - (count * sizeof(*v) - 1))))
    return NULL;
  for (size_t i = 0; i < count; ++i) {
    if (!known(v[i].index) || !v[i].data || !v[i].length || v[i].length > 256 ||
        (v[i].type != 1 && v[i].type != 3 && v[i].type != 4) ||
        (v[i].type == 4 && v[i].length != 4) ||
        (uintptr_t)v[i].data > UINTPTR_MAX - (v[i].length - 1))
      return NULL;
    for (size_t j = 0; j < i; ++j)
      if (v[i].index == v[j].index)
        return NULL;
  }
  GamePkgX86Nv *s = malloc(sizeof(*s) + count * sizeof(Entry));
  if (!s)
    return NULL;
  s->cpu = c;
  s->memory = m;
  s->count = count;
  for (size_t i = 0; i < count; ++i) {
    s->entries[i].index = v[i].index;
    s->entries[i].type = v[i].type;
    s->entries[i].length = v[i].length;
    memcpy(s->entries[i].bytes, v[i].data, v[i].length);
  }
  return s;
}
void gamepkg_x86_nv_release(GamePkgX86Nv *s) { free(s); }
static int owner(GamePkgX86Nv *s, GamePkgX86CpuThread *t) {
  GamePkgX86CpuExecution e;
  return !s || gamepkg_x86_cpu_owner_in_domain(t, s->cpu) ||
                 !gamepkg_x86_memory_is_bound(s->memory, 0) ||
                 gamepkg_x86_cpu_snapshot_execution(t, &e) || e.interrupt.cpl ||
                 e.code.cs_base || !(e.code.cs_attributes & 0x400u)
             ? -1
             : 0;
}
static int span(uint32_t a, uint32_t n) {
  return !n || a <= UINT32_MAX - (n - 1);
}
static uint32_t chunk(uint32_t n) { return n > 512 ? 512 : n; }
static void check(uint32_t a, uint32_t n, uint32_t ip, uint32_t bp) {
  for (uint32_t offset = 0; offset < n;) {
    uint32_t bytes = chunk(n - offset);
    gamepkg_x86_memory_check(a + offset, bytes, 1, ip, bp);
    offset += bytes; /* n<=UINT32_MAX; the admitted span does not wrap. */
  }
}
static void word(uint32_t a, uint32_t value, uint32_t ip, uint32_t bp) {
  uint32_t w[4] = {value, 0, 0, 0};
  gamepkg_x86_memory_write(a, w, 4, 15, 0, ip, bp);
}
GamePkgX86NvResult gamepkg_x86_nv_query(GamePkgX86Nv *s, GamePkgX86CpuThread *t,
                                        uint32_t index, uint32_t type,
                                        uint32_t value, uint32_t length,
                                        uint32_t result, uint32_t ip,
                                        uint32_t bp) {
  if (owner(s, t))
    return (GamePkgX86NvResult){-1, 0, 0, 0};
  const Entry *entry = NULL;
  for (size_t i = 0; i < s->count; ++i)
    if (s->entries[i].index == index) {
      entry = &s->entries[i];
      break;
    }
  if (!entry)
    return known(index) ? (GamePkgX86NvResult){-2, 0, 0, 0}
                        : (GamePkgX86NvResult){0, 0xc0000034u, 0, 0};
  uint32_t short_buffer = length < entry->length;
  if ((result && !span(result, 4)) ||
      (!short_buffer && (!span(type, 4) || !span(value, length))))
    return (GamePkgX86NvResult){-1, 0, 0, 0};
  if (result)
    check(result, 4, ip, bp);
  if (!short_buffer) {
    check(type, 4, ip, bp);
    check(value, length, ip, bp);
  }
  if (result)
    word(result, entry->length, ip, bp);
  if (!short_buffer) {
    word(type, entry->type, ip, bp);
    unsigned char zeros[512] = {0};
    for (uint32_t offset = 0; offset < length;) {
      uint32_t bytes = chunk(length - offset);
      gamepkg_x86_memory_bytes(value + offset, zeros, bytes, 1, ip, bp);
      offset += bytes;
    }
    /* The frozen profile cannot alias guest RAM under the host contract. */
    unsigned char payload[256];
    memcpy(payload, entry->bytes, entry->length);
    gamepkg_x86_memory_bytes(value, payload, entry->length, 1, ip, bp);
  }
  return (GamePkgX86NvResult){0, short_buffer ? 0xc0000023u : 0, entry->type,
                              entry->length};
}
