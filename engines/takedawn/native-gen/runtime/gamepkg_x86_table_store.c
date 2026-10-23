#include "gamepkg_x86_table_store.h"
#include "gamepkg_x86_memory.h"
int gamepkg_x86_cpu_store_table(GamePkgX86CpuThread *thread, uint32_t op,
                                uint32_t operand16, uint32_t address,
                                uint32_t eip, uint32_t ebp) {
  if (op > 1 || operand16 > 1)
    return -1;
  GamePkgX86InterruptState state;
  GamePkgX86Tables tables;
  if (gamepkg_x86_cpu_snapshot_tables(thread, &tables) ||
      gamepkg_x86_cpu_snapshot(thread, &state) || !state.protected_mode ||
      (state.flags & 0x20000u))
    return -1;
  const GamePkgX86TableRegister value = op ? tables.idtr : tables.gdtr;
  return gamepkg_x86_store_table_image(value, operand16, address, eip, ebp);
}
int gamepkg_x86_store_table_image(GamePkgX86TableRegister value,
                                  uint32_t operand16, uint32_t address,
                                  uint32_t eip, uint32_t ebp) {
  if (operand16 > 1 || value.limit > 65535)
    return -1;
  uint32_t base = operand16 ? value.base & 0x00ffffffu : value.base;
  unsigned char bytes[6] = {
      (unsigned char)value.limit,  (unsigned char)(value.limit >> 8),
      (unsigned char)base,         (unsigned char)(base >> 8),
      (unsigned char)(base >> 16), (unsigned char)(base >> 24)};
  gamepkg_x86_memory_bytes(address, bytes, 6, 1, eip, ebp);
  return 0;
}
