#ifndef GAMEPKG_X86_FLAGS_H
#define GAMEPKG_X86_FLAGS_H
#include <stdint.h>
/* Reviewed host eager-register API, fixed protected-mode CPL0/32bit stack.
 * Image masks reserved bits, forces bit1 and combines DF from its TLS bridge;
 * RF/VM are cleared in the PUSH image without mutating the source registers.
 * Load bytes2/4 updates defined writable bits, preserves VIF/VIP/VM, clears RF
 * and publishes DF to both representations. Word loads preserve upper flags.
 * This is register data transfer, not V86/CPL switching, interrupt scheduling,
 * single-step/debug delivery or a legacy lazy-flags materialization bridge. */
uint32_t gamepkg_x86_flags_image(void);
void gamepkg_x86_flags_load(uint32_t value, uint32_t bytes);
#endif
