#ifndef GAMEPKG_X86_APPROX_H
#define GAMEPKG_X86_APPROX_H
#include <stdint.h>
/* Reviewed host service, not guest SDK. Raw binary32 fixed approximation.
 * op0=RCP,1=RSQRT; lanes1/4. Scalar high lanes preserve destination.
 * In-place/overlapping source-result supported through complete staging.
 * No SIMD numerical exception or MXCSR modification, even with unmasked bits.
 * Caller stages all memory/alignment faults before invoking this service. */
void gamepkg_x86_approx_f32(const uint32_t *source, uint32_t *result,
                            uint32_t lanes, uint32_t operation);
#endif
