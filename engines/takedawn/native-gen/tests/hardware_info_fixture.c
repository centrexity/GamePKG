/* Public nxdk CC0 ABI: ULONG Flags; UCHAR GpuRevision,McpRevision,reserved[2].
 * Values describe explicit virtual profiles; no physical revision is inferred.
 */
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_memory.h"
#include <assert.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
typedef struct HardwareAbi {
  uint32_t flags;
  unsigned char gpu, mcp, reserved[2];
} HardwareAbi;
_Static_assert(sizeof(HardwareAbi) == 8 && offsetof(HardwareAbi, gpu) == 4 &&
                   offsetof(HardwareAbi, mcp) == 5 &&
                   offsetof(HardwareAbi, reserved) == 6,
               "Xbox ABI");
typedef struct VersionAbi {
  uint16_t major, minor, build, qfe;
} VersionAbi;
_Static_assert(sizeof(VersionAbi) == 8 && offsetof(VersionAbi, minor) == 2 &&
                   offsetof(VersionAbi, build) == 4 &&
                   offsetof(VersionAbi, qfe) == 6,
               "Xbox version ABI");
struct Profile {
  uint32_t flags;
  unsigned char gpu, mcp, bytes[8];
  uint16_t version[4];
  unsigned char version_bytes[8];
};
static const struct Profile profiles[] = {
    /* ROWS */
};
static unsigned char backing[8], version_backing[8];
static uint32_t write_address;
static void attempt_write(void) {
  uint32_t words[4] = {0xdeadbeef, 0xcafebabe, 0, 0};
  gamepkg_x86_memory_write(write_address, words, 8, 255, 0, 0x1234, 0xabcdef12);
}
int main(void) {
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
  _Static_assert(sizeof(void *) == 4, "actual32 required");
#if __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error Actual big endian required
#endif
#endif
  g_eflags = 0x202;
  g_df = 0;
  for (size_t i = 0; i < sizeof(profiles) / sizeof(profiles[0]); ++i) {
    const struct Profile *p = &profiles[i];
    HardwareAbi abi = {p->flags, p->gpu, p->mcp, {0, 0}};
#if defined(_WIN32) || __BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__
    assert(!memcmp(&abi, p->bytes, 8));
#else
    assert(abi.flags == p->flags && abi.gpu == p->gpu && abi.mcp == p->mcp);
#endif
    VersionAbi version = {p->version[0], p->version[1], p->version[2],
                          p->version[3]};
#if defined(_WIN32) || __BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__
    assert(!memcmp(&version, p->version_bytes, 8));
#else
    assert(version.major == p->version[0] && version.build == p->version[2]);
#endif
    memcpy(backing, p->bytes, 8);
    memcpy(version_backing, p->version_bytes, 8);
    GamePkgX86Region regions[2] = {
        {0x80010000, 8, GAMEPKG_X86_READ, backing},
        {0x80010010, 8, GAMEPKG_X86_READ, version_backing}};
    GamePkgX86Memory *memory = gamepkg_x86_memory_create(regions, 2, 0);
    assert(memory);
    gamepkg_x86_memory_bind(memory);
    uint32_t words[4];
    gamepkg_x86_memory_read(0x80010000, words, 8, 0x1234, 0xabcdef12);
    assert(words[0] == p->flags &&
           words[1] == ((uint32_t)p->gpu | ((uint32_t)p->mcp << 8)));
    gamepkg_x86_memory_read(0x80010010, words, 8, 0x1234, 0xabcdef12);
    assert(words[0] ==
               ((uint32_t)p->version[0] | ((uint32_t)p->version[1] << 16)) &&
           words[1] ==
               ((uint32_t)p->version[2] | ((uint32_t)p->version[3] << 16)));
    for (unsigned which = 0; which < 2; ++which) {
      write_address = which ? 0x80010010 : 0x80010000;
      GamePkgX86Fault fault;
      assert(gamepkg_x86_run(attempt_write, &fault) == 1 &&
             fault.vector == 14 && fault.eip == 0x1234 &&
             fault.ebp == 0xabcdef12 && fault.fault_address == write_address &&
             fault.error_code == 3 && !memcmp(backing, p->bytes, 8) &&
             !memcmp(version_backing, p->version_bytes, 8));
    }
    gamepkg_x86_memory_bind(NULL);
    gamepkg_x86_memory_release(memory);
  }
  printf(
      "Static kernel-data ABI: %zu explicit profiles, LE fields/8-byte native "
      "layout and sparse high-VA read-only protection passed; "
      "pointer_bits=%u\n",
      sizeof(profiles) / sizeof(profiles[0]), (unsigned)(sizeof(void *) * 8));
  return 0;
}
