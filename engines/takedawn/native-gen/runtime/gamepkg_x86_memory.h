#ifndef GAMEPKG_X86_MEMORY_H
#define GAMEPKG_X86_MEMORY_H
#include <stddef.h>
#include <stdint.h>
/* Reviewed host API, never guest-provided pointers/callbacks. Immutable region
 * descriptors hold borrowed host RAM, which must outlive the object/bindings.
 * Maps do not wrap; an access's linear32 bytes may cross into a second map.
 * Permissions are resolved host capabilities, not an x86 page-table walker.
 * No device/MMIO pointer may be registered as RAM. Optional virtual pages use
 * this same resolver/lock; see gamepkg_x86_virtual_pages.h. Fixed descriptors
 * stay immutable. Page mutation must be serialized at exclusive CPU host
 * boundaries, never between another service's admission and commit. */
typedef struct GamePkgX86Memory GamePkgX86Memory;
typedef struct {
  uint32_t base, length, permissions;
  unsigned char *data;
} GamePkgX86Region;
enum { GAMEPKG_X86_READ = 1, GAMEPKG_X86_WRITE = 2 };
/* At most64 nonoverlapping regions; user_fault=0/1 supplies PF U/S bit.
 * ReturnsNULL on invalid descriptors/allocation failure. Owner and bindings
 * hold references; each host worker must unbind before exiting. */
GamePkgX86Memory *gamepkg_x86_memory_create(const GamePkgX86Region *regions,
                                            size_t count, uint32_t user_fault);
void gamepkg_x86_memory_release(GamePkgX86Memory *memory);
void gamepkg_x86_memory_bind(GamePkgX86Memory *memory);
/* Nonmutating identity/profile guard for trusted services. No RAM access. */
int gamepkg_x86_memory_is_bound(const GamePkgX86Memory *memory,
                                 uint32_t user_fault);
/* Call within a live fault boundary. Whole-span admission precedes reads or
 * selected-byte writes. Staging arrays are16 host bytes, never guest aliases.
 * Accesses are serialized across workers bound to this object; other host
 * access to the borrowed arena must use the same synchronization domain. */
void gamepkg_x86_memory_read(uint32_t address, uint32_t words[4],
                             uint32_t bytes, uint32_t eip, uint32_t ebp);
/* Admit without reading or writing bytes, for instructions whose numerical
 * exceptions can suppress a store. Span1..512, write0/1. Permissions
 * and maps are immutable under a live binding; this is not a device callback
 * or a transferable lock/token. A later store still performs admission. */
void gamepkg_x86_memory_check(uint32_t address, uint32_t bytes, uint32_t write,
                              uint32_t eip, uint32_t ebp);
/* Complete byte-image transactions for bounded architectural save areas.
 * Span1..512, write0=read/1=write; caller buffer is private host storage and
 * must not alias backing RAM. Failed reads preserve the complete buffer. */
void gamepkg_x86_memory_bytes(uint32_t address, unsigned char *buffer,
                              uint32_t bytes, uint32_t write, uint32_t eip,
                              uint32_t ebp);
void gamepkg_x86_memory_write(uint32_t address, const uint32_t words[4],
                              uint32_t bytes, uint32_t mask,
                              uint32_t non_temporal, uint32_t eip,
                              uint32_t ebp);
/* One serialized checked RAM read/modify/write transaction. op0=XCHG,
 * op1=XADD, op2=CMPXCHG; byte widths1/2/4. Returns old value. Both read and
 * write permission are required even when CMPXCHG fails (old value written).
 * All workers must share the same memory object/synchronization domain.
 * No output register or flags are changed on a fault. No device callbacks. */
uint32_t gamepkg_x86_memory_rmw(uint32_t address, uint32_t bytes, uint32_t op,
                                uint32_t value, uint32_t expected, uint32_t eip,
                                uint32_t ebp);
/* Trusted processor descriptor services, not guest memory instructions.
 * Read a complete eight-byte LE descriptor into two logical words; failed
 * admission leaves both words unchanged. Output is private host storage.
 * Accessed takes the rights-BYTE address (descriptor+5), atomically ORs bit0
 * under this RAM object's shared lock and preserves all other bits. A set
 * bit needs only read permission; a clear bit needs conditional write
 * admission. Faults release locks before delivery and never partially write.
 * These implicit accesses report supervisor PF U/S=0, independent of the
 * initiating CPL/user_fault; they still enforce every host RAM capability.
 * No descriptor validation, code-cache commit, retirement or native host
 * GDT access is performed. A live guest fault boundary is required. */
void gamepkg_x86_memory_descriptor_read(uint32_t address, uint32_t words[2],
                                        uint32_t eip, uint32_t ebp);
void gamepkg_x86_memory_descriptor_accessed(uint32_t rights_address,
                                            uint32_t eip, uint32_t ebp);
void gamepkg_x86_store_fence(void);
/* Hints0=NTA,1=T0,2=T1,3=T2; unmapped/nonreadable addresses are ignored.
 * Compiler-supported host prefetch is advisory, without guest reads/faults. */
void gamepkg_x86_prefetch(uint32_t address, uint32_t hint);
/* Reviewed Xbox RTL critical-section backend, bound to this same RAM domain.
 * op0=initialize,1=try-enter,2=blocking enter,3=leave. The 28-byte guest ABI
 * has a 16-byte synchronization-event image then LockCount/RecursionCount/
 * OwningThread at16/20/24. thread is a stable nonzero guest KTHREAD token
 * supplied by trusted host code, not an OS ID or native pointer. Unused at0.
 * Returns0=success,1=try busy,-1=invalid host op/token/span (no mutation),
 * -2=inconsistent guest counts/ownership or signed count overflow (no mutation
 * for that transition). No fake acquisition, success on exhaustion or timeout.
 * Memory faults propagate after releasing host locks; a live fault boundary
 * is mandatory. Whole transition admission precedes writes. Native condition
 * waits release RAM/metadata locks; guest execution has no count/depth quota.
 * Each participating worker must bind the SAME memory object until return.
 * Do not cancel/unwind/unbind a host worker inside a blocking operation.
 * Host process time/memory watchdogs still apply. Reinitializing a live lock,
 * mutating its fields externally or reusing a token for concurrent workers is
 * invalid. Guest wait-list links, APC/critical-region variants, kernel event
 * handles, thread termination and SEH/import ABI are separate integration work.
 */
int gamepkg_x86_critical_section(uint32_t address, uint32_t thread, uint32_t op,
                                 uint32_t eip, uint32_t ebp);
#endif
