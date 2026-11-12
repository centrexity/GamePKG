#include "gamepkg_x86_memory.h"
#include "gamepkg_x86_x87.h"
#include <stdlib.h>
static uint32_t read_le(const unsigned char *p, unsigned n) {
  uint32_t v = 0;
  for (unsigned i = 0; i < n; ++i)
    v |= (uint32_t)p[i] << (8 * i);
  return v;
}
static void write_le(unsigned char *p, uint32_t v, unsigned n) {
  for (unsigned i = 0; i < n; ++i)
    p[i] = (unsigned char)(v >> (8 * i));
}
static unsigned classify(uint64_t sig, unsigned se) {
  unsigned e = se & 0x7fff;
  return !e && !sig ? 1 : e && e != 0x7fff && (sig >> 63) ? 0 : 2;
}
static void reclassify(GamePkgX87State *s) {
  for (unsigned i = 0; i < 8; ++i)
    if (((s->tags >> (2 * i)) & 3) != 3)
      s->tags = (uint16_t)((s->tags & ~(3u << (2 * i))) |
                           (classify(s->significand[i], s->sign_exponent[i])
                            << (2 * i)));
}
static void control(GamePkgX87State *s, uint32_t value) {
  s->control = (uint16_t)((value & 0x1f3f) | 0x40);
  s->status &= (uint16_t)~0x8080u;
  if (s->status & ~s->control & 63u)
    s->status |= 0x8080;
}
static void pack(const GamePkgX87State *s, unsigned char image[108],
                 unsigned word16) {
  for (unsigned i = 0; i < 108; ++i)
    image[i] = 0;
  if (word16) {
    write_le(image, s->control, 2);
    write_le(image + 2, s->status, 2);
    write_le(image + 4, s->tags, 2);
    write_le(image + 6, s->instruction_pointer, 2);
    write_le(image + 8, s->instruction_selector, 2);
    write_le(image + 10, s->data_pointer, 2);
    write_le(image + 12, s->data_selector, 2);
  } else {
    write_le(image, 0xffff0000u | s->control, 4);
    write_le(image + 4, 0xffff0000u | s->status, 4);
    write_le(image + 8, 0xffff0000u | s->tags, 4);
    write_le(image + 12, s->instruction_pointer, 4);
    write_le(image + 16, s->instruction_selector | ((uint32_t)s->opcode << 16),
             4);
    write_le(image + 20, s->data_pointer, 4);
    write_le(image + 24, 0xffff0000u | s->data_selector, 4);
  }
  unsigned offset = word16 ? 14 : 28, top = (s->status >> 11) & 7;
  for (unsigned i = 0; i < 8; ++i) {
    unsigned p = (top + i) & 7;
    for (unsigned j = 0; j < 8; ++j)
      image[offset + 10 * i + j] =
          (unsigned char)(s->significand[p] >> (8 * j));
    write_le(image + offset + 10 * i + 8, s->sign_exponent[p], 2);
  }
}
static void unpack(GamePkgX87State *s, const unsigned char *image,
                   unsigned word16, unsigned registers) {
  unsigned cw;
  if (word16) {
    cw = read_le(image, 2);
    s->status = (uint16_t)read_le(image + 2, 2);
    s->tags = (uint16_t)read_le(image + 4, 2);
    s->instruction_pointer = read_le(image + 6, 2);
    s->instruction_selector = (uint16_t)read_le(image + 8, 2);
    s->data_pointer = read_le(image + 10, 2);
    s->data_selector = (uint16_t)read_le(image + 12, 2);
    s->opcode = 0;
  } else {
    cw = read_le(image, 2);
    s->status = (uint16_t)read_le(image + 4, 2);
    s->tags = (uint16_t)read_le(image + 8, 2);
    s->instruction_pointer = read_le(image + 12, 4);
    s->instruction_selector = (uint16_t)read_le(image + 16, 2);
    s->opcode = (uint16_t)(read_le(image + 18, 2) & 0x7ff);
    s->data_pointer = read_le(image + 20, 4);
    s->data_selector = (uint16_t)read_le(image + 24, 2);
  }
  if (registers) {
    unsigned offset = word16 ? 14 : 28, top = (s->status >> 11) & 7;
    for (unsigned i = 0; i < 8; ++i) {
      unsigned p = (top + i) & 7;
      s->significand[p] = 0;
      for (unsigned j = 0; j < 8; ++j)
        s->significand[p] |= (uint64_t)image[offset + 10 * i + j] << (8 * j);
      s->sign_exponent[p] = (uint16_t)read_le(image + offset + 10 * i + 8, 2);
    }
  }
  reclassify(s);
  control(s, cw);
}
void gamepkg_x86_x87_environment(uint32_t op, uint32_t word16, uint32_t address,
                                 uint32_t ip, uint32_t ebp) {
  /* Protected-mode images. Real/V86 formats and FXSAVE/SSE images need their
   * own CPU-mode/context integration; never dump the host C struct. */
  if (op > 4 || word16 > 1)
    abort();
  if (op <= 2)
    gamepkg_x86_x87_wait(ip, ebp);
  GamePkgX87State *s = gamepkg_x86_x87_state(), staged = *s;
  unsigned env = word16 ? 14 : 28, size = op == 0                ? 2
                                          : (op == 2 || op == 4) ? env + 80
                                                                 : env;
  unsigned char image[108];
  if (op <= 2) {
    gamepkg_x86_memory_bytes(address, image, size, 0, ip, ebp);
    if (op == 0)
      control(&staged, read_le(image, 2));
    else
      unpack(&staged, image, word16, op == 2);
  } else {
    reclassify(&staged);
    pack(&staged, image, word16);
    gamepkg_x86_memory_bytes(address, image, size, 1, ip, ebp);
    if (op == 3) {
      staged.control |= 63;
      staged.status &= (uint16_t)~0x8080u;
    } else {
      staged.control = 0x37f;
      staged.status = 0;
      staged.tags = 0xffff;
      staged.instruction_pointer = staged.data_pointer = 0;
      staged.opcode = staged.instruction_selector = staged.data_selector = 0;
    }
  }
  *s = staged;
}
