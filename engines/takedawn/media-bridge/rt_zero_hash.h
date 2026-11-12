/* Exact shortcut for the reviewed renderer's four-lane FNV recurrence.
 * Inspect every byte in a zero prefix, then apply the corresponding prime
 * power modulo 2^64. Nonzero data follows the original recurrence. Nothing is
 * cached: writes, ownership rules, bounds and hash results remain unchanged. */
#ifndef GAMEPKG_RT_ZERO_HASH_H
#define GAMEPKG_RT_ZERO_HASH_H

#include <stdint.h>
#include <string.h>
#if defined(__AVX2__) && !defined(GAMEPKG_RT_HASH_PORTABLE)
#include <immintrin.h>
#elif defined(__SSE2__) && !defined(GAMEPKG_RT_HASH_PORTABLE)
#include <emmintrin.h>
#endif

uint64_t nv2a_tex_hash(const uint8_t *, uint32_t, uint64_t);

static inline uint32_t gamepkg_rt_zero_prefix(const uint8_t *p, uint32_t n)
{
    uint32_t i = 0;
    while (n - i >= 32) {
#if defined(__AVX2__) && !defined(GAMEPKG_RT_HASH_PORTABLE)
        __m256i v = _mm256_loadu_si256((const __m256i *)(const void *)(p + i));
        if (!_mm256_testz_si256(v, v))
            break;
#elif defined(__SSE2__) && !defined(GAMEPKG_RT_HASH_PORTABLE)
        __m128i a = _mm_loadu_si128((const __m128i *)(const void *)(p + i));
        __m128i b = _mm_loadu_si128((const __m128i *)(const void *)(p + i + 16));
        if (_mm_movemask_epi8(_mm_cmpeq_epi8(_mm_or_si128(a, b), _mm_setzero_si128())) != 65535)
            break;
#else
        uint64_t words[4];
        memcpy(words, p + i, sizeof words);
        if (words[0] | words[1] | words[2] | words[3])
            break;
#endif
        i += 32;
    }
    return i;
}

static inline uint64_t gamepkg_rt_prime_power(uint32_t n)
{
    uint64_t prime = UINT64_C(0x100000001b3), value = 1;
    while (n) {
        if (n & 1)
            value *= prime;
        prime *= prime;
        n >>= 1;
    }
    return value;
}

static inline uint64_t gamepkg_rt_hash(const uint8_t *p, uint32_t n, uint64_t seed)
{
    /* Keep the original path for short inputs and its upper-length boundary.
     * The caller retains its existing resolved-memory-window clamp. */
    if (n < 256 || n > UINT32_MAX - 32u)
        return nv2a_tex_hash(p, n, seed);
    uint32_t i = gamepkg_rt_zero_prefix(p, n);
    if (i < 256)
        return nv2a_tex_hash(p, n, seed);
    uint64_t factor = gamepkg_rt_prime_power(i / 32);
    uint64_t lanes[4] = {
        seed * factor, (seed ^ 1u) * factor,
        (seed ^ 2u) * factor, (seed ^ 3u) * factor
    };
    for (; i + 32 <= n; i += 32) {
        uint64_t words[4];
        memcpy(words, p + i, sizeof words);
        lanes[0] = (lanes[0] ^ words[0]) * UINT64_C(0x100000001b3);
        lanes[1] = (lanes[1] ^ words[1]) * UINT64_C(0x100000001b3);
        lanes[2] = (lanes[2] ^ words[2]) * UINT64_C(0x100000001b3);
        lanes[3] = (lanes[3] ^ words[3]) * UINT64_C(0x100000001b3);
    }
    for (; i < n; i++)
        lanes[0] = (lanes[0] ^ p[i]) * UINT64_C(0x100000001b3);
    return (lanes[0] ^ (lanes[1] << 1) ^ (lanes[2] << 2) ^ (lanes[3] << 3))
           * UINT64_C(0x100000001b3);
}

#endif
