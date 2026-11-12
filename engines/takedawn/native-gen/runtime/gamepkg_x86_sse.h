#ifndef GAMEPKG_X86_SSE_H
#define GAMEPKG_X86_SSE_H
#include <stdint.h>
/* MIN/MAX source-bit selection: op0=min,1=max; lanes1/4. Packed conditions
 * aggregate before commit. Supports in-place results, preserves raw SNaNs and
 * second-source signed zero on equal zeros. No native float/FTZ rounding. */
void gamepkg_x86_select_f32(const uint32_t *left, const uint32_t *right,
                            uint32_t *result, uint32_t lanes,
                            uint32_t operation, uint32_t eip, uint32_t ebp);
/* Binary32 bit patterns, not host float expressions. Return only defined
 * CF/PF/ZF bits after applying COMISS/UCOMISS exceptions to guest MXCSR. */
uint32_t gamepkg_x86_compare_f32(uint32_t a, uint32_t b, int unordered_silent,
                                 uint32_t eip, uint32_t ebp);
/* CMPSS/CMPPS, predicates0..7 only. Quiet predicates0/3/4/7 raise IE only
 * for SNaN; signaling predicates1/2/5/6 also raise IE for QNaN.
 * Output is a raw all-ones/zero mask; packed status precedes all stores. */
void gamepkg_x86_predicate_f32(const uint32_t *left, const uint32_t *right,
                               uint32_t *result, uint32_t lanes,
                               uint32_t predicate, uint32_t eip, uint32_t ebp);
/* 0=signed32 to binary32,1=binary32 to signed32 using MXCSR rounding,
 * 2=binary32 to signed32 truncating. Result is a bit pattern. Raises guest
 * IE/PE before returning; no host float casts, DAZ or FTZ transformation. */
uint32_t gamepkg_x86_convert_f32(uint32_t value, uint32_t operation,
                                 uint32_t eip, uint32_t ebp);
/* Non-committing conversion primitive. Explicit RC0..3, new IE/PE only;
 * no TLS/MXCSR/host-FP changes. Used for packed exception aggregation. */
uint32_t gamepkg_x86_convert_bits(uint32_t value, uint32_t operation,
                                  uint32_t rounding, uint32_t *exceptions);
#endif
