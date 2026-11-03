#include "gamepkg_x86_dpc.h"
#include "gamepkg_x86_memory.h"
int gamepkg_x86_dpc_initialize(GamePkgX86CpuThread *t, GamePkgX86Memory *m,
                               uint32_t a, uint32_t routine, uint32_t context,
                               uint32_t ip, uint32_t bp) {
  GamePkgX86CpuExecution e;
  if (a > UINT32_MAX - 27 || !gamepkg_x86_memory_is_bound(m, 0) ||
      gamepkg_x86_cpu_snapshot_execution(t, &e) || e.interrupt.cpl ||
      e.code.cs_base || !(e.code.cs_attributes & 0x400u))
    return -1;
  gamepkg_x86_memory_check(a, 3, 1, ip, bp);
  gamepkg_x86_memory_check(a + 12, 8, 1, ip, bp);
  uint32_t w[4] = {19, 0, 0, 0};
  gamepkg_x86_memory_write(a, w, 2, 3, 0, ip, bp);
  w[0] = 0;
  gamepkg_x86_memory_write(a + 2, w, 1, 1, 0, ip, bp);
  w[0] = routine;
  w[1] = context;
  gamepkg_x86_memory_write(a + 12, w, 8, 255, 0, ip, bp);
  return 0;
}
