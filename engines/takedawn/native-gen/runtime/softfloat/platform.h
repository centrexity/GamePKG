/* GamePKG's reviewed generic C11 platform for the selected SoftFloat3e F32
 * sources. No CPU-specific intrinsics, host float types or 128bit extensions. */
#ifndef GAMEPKG_SOFTFLOAT_PLATFORM_H
#define GAMEPKG_SOFTFLOAT_PLATFORM_H
#include <limits.h>
#include <stdint.h>
#if CHAR_BIT != 8
#error GamePKG requires eight-bit bytes
#endif
#if defined(_WIN32) || (defined(__BYTE_ORDER__) && __BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__)
#define LITTLEENDIAN 1
#elif !defined(__BYTE_ORDER__) || __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error Unknown host byte order
#endif
#define INLINE inline
#define INLINE_LEVEL 1
#define THREAD_LOCAL _Thread_local
extern THREAD_LOCAL uint_fast8_t gamepkg_softfloat_unbounded_inexact;
void gamepkg_softfloat_ext80_observe(uint32_t sign,int32_t exponent,const uint32_t *words);
#define SOFTFLOAT_FAST_DIV64TO32 1
#define SOFTFLOAT_ROUND_ODD 1
#endif
