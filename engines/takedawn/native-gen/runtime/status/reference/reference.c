/* Maintainer-only, separately compiled original pinned Wine NoTeb body/table.
 * Full license and notices in ../upstream/COPYING.LIB and included sources.
 * This source is never linked into the player. */
#include <stdint.h>
typedef uint32_t NTSTATUS, ULONG, DWORD;
#define WINE_NO_LONG_TYPES 1
#define ARRAY_SIZE(a) (sizeof(a) / sizeof((a)[0]))
#define WINAPI
#define HIWORD(a) ((uint32_t)(a) >> 16)
#define LOWORD(a) ((uint32_t)(a) & 0xffffu)
#define WARN(...) ((void)0)
/* Table constants require the original status/error definitions first. */
// clang-format off
#include "../upstream/ntstatus.h"
#include "../upstream/winerror.h"
#include "../upstream/error.h"
// clang-format on
#define RtlNtStatusToDosErrorNoTeb gamepkg_status_reference
#include "no_teb.inc"
