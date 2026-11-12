#include "gamepkg_x86_sse.h"
#include "gamepkg_x86_fault.h"
#include <stdlib.h>

void gamepkg_x86_predicate_f32(const uint32_t *left, const uint32_t *right,
                               uint32_t *result, uint32_t lanes,
                               uint32_t predicate, uint32_t eip, uint32_t ebp) {
  if (!left || !right || !result || (lanes != 1 && lanes != 4) || predicate > 7)
    abort();
  uint32_t staged[4], exceptions = 0;
  for (uint32_t i = 0; i < lanes; ++i) {
    uint32_t a = left[i], b = right[i], aa = a & 0x7fffffffu,
             bb = b & 0x7fffffffu;
    unsigned nan_a = aa > 0x7f800000u, nan_b = bb > 0x7f800000u;
    unsigned unordered = nan_a || nan_b, equal = 0, less = 0;
    if (unordered) {
      if (predicate == 1 || predicate == 2 || predicate == 5 ||
          predicate == 6 || (nan_a && !(a & 0x400000u)) ||
          (nan_b && !(b & 0x400000u)))
        exceptions |= 1;
    } else {
      if ((aa && aa < 0x800000u) || (bb && bb < 0x800000u))
        exceptions |= 2;
      equal = a == b || (!aa && !bb);
      if (!equal)
        less = ((a ^ b) & 0x80000000u) ? a >> 31 : (a >> 31) ? a > b : a < b;
    }
    unsigned match;
    switch (predicate) {
    case 0:
      match = equal;
      break;
    case 1:
      match = less;
      break;
    case 2:
      match = less || equal;
      break;
    case 3:
      match = unordered;
      break;
    case 4:
      match = !equal;
      break;
    case 5:
      match = !less;
      break;
    case 6:
      match = !(less || equal);
      break;
    default:
      match = !unordered;
      break;
    }
    staged[i] = match ? UINT32_MAX : 0;
  }
  gamepkg_x86_sse_status(exceptions, eip, ebp);
  for (uint32_t i = 0; i < lanes; ++i)
    result[i] = staged[i];
}

void gamepkg_x86_select_f32(const uint32_t *left, const uint32_t *right,
                            uint32_t *result, uint32_t lanes,
                            uint32_t operation, uint32_t eip, uint32_t ebp) {
  if (!left || !right || !result || (lanes != 1 && lanes != 4) || operation > 1)
    abort();
  uint32_t staged[4], exceptions = 0;
  for (uint32_t i = 0; i < lanes; ++i) {
    uint32_t a = left[i], b = right[i], aa = a & 0x7fffffffu,
             bb = b & 0x7fffffffu;
    if (aa > 0x7f800000u || bb > 0x7f800000u) {
      /* Both quiet/signaling NaNs raise invalid. The second operand is
       * returned unchanged, including an unquieted signaling payload.
       * Invalid precedes denormal classification within this lane. */
      exceptions |= 1;
      staged[i] = b;
    } else {
      if ((aa && aa < 0x800000u) || (bb && bb < 0x800000u))
        exceptions |= 2;
      if ((!aa && !bb) || a == b)
        staged[i] = b;
      else {
        unsigned less = ((a ^ b) & 0x80000000u) ? a >> 31
                        : (a >> 31)             ? a > b
                                                : a < b;
        staged[i] = (operation ? !less : less) ? a : b;
      }
    }
  }
  gamepkg_x86_sse_status(exceptions, eip, ebp);
  for (uint32_t i = 0; i < lanes; ++i)
    result[i] = staged[i];
}

uint32_t gamepkg_x86_compare_f32(uint32_t a, uint32_t b, int unordered_silent,
                                 uint32_t eip, uint32_t ebp) {
  uint32_t aa = a & 0x7fffffffu, bb = b & 0x7fffffffu;
  int nan_a = aa > 0x7f800000u, nan_b = bb > 0x7f800000u;
  if (nan_a || nan_b) {
    /* NaN classification precedes denormals. In particular a quiet NaN
     * plus a subnormal raises neither condition for UCOMISS. */
    if (!unordered_silent || (nan_a && !(a & 0x400000u)) ||
        (nan_b && !(b & 0x400000u)))
      gamepkg_x86_sse_status(1, eip, ebp);
    return 0x45u;
  }
  if ((aa && aa < 0x800000u) || (bb && bb < 0x800000u))
    gamepkg_x86_sse_status(2, eip, ebp);
  if (a == b || (!aa && !bb))
    return 0x40u;
  if ((a ^ b) & 0x80000000u)
    return (a >> 31u) & 1u;
  return (a & 0x80000000u) ? (a > b) : (a < b);
}

/* All shifts are bounded and all sign/magnitude operations use unsigned
 * representations. Signed32 minimum and invalid float conversions never
 * reach a C signed negation or an out-of-range floating-to-integer cast. */
static int round_up(uint32_t magnitude, uint32_t remainder, uint32_t half,
                    uint32_t sign, uint32_t rounding) {
  if (!remainder)
    return 0;
  if (rounding == 0)
    return remainder > half || (remainder == half && (magnitude & 1u));
  return (rounding == 1 && sign) || (rounding == 2 && !sign);
}
uint32_t gamepkg_x86_convert_bits(uint32_t value, uint32_t operation,
                                  uint32_t rounding, uint32_t *exceptions) {
  if (operation > 2 || rounding > 3 || !exceptions)
    abort();
  *exceptions = 0;
  uint32_t sign = value >> 31u;
  if (operation == 0) {
    uint32_t magnitude = sign ? 0u - value : value;
    if (!magnitude)
      return 0;
    uint32_t top = 0, scan = magnitude;
    while (scan >> 1u) {
      scan >>= 1u;
      ++top;
    }
    uint32_t exponent = 127u + top, significand, remainder = 0;
    if (top <= 23)
      significand = magnitude << (23u - top);
    else {
      uint32_t shift = top - 23u; /* 1..8 */
      significand = magnitude >> shift;
      remainder = magnitude & ((1u << shift) - 1u);
      if (round_up(significand, remainder, 1u << (shift - 1u), sign,
                   rounding)) {
        ++significand;
        if (significand == 0x1000000u) {
          significand >>= 1u;
          ++exponent;
        }
      }
    }
    *exceptions = remainder ? 32u : 0u;
    return (sign << 31u) | (exponent << 23u) | (significand & 0x7fffffu);
  }
  if (operation != 1 && operation != 2)
    abort();
  if (operation == 2)
    rounding = 3;
  uint32_t exponent = (value >> 23u) & 255u, significand = value & 0x7fffffu;
  if (exponent)
    significand |= 0x800000u;
  int power = exponent ? (int)exponent - 127 : -126;
  if (power >= 31) {
    if (power == 31 && sign && significand == 0x800000u)
      return 0x80000000u;
    *exceptions = 1;
    return 0x80000000u;
  }
  uint32_t magnitude, remainder = 0;
  if (power >= 23)
    magnitude = significand << (unsigned)(power - 23);
  else {
    unsigned shift = (unsigned)(23 - power); /* 1..149 */
    uint32_t half;
    if (shift >= 32) {
      magnitude = 0;
      remainder = significand;
      half = 0x80000000u;
    } else {
      magnitude = significand >> shift;
      remainder = significand & ((1u << shift) - 1u);
      half = 1u << (shift - 1u);
    }
    if (round_up(magnitude, remainder, half, sign, rounding))
      ++magnitude;
  }
  /* Conversion instructions signal precision for nonzero subnormals, but
   * never a denormal-operand exception. FTZ does not change integer results. */
  *exceptions = remainder ? 32u : 0u;
  return sign ? 0u - magnitude : magnitude;
}

uint32_t gamepkg_x86_convert_f32(uint32_t value, uint32_t operation,
                                 uint32_t eip, uint32_t ebp) {
  uint32_t flags,
      result = gamepkg_x86_convert_bits(
          value, operation, (gamepkg_x86_get_mxcsr() >> 13) & 3, &flags);
  gamepkg_x86_sse_status(flags, eip, ebp);
  return result;
}
