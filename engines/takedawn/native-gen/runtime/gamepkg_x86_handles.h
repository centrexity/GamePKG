#ifndef GAMEPKG_X86_HANDLES_H
#define GAMEPKG_X86_HANDLES_H
#include "gamepkg_x86_cpu.h"
#include <stddef.h>
typedef struct GamePkgX86Handles GamePkgX86Handles;
typedef struct GamePkgX86HandleTicket GamePkgX86HandleTicket;
/* Typed, per-CPU/process host object table. Guest handles are nonzero aligned
 * uint32 IDs, never host pointers/fds. IDs are unique among live objects; the
 * namespace cycles, skipping live IDs. Exhaustion is the natural32-bit limit,
 * not an execution/handle-count quota. Native object storage/callbacks are
 * trusted, immutable and must remain live until the last owned handle closes.
 * All guest-facing operations require exact active CPU, bound supervisor RAM,
 * explicit flat32 CPL0, configured PASSIVE0 and a live fault boundary.
 * Prepare runs that admission before allocating a ticket. Commit/cancel do
 * metadata-only owner checks; they cannot fault after external I/O succeeds.
 * No mutex crosses a RAM access, fault or trusted finalizer. Finalizers must
 * preserve guest registers/bindings/CPU, return normally, and never reenter
 * this table. A pending reservation excludes reentrant table mutations.
 */
GamePkgX86Handles *gamepkg_x86_handles_create(GamePkgX86Cpu *,
                                              GamePkgX86Memory *);
int gamepkg_x86_handles_in_domain(GamePkgX86Handles *, GamePkgX86Cpu *,
                                  GamePkgX86Memory *);
int gamepkg_x86_handles_owner(GamePkgX86Handles *, GamePkgX86CpuThread *,
                              uint32_t eip, uint32_t ebp);
/*0=host operation valid,-1=host/profile/protocol,1=metadata/ID space
 * exhausted*/
typedef struct GamePkgX86HandleReservation {
  int status;
  GamePkgX86HandleTicket *ticket;
} GamePkgX86HandleReservation;
GamePkgX86HandleReservation gamepkg_x86_handles_prepare(GamePkgX86Handles *,
                                                        GamePkgX86CpuThread *,
                                                        uint32_t eip,
                                                        uint32_t ebp);
typedef int (*GamePkgX86HandleClose)(void *owner, const uint32_t data[4],
                                     uint32_t terminal, uint32_t *ntstatus);
typedef struct GamePkgX86HandleValue {
  uint32_t kind, access;
  void *owner;
  uint32_t data[4];
  GamePkgX86HandleClose close;
} GamePkgX86HandleValue;
/* Host-defined tags, shared by future file/event/thread object families. */
enum { GAMEPKG_X86_HANDLE_DIRECTORY = 1, GAMEPKG_X86_HANDLE_FILE = 2 };
int gamepkg_x86_handles_commit(GamePkgX86Handles *, GamePkgX86CpuThread *,
                               GamePkgX86HandleTicket *, GamePkgX86HandleValue,
                               uint32_t *guest_handle);
int gamepkg_x86_handles_cancel(GamePkgX86Handles *, GamePkgX86CpuThread *,
                               GamePkgX86HandleTicket *);
typedef struct GamePkgX86HandleLookup {
  int status;
  uint32_t ntstatus;
  GamePkgX86HandleValue value;
} GamePkgX86HandleLookup;
GamePkgX86HandleLookup gamepkg_x86_handles_lookup(GamePkgX86Handles *,
                                                  GamePkgX86CpuThread *,
                                                  uint32_t handle, uint32_t eip,
                                                  uint32_t ebp);
typedef struct GamePkgX86HandleResult {
  int status;
  uint32_t ntstatus;
} GamePkgX86HandleResult;
GamePkgX86HandleResult gamepkg_x86_handles_close(GamePkgX86Handles *,
                                                 GamePkgX86CpuThread *,
                                                 uint32_t handle, uint32_t eip,
                                                 uint32_t ebp);
/* Nonfaulting host metadata query, only with a live/exclusively owned table. */
size_t gamepkg_x86_handles_owned(GamePkgX86Handles *, void *owner);
/* Quiescent teardown. terminal0 requires empty table. terminal1 calls native
 * finalizers with terminal1 (no guest resume). Failure retains remaining nodes
 * and the table; it never invents successful close or discards native objects.
 */
int gamepkg_x86_handles_release(GamePkgX86Handles *, uint32_t terminal);
/* Terminal finalizers without freeing the table. Drain, release dependent
 * services, then release the empty table, while all borrowed backends live. */
int gamepkg_x86_handles_drain(GamePkgX86Handles *);
#endif
