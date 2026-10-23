#include "gamepkg_x86_x87.h"
#include "gamepkg_x86_fault.h"
#include <stdlib.h>
static _Thread_local GamePkgX87State state = {.control = 0x37f, .tags = 0xffff};
static _Thread_local uint16_t segments[4];
static _Thread_local uint32_t data_segment, data_base;
void gamepkg_x86_x87_code_segment(uint32_t cs) {
  if (cs > 65535)
    abort();
  segments[0] = (uint16_t)cs;
}
uint32_t gamepkg_x86_x87_current_code_segment(void) { return segments[0]; }
void gamepkg_x86_x87_bind_segments(uint32_t cs, uint32_t ds, uint32_t ss,
                                   uint32_t fs) {
  if ((cs | ds | ss | fs) > 65535)
    abort();
  segments[0] = (uint16_t)cs;
  segments[1] = (uint16_t)ds;
  segments[2] = (uint16_t)ss;
  segments[3] = (uint16_t)fs;
  data_segment = data_base = 0;
}
void gamepkg_x86_x87_data_segment(uint32_t kind, uint32_t base) {
  if (kind > 2)
    abort();
  data_segment = kind;
  data_base = base;
}
void gamepkg_x86_x87_record(GamePkgX87State *s, uint32_t ip, uint32_t opcode) {
  if (!s || opcode > 0x7ff)
    abort();
  s->instruction_pointer = ip;
  s->opcode = (uint16_t)opcode;
  s->instruction_selector = segments[0];
}
void gamepkg_x86_x87_record_data(GamePkgX87State *s, uint32_t address) {
  if (!s)
    abort();
  s->data_pointer = address - data_base;
  s->data_selector = segments[1 + data_segment];
}
GamePkgX87State *gamepkg_x86_x87_state(void) { return &state; }
void gamepkg_x86_x87_init(void) {
  /* FNINIT does not clear the physical data registers. */
  state.control = 0x37f;
  state.status = 0;
  state.tags = 0xffff;
  state.instruction_pointer = state.data_pointer = 0;
  state.opcode = 0;
  state.instruction_selector = state.data_selector = 0;
}
void gamepkg_x86_x87_clear(void) { state.status &= 0x7f00u; }
void gamepkg_x86_x87_wait(uint32_t eip, uint32_t ebp) {
  /* Deferred x87 exception, before any MMX register/memory access. Full
   * CR0 NE/TS/EM mode selection and guest exception dispatch remain open. */
  if (state.status & ~state.control & 0x3fu)
    gamepkg_x86_x87_error(eip, ebp);
}
void gamepkg_x86_mmx_read_commit(void) {
  state.tags = 0;
  state.status &= (uint16_t)~0x3800u;
}
void gamepkg_x86_mmx_commit(uint32_t reg, uint64_t value) {
  if (reg >= 8)
    abort();
  gamepkg_x86_mmx_read_commit();
  state.significand[reg] = value;
  state.sign_exponent[reg] = 0xffff;
}
void gamepkg_x86_mmx_empty(void) {
  state.tags = 0xffff;
  state.status &= (uint16_t)~0x3800u;
}
uint64_t gamepkg_x86_mmx_shift(uint64_t value, uint64_t count, uint32_t op,
                               uint32_t width) {
  if (op > 2 || (width != 16 && width != 32 && width != 64) ||
      (op == 2 && width == 64))
    abort();
  /* MMX uses the entire unsigned64 count, without the GPR five-bit mask.
   * Handle zero/full-width counts before C shifts. Sign fill is unsigned. */
  if (!count)
    return value;
  uint64_t mask = width == 64 ? UINT64_MAX : (UINT64_C(1) << width) - 1,
           result = 0;
  for (unsigned lane = 0; lane < 64; lane += width) {
    uint64_t x = (value >> lane) & mask, z = 0;
    if (count < width) {
      z = op == 0 ? (x << (unsigned)count) & mask : x >> (unsigned)count;
      if (op == 2 && (x >> (width - 1)))
        z |= mask ^ (mask >> (unsigned)count);
    } else if (op == 2 && (x >> (width - 1)))
      z = mask;
    result |= z << lane;
  }
  return result;
}
uint64_t gamepkg_x86_mmx_binary(uint64_t a, uint64_t b, uint32_t op,
                                uint32_t width) {
  if (op < 4) {
    switch (op) {
    case 0:
      return a & b;
    case 1:
      return ~a & b;
    case 2:
      return a | b;
    default:
      return a ^ b;
    }
  }
  if (op > 19 || (width != 8 && width != 16 && width != 32) ||
      (op >= 8 && op <= 11 && width == 32) ||
      (op >= 12 && op <= 15 && width != 16) || (op == 16 && width == 8) ||
      (op == 17 && width != 16))
    abort();
  uint64_t result = 0, mask = (UINT64_C(1) << width) - 1,
           sign = UINT64_C(1) << (width - 1);
  if (op == 15) {
    /* The sole PMADDWD overflow (four 0x8000 words) wraps to 0x80000000.
     * Accumulate in signed64, then retain the architectural unsigned32. */
    for (unsigned shift = 0; shift < 64; shift += 32) {
      int64_t sum = 0;
      for (unsigned lane = 0; lane < 2; ++lane) {
        unsigned n = shift + 16 * lane;
        int64_t x = (int64_t)(((a >> n) & mask) ^ sign) - (int64_t)sign;
        int64_t y = (int64_t)(((b >> n) & mask) ^ sign) - (int64_t)sign;
        sum += x * y;
      }
      result |= ((uint64_t)sum & UINT64_C(0xffffffff)) << shift;
    }
    return result;
  }
  if (op == 16 || op == 17) {
    unsigned narrow = width / 2;
    uint64_t outmask = (UINT64_C(1) << narrow) - 1;
    int64_t lower = op == 17 ? 0 : -(INT64_C(1) << (narrow - 1));
    int64_t upper =
        op == 17 ? (int64_t)outmask : (INT64_C(1) << (narrow - 1)) - 1;
    for (unsigned half = 0; half < 2; ++half) {
      uint64_t input = half ? b : a;
      for (unsigned lane = 0; lane < 64 / width; ++lane) {
        int64_t x = (int64_t)(((input >> (lane * width)) & mask) ^ sign) -
                    (int64_t)sign;
        if (x < lower) {
          x = lower;
        }
        if (x > upper) {
          x = upper;
        }
        result |= ((uint64_t)x & outmask)
                  << ((half * (64 / width) + lane) * narrow);
      }
    }
    return result;
  }
  if (op >= 18) {
    unsigned start = op == 19 ? 32 : 0;
    for (unsigned lane = 0; lane < 32 / width; ++lane) {
      unsigned input = start + lane * width, out = 2 * lane * width;
      result |= ((a >> input) & mask) << out;
      result |= ((b >> input) & mask) << (out + width);
    }
    return result;
  }
  for (unsigned shift = 0; shift < 64; shift += width) {
    uint64_t x = (a >> shift) & mask, y = (b >> shift) & mask, z;
    if (op == 4)
      z = (x + y) & mask;
    else if (op == 5)
      z = (x - y) & mask;
    else if (op == 6)
      z = x == y ? mask : 0;
    else if (op == 7)
      z = (x ^ sign) > (y ^ sign) ? mask : 0;
    else if (op == 9)
      z = x + y > mask ? mask : x + y;
    else if (op == 11)
      z = x < y ? 0 : x - y;
    else if (op == 14)
      z = (x * y) >> 16;
    else {
      int64_t sx = (int64_t)(x ^ sign) - (int64_t)sign,
              sy = (int64_t)(y ^ sign) - (int64_t)sign;
      int64_t value = op == 8 ? sx + sy : op == 10 ? sx - sy : sx * sy;
      if (op == 8 || op == 10) {
        if (value < -(int64_t)sign) {
          value = -(int64_t)sign;
        }
        if (value > (int64_t)(sign - 1)) {
          value = (int64_t)(sign - 1);
        }
      }
      z = (uint64_t)value;
      if (op == 13)
        z >>= 16;
      z &= mask;
    }
    result |= z << shift;
  }
  return result;
}

uint32_t gamepkg_x86_x87_control(void) { return state.control; }
uint32_t gamepkg_x86_x87_status(void) { return state.status; }
uint64_t gamepkg_x86_mmx_read(uint32_t reg) {
  if (reg >= 8)
    abort();
  return state.significand[reg];
}
