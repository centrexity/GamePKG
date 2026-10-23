#include "gamepkg_x86_irql.h"
#include "gamepkg_x86_memory.h"
#include <stdlib.h>
static int profile(GamePkgX86CpuThread *t, GamePkgX86CpuIrql *irql) {
  GamePkgX86CpuExecution e;
  return gamepkg_x86_cpu_snapshot_irql(t, irql) || !irql->required ||
                 gamepkg_x86_cpu_snapshot_execution(t, &e) || e.interrupt.cpl ||
                 e.code.cs_base || !(e.code.cs_attributes & 0x400u)
             ? -1
             : 0;
}
int gamepkg_x86_irql_initialize(GamePkgX86CpuThread *t, GamePkgX86Memory *m,
                                uint32_t a, uint32_t ip, uint32_t bp) {
  GamePkgX86CpuIrql image;
  if (profile(t, &image) || image.configured ||
      !gamepkg_x86_memory_is_bound(m, 0))
    return -1;
  uint32_t word[4] = {0, 0, 0, 0};
  gamepkg_x86_memory_write(a, word, 1, 1, 0, ip, bp);
  /* All metadata conditions were checked while retaining exclusive ownership;
   * immutable RAM permissions and no guest reentry make commit nonfallible. */
  if (gamepkg_x86_cpu_bind_irql(t, m, a))
    abort();
  return 0;
}
GamePkgX86IrqlResult gamepkg_x86_irql(GamePkgX86CpuThread *t, uint32_t op,
                                      uint32_t level, uint32_t ip,
                                      uint32_t bp) {
  GamePkgX86CpuIrql image;
  if (op > 2 || level > 31 || profile(t, &image) || !image.configured ||
      !gamepkg_x86_memory_is_bound(image.memory, 0))
    return (GamePkgX86IrqlResult){-1, 0, 0};
  if ((op == 1 && level < image.level) || (op == 2 && level > image.level))
    return (GamePkgX86IrqlResult){-2, image.level, image.level};
  uint32_t word[4];
  gamepkg_x86_memory_read(image.address, word, 1, ip, bp);
  if (word[0] != image.level)
    return (GamePkgX86IrqlResult){-2, image.level, image.level};
  if (op) {
    word[0] = level;
    gamepkg_x86_memory_write(image.address, word, 1, 1, 0, ip, bp);
    if (gamepkg_x86_cpu_commit_irql(t, image.level, level))
      abort();
  }
  return (GamePkgX86IrqlResult){0, image.level, op ? level : image.level};
}
