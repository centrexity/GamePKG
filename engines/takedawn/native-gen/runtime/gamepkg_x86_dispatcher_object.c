#include "gamepkg_x86_dispatcher_object.h"
#include "gamepkg_x86_memory.h"
static int owner(GamePkgX86CpuThread *t, GamePkgX86Memory *m) {
  GamePkgX86CpuExecution e;
  return !gamepkg_x86_memory_is_bound(m, 0) ||
                 gamepkg_x86_cpu_snapshot_execution(t, &e) || e.interrupt.cpl ||
                 e.code.cs_base || !(e.code.cs_attributes & 0x400u)
             ? -1
             : 0;
}
static void le32(unsigned char *p, uint32_t value) {
  for (unsigned i = 0; i < 4; ++i)
    p[i] = (unsigned char)(value >> (i * 8));
}
int gamepkg_x86_event_initialize(GamePkgX86CpuThread *t, GamePkgX86Memory *m,
                                 uint32_t a, uint32_t type, uint32_t state,
                                 uint32_t ip, uint32_t bp) {
  if (type > 1 || state > 1 || a > UINT32_MAX - 15 || owner(t, m))
    return -1;
  gamepkg_x86_memory_check(a, 1, 1, ip, bp);
  gamepkg_x86_memory_check(a + 2, 1, 1, ip, bp);
  gamepkg_x86_memory_check(a + 4, 12, 1, ip, bp);
  unsigned char image[16] = {0};
  image[0] = (unsigned char)type;
  image[2] = 4;
  le32(image + 4, state);
  le32(image + 8, a + 8);
  le32(image + 12, a + 8);
  gamepkg_x86_memory_bytes(a, image, 1, 1, ip, bp);
  gamepkg_x86_memory_bytes(a + 2, image + 2, 1, 1, ip, bp);
  gamepkg_x86_memory_bytes(a + 4, image + 4, 12, 1, ip, bp);
  return 0;
}
int gamepkg_x86_timer_initialize(GamePkgX86CpuThread *t, GamePkgX86Memory *m,
                                 uint32_t a, uint32_t type, uint32_t ip,
                                 uint32_t bp) {
  if (type > 1 || a > UINT32_MAX - 39 || owner(t, m))
    return -1;
  gamepkg_x86_memory_check(a, 32, 1, ip, bp);
  gamepkg_x86_memory_check(a + 36, 4, 1, ip, bp);
  unsigned char image[40] = {0};
  image[0] = (unsigned char)(8 + type);
  image[2] = 10;
  le32(image + 8, a + 8);
  le32(image + 12, a + 8);
  gamepkg_x86_memory_bytes(a, image, 32, 1, ip, bp);
  gamepkg_x86_memory_bytes(a + 36, image + 36, 4, 1, ip, bp);
  return 0;
}
