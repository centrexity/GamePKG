#ifndef GAMEPKG_X86_NV_H
#define GAMEPKG_X86_NV_H
#include "gamepkg_x86_cpu.h"
#include <stddef.h>
typedef struct GamePkgX86Nv GamePkgX86Nv;
/* Reviewed host logical virtual-console values, never guest pointers. Public
 * EEPROM indices only; REG_SZ(1),BINARY(3),DWORD(4). A DWORD has four LE bytes;
 * each value fits the256-byte physical EEPROM format. All bytes are copied at
 * creation. No implicit defaults, host settings, BIOS image, checksum/crypto
 * emulation or persistent writes are supplied here. Missing known indices
 * stop as unconfigured host services, rather than returning fabricated data.
 * Input descriptor/data storage is readable/live through creation. */
typedef struct GamePkgX86NvValue {
  uint32_t index, type, length;
  const unsigned char *data;
} GamePkgX86NvValue;
GamePkgX86Nv *gamepkg_x86_nv_create(GamePkgX86Cpu *cpu,
                                    GamePkgX86Memory *memory,
                                    const GamePkgX86NvValue *values,
                                    size_t count);
/* Quiescent host teardown only: no outstanding queries/host references.
 * CPU and RAM are borrowed and must outlive the service and every query. */
void gamepkg_x86_nv_release(GamePkgX86Nv *service);
typedef struct GamePkgX86NvResult {
  int status; /*0=guest status valid,-1=host/profile/span
                 misuse,-2=unconfigured*/
  uint32_t ntstatus, type, length;
} GamePkgX86NvResult;
/* Active owner in the exact CPU, explicit flat/default32 CPL0 code, exact
 * bound supervisor RAM, mandatory live fault boundary. Never changes GPR/FP,
 * flags, IRQL, retirement or handles. HLE owns stdcall5/NTSTATUS/checked
 * return. Unknown indices return OBJECT_NAME_NOT_FOUND without output
 * access. Short buffers write only optional ResultLength and return
 * BUFFER_TOO_SMALL. Success writes optional ResultLength, required Type, then
 * zeroes ValueLength bytes and overlays the frozen payload, in that order
 * (including aliases). Whole selected spans are checked before ANY write.
 * Pointer spans may not wrap; rejected spans return host-1 unchanged. Tail
 * clearing uses bounded chunks with no invented total byte/iteration quota.
 * Immutable RAM rights plus exclusive CPU ownership serialize guest work;
 * unrelated host arena writers are outside this guarantee. No native locks
 * remain over faults.
 */
GamePkgX86NvResult gamepkg_x86_nv_query(
    GamePkgX86Nv *service, GamePkgX86CpuThread *thread, uint32_t index,
    uint32_t type_address, uint32_t value_address, uint32_t value_length,
    uint32_t result_length_address, uint32_t eip, uint32_t ebp);
#endif
