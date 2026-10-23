#include "gamepkg_x86_string.h"
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_memory.h"
#include <stdlib.h>
extern _Thread_local uint32_t g_eax, g_ecx, g_esi, g_edi, g_eflags, g_fs_base;
extern _Thread_local int g_df;
static uint32_t offset(uint32_t value, uint32_t mask, uint32_t bytes,
                       uint32_t ip, uint32_t bp) {
  uint32_t v = value & mask;
  if (v > UINT32_MAX - (bytes - 1))
    gamepkg_x86_general_protection(ip, bp);
  return v;
}
static void advance(uint32_t *reg, uint32_t mask, uint32_t delta) {
  *reg = (*reg & ~mask) | ((*reg + delta) & mask);
}
static uint32_t subtract(uint32_t a, uint32_t b, uint32_t bytes,
                         uint32_t initial) {
  uint32_t mask = UINT32_MAX >> (32 - bytes * 8), sign = 1u << (bytes * 8 - 1);
  a &= mask;
  b &= mask;
  uint32_t r = (a - b) & mask;
  return (initial & ~0x8d5u) | 2u | (a < b ? 1u : 0u) |
         (((0x9669u >> ((r ^ (r >> 4)) & 15)) & 1) << 2) | ((a ^ b ^ r) & 16) |
         (r ? 0u : 0x40u) | (r & sign ? 0x80u : 0u) |
         ((a ^ b) & (a ^ r) & sign ? 0x800u : 0u);
}
static int run(uint32_t op, uint32_t bytes, uint32_t a16, uint32_t repeat,
               uint32_t fs, uint32_t ip, uint32_t bp,
               int (*boundary)(uint32_t, uint32_t)) {
  if (op > 4 || (bytes != 1 && bytes != 2 && bytes != 4) || a16 > 1 || fs > 1 ||
      repeat > 2 || (repeat == 2 && op < 3))
    abort();
  uint32_t amask = a16 ? 0xffffu : UINT32_MAX,
           vmask = UINT32_MAX >> (32 - bytes * 8);
  uint32_t delta = g_df ? 0u - bytes : bytes, original_flags = g_eflags,
           flags = original_flags;
  while (!repeat || (g_ecx & amask)) {
    uint32_t a = g_eax & vmask, b = 0, source = 0, destination = 0,
             words[4] = {0};
    if (op == 0 || op == 2 || op == 3) {
      source = offset(g_esi, amask, bytes, ip, bp) + (fs ? g_fs_base : 0u);
      gamepkg_x86_memory_read(source, words, bytes, ip, bp);
      a = words[0] & vmask;
    }
    if (op != 2) {
      destination = offset(g_edi, amask, bytes, ip, bp);
      if (op >= 3) {
        gamepkg_x86_memory_read(destination, words, bytes, ip, bp);
        b = words[0] & vmask;
      } else {
        words[0] = a;
        gamepkg_x86_memory_write(destination, words, bytes, (1u << bytes) - 1,
                                 0, ip, bp);
      }
    }
    /* All fault-capable work for this element is done. Earlier completed
     * elements remain committed; no count/index/data moves on this fault. */
    if (op == 2)
      g_eax = (g_eax & ~vmask) | a;
    if (op == 0 || op == 2 || op == 3)
      advance(&g_esi, amask, delta);
    if (op != 2)
      advance(&g_edi, amask, delta);
    if (op >= 3)
      flags = subtract(a, b, bytes, flags);
    if (!repeat)
      break;
    advance(&g_ecx, amask, UINT32_MAX);
    if (op >= 3 &&
        ((repeat == 1 && !(flags & 0x40)) || (repeat == 2 && (flags & 0x40))))
      break;
    if (boundary && (g_ecx & amask)) {
      g_eflags = flags;
      if (boundary(ip, bp))
        return 1;
      g_eflags = original_flags;
    }
  }
  if (op >= 3)
    g_eflags = flags;
  return 0;
}
void gamepkg_x86_string(uint32_t op, uint32_t bytes, uint32_t a16,
                        uint32_t repeat, uint32_t fs, uint32_t ip,
                        uint32_t bp) {
  (void)run(op, bytes, a16, repeat, fs, ip, bp, NULL);
}
int gamepkg_x86_string_controlled(uint32_t op, uint32_t bytes, uint32_t a16,
                                  uint32_t repeat, uint32_t fs, uint32_t ip,
                                  uint32_t bp,
                                  int (*boundary)(uint32_t, uint32_t)) {
  if (!boundary)
    abort();
  return run(op, bytes, a16, repeat, fs, ip, bp, boundary);
}
