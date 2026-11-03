#ifndef GAMEPKG_X86_VIRTUAL_PAGES_H
#define GAMEPKG_X86_VIRTUAL_PAGES_H
#include "gamepkg_x86_memory.h"
/* Trusted host page store, not a guest allocator/import ABI or page-table
 * walker. Configure exactly once, before binding/sharing the RAM object.
 * The page-aligned virtual arena must be disjoint from every fixed region.
 * Physical limit counts actual4096-byte backing pages, independently of VA
 * reservations. It is an explicit machine/resource profile, not fuel.
 * Ownership is the RAM object's; all bindings must quiesce before final
 * release. Mutations require a bound RAM object and the shared CPU's exclusive
 * host boundary; never mutate between another service's admission and commit.
 * The RAM lock serializes access/mutation and is released before any fault.
 * No native callback, host executable page or guest/native pointer conversion.
 */
int gamepkg_x86_virtual_configure(GamePkgX86Memory *, uint32_t base,
                                  uint32_t length, uint32_t physical_limit);
/* Alternative initial configuration: fixed regions within the arena are
 * excluded, rounded outward to pages and merged. They cannot be allocated,
 * protected, freed or accessed through the pager. Querying such a fixed page
 * reports host-2 until a complete loader/static-map query ABI is supplied.
 * physical_limit is the available backing AFTER the loader's fixed charge. */
int gamepkg_x86_virtual_configure_excluding_fixed(GamePkgX86Memory *,
                                                  uint32_t base,
                                                  uint32_t length,
                                                  uint32_t physical_limit);
int gamepkg_x86_virtual_is_configured(GamePkgX86Memory *);
/* Core geometry is exact/page-aligned; the NT wrapper owns ABI rounding.
 * status0=success,-1=host misuse/geometry,-2=recognized unimplemented
 * protection, 1=conflict/outside arena,2=backing/metadata exhausted,3=not one
 * reservation, 4=not entirely committed. All failures leave
 * mappings/content/statistics unchanged. Reserve+commit is one transaction.
 * Commit preserves existing bytes and applies protection to the whole selected
 * range, with zeroed new pages. Protect requires all selected pages committed.
 * Decommit is idempotent. Partial release splits reservations without copying
 * backing bytes. Supported basic NT protection bits:1,2,4,10,20,40(hex).
 * Execute permission is remembered only; generated instruction-fetch/code
 * mutation/SEH/guard, cache attributes, physical PFNs/aliases and full device
 * paging are separate. No-access and uncommitted pages report nonpresent PF;
 * readonly writes report present protection PF. Every existing RAM service uses
 * this same resolver.
 */
typedef struct GamePkgX86VirtualResult {
  int status;
  uint32_t base, length;
} GamePkgX86VirtualResult;
GamePkgX86VirtualResult
gamepkg_x86_virtual_reserve(GamePkgX86Memory *, uint32_t hint, uint32_t length,
                            uint32_t maximum_address, uint32_t top_down,
                            uint32_t protection, uint32_t commit);
GamePkgX86VirtualResult gamepkg_x86_virtual_commit(GamePkgX86Memory *,
                                                   uint32_t base,
                                                   uint32_t length,
                                                   uint32_t protection);
GamePkgX86VirtualResult gamepkg_x86_virtual_protect(GamePkgX86Memory *,
                                                    uint32_t base,
                                                    uint32_t length,
                                                    uint32_t protection);
GamePkgX86VirtualResult gamepkg_x86_virtual_free(GamePkgX86Memory *,
                                                 uint32_t base, uint32_t length,
                                                 uint32_t release);
typedef struct GamePkgX86VirtualInfo {
  int status;
  uint32_t base, allocation_base, allocation_protection, length, state,
      protection, type;
} GamePkgX86VirtualInfo;
GamePkgX86VirtualInfo gamepkg_x86_virtual_query(GamePkgX86Memory *,
                                                uint32_t address);
typedef struct GamePkgX86VirtualStats {
  uint32_t reserved, committed, physical_limit;
} GamePkgX86VirtualStats;
int gamepkg_x86_virtual_stats(GamePkgX86Memory *, GamePkgX86VirtualStats *);
#endif
