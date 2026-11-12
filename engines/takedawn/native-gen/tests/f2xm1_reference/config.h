#ifndef CONFIG_H
#define CONFIG_H
#include <stdint.h>
#include <assert.h>
typedef uint8_t Bit8u;typedef int8_t Bit8s;typedef uint16_t Bit16u;typedef int16_t Bit16s;typedef uint32_t Bit32u;typedef int32_t Bit32s;typedef uint64_t Bit64u;typedef int64_t Bit64s;
#define BX_LITTLE_ENDIAN 1
#define BX_CPP_INLINE inline
#define BX_CONST64(x) UINT64_C(x)
#define BX_HAVE___BUILTIN_CLZ 1
#define BX_HAVE_INT128 0
#define SOFTFLOAT_FAST_DIV64TO32 1
#define BX_ASSERT(x) assert(x)
#endif
