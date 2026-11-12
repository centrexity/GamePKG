#ifndef GAMEPKG_X86_VIRTUAL_MEMORY_H
#define GAMEPKG_X86_VIRTUAL_MEMORY_H
#include "gamepkg_x86_cpu.h"
#include "gamepkg_x86_virtual_pages.h"
typedef struct GamePkgX86VirtualMemory GamePkgX86VirtualMemory;
/* Reviewed NT-style host frontend. Borrowed CPU/RAM outlive quiescent release.
 * Requires configured pages, exact active CPU owner, flat/default32 CPL0,
 * required supervisor IRQL at PASSIVE0 and a live fault boundary. Does not
 * alter GPR/FP/flags/IRQL/retirement. Caller owns stdcall and checked return.
 * Input pointees read before validation; success output spans admitted before
 * mutation. Outputs are written BaseAddress then RegionSize (aliases retain
 * that order). Affected mappings overlapping output pointers stop unsupported
 * rather than dereference freed/protected pointers after commit. No native
 * lock crosses RAM/faults. Pointer-wrap returns host-1, not wrapped access.
 * status0 supplies guest NTSTATUS,-1 host/profile/span,-2 recognized feature
 * not qualified yet. MEM_RESET/NOZERO, guard/cache/COW and physical PFNs,
 * dynamic instruction fetch and full exception/retail failure precedence are
 * separate work; no successful no-op stands in for those facilities.
 */
GamePkgX86VirtualMemory *gamepkg_x86_virtual_memory_create(GamePkgX86Cpu *,
                                                           GamePkgX86Memory *);
void gamepkg_x86_virtual_memory_release(GamePkgX86VirtualMemory *);
typedef struct GamePkgX86VirtualMemoryResult {
  int status;
  uint32_t ntstatus, base, length;
} GamePkgX86VirtualMemoryResult;
GamePkgX86VirtualMemoryResult gamepkg_x86_virtual_memory_allocate(
    GamePkgX86VirtualMemory *, GamePkgX86CpuThread *,
    uint32_t base_address_pointer, uint32_t zero_bits, uint32_t size_pointer,
    uint32_t allocation_type, uint32_t protection, uint32_t eip, uint32_t ebp);
GamePkgX86VirtualMemoryResult gamepkg_x86_virtual_memory_free(
    GamePkgX86VirtualMemory *, GamePkgX86CpuThread *,
    uint32_t base_address_pointer, uint32_t size_pointer, uint32_t free_type,
    uint32_t eip, uint32_t ebp);
#endif
