#include "gamepkg_x86_rtl_string.h"
#include "gamepkg_x86_memory.h"
static int rtl_owner(GamePkgX86Cpu *c, GamePkgX86CpuThread *t,
                     GamePkgX86Memory *m, uint32_t ip, uint32_t bp) {
  if (!c || gamepkg_x86_cpu_owner_in_domain(t, c) ||
      !gamepkg_x86_memory_is_bound(m, 0))
    return -1;
  GamePkgX86CpuExecution e;
  GamePkgX86CpuIrql p;
  if (gamepkg_x86_cpu_snapshot_execution(t, &e) || e.interrupt.cpl ||
      e.code.cs_base || !(e.code.cs_attributes & 0x400u) ||
      gamepkg_x86_cpu_snapshot_irql(t, &p) || !p.required || !p.configured ||
      p.memory != m)
    return -1;
  uint32_t word[4];
  gamepkg_x86_memory_read(p.address, word, 1, ip, bp);
  if (word[0] != p.level)
    return -1;
  return 0;
}
int gamepkg_x86_rtl_init_ansi(GamePkgX86Cpu *c, GamePkgX86CpuThread *t,
                              GamePkgX86Memory *m, uint32_t d, uint32_t s,
                              uint32_t ip, uint32_t bp) {
  if (rtl_owner(c, t, m, ip, bp) || d > UINT32_MAX - 7)
    return -1;
  uint32_t word[4];
  uint32_t length = 0;
  if (s) {
    uint32_t at = s;
    for (;;) {
      gamepkg_x86_memory_read(at, word, 1, ip, bp);
      if (!word[0])
        break;
      ++at;
      ++length;
    }
  }
  gamepkg_x86_memory_check(d, 8, 1, ip, bp);
  uint32_t n = length & 0xffffu, maximum = s ? (n + 1) & 0xffffu : 0;
  word[0] = n | (maximum << 16);
  word[1] = s;
  word[2] = word[3] = 0;
  gamepkg_x86_memory_write(d, word, 8, 255, 0, ip, bp);
  return 0;
}

GamePkgX86RtlEqualResult
gamepkg_x86_rtl_equal_ansi(GamePkgX86Cpu *c, GamePkgX86CpuThread *t,
                           GamePkgX86Memory *m, uint32_t a, uint32_t b,
                           uint32_t insensitive, uint32_t ip, uint32_t bp) {
  if (rtl_owner(c, t, m, ip, bp) || a > UINT32_MAX - 1 || b > UINT32_MAX - 1)
    return (GamePkgX86RtlEqualResult){-1, 0};
  uint32_t x[4], y[4];
  gamepkg_x86_memory_read(a, x, 2, ip, bp);
  gamepkg_x86_memory_read(b, y, 2, ip, bp);
  if (x[0] != y[0])
    return (GamePkgX86RtlEqualResult){0, 0};
  uint32_t n = x[0];
  if (!n)
    return (GamePkgX86RtlEqualResult){0, 1};
  if (a > UINT32_MAX - 7 || b > UINT32_MAX - 7)
    return (GamePkgX86RtlEqualResult){-1, 0};
  gamepkg_x86_memory_read(a + 4, x, 4, ip, bp);
  gamepkg_x86_memory_read(b + 4, y, 4, ip, bp);
  uint32_t pa = x[0], pb = y[0];
  for (uint32_t i = 0; i < n; ++i) {
    gamepkg_x86_memory_read(pa + i, x, 1, ip, bp);
    gamepkg_x86_memory_read(pb + i, y, 1, ip, bp);
    if (insensitive & 255u) {
      if (x[0] >= 'a' && x[0] <= 'z')
        x[0] -= 32;
      if (y[0] >= 'a' && y[0] <= 'z')
        y[0] -= 32;
    }
    if (x[0] != y[0])
      return (GamePkgX86RtlEqualResult){0, 0};
  }
  return (GamePkgX86RtlEqualResult){0, 1};
}
