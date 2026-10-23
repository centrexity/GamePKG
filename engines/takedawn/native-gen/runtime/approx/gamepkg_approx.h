#ifndef GAMEPKG_APPROX_H
#define GAMEPKG_APPROX_H
#include <stdint.h>
/* Fixed pinned SSE approximation profile. kind0=RCP,1=RSQRT; raw binary32.
 * No host/guest floating state or TLS mutation. Caller validates kind. */
uint32_t gamepkg_approx_f32(uint32_t bits, uint32_t sqrt_reciprocal);
#endif
