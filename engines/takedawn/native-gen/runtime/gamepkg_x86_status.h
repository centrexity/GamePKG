#ifndef GAMEPKG_X86_STATUS_H
#define GAMEPKG_X86_STATUS_H
#include <stdint.h>
/* Pure integer NTSTATUS->DOS mapping, pinned Wine9.0 compatibility profile.
 * No host ntdll/TEB/TLS access, guest RAM/GPR/FP/IRQL/flags or retirement.
 * Xbox caller code owns its own last-error storage. Host HLE separately reads
 * the one stdcall argument and performs a checked return. Not an unqualified
 * assertion of every original retail Xbox kernel's extended-status table.
 * LGPL-2.1-or-later; retain status/ source, notices and rebuild/relink inputs.
 */
uint32_t gamepkg_x86_status_to_dos(uint32_t status);
#endif
