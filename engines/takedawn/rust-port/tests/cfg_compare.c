#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
__thread uint32_t shared;
uint32_t *gamepkg_tls_shared(void) { return &shared; }
static uint32_t calls;
void native_touch(void) {
  shared = shared * 5u + 0x87654321u;
  calls++;
}
uint32_t cfg_case(uint32_t, uint32_t, uint32_t *);
uint32_t c_cfg_case(uint32_t, uint32_t, uint32_t *);
void cfg_void(uint32_t *);
void c_cfg_void(uint32_t *);
int main(void) {
  unsigned count = 0;
  for (uint32_t seed = 0; seed < 64; seed++)
    for (uint32_t steps = 0; steps < 256; steps++) {
      uint32_t a[3], b[3];
      calls = 0;
      uint32_t ar = c_cfg_case(seed * 65537u, steps, a), ac = calls,
               as = shared;
      calls = 0;
      uint32_t br = cfg_case(seed * 65537u, steps, b), bc = calls, bs = shared;
      if (ar != br || ac != bc || as != bs || memcmp(a, b, sizeof a)) {
        fprintf(stderr, "CFG mismatch seed=%u steps=%u\n", seed, steps);
        return 1;
      }
      count++;
    }
  for (uint32_t v = 0; v < 256; v++) {
    uint32_t a = v, b = v;
    c_cfg_void(&a);
    cfg_void(&b);
    if (a != b || a != v + 6)
      return 2;
    count++;
  }
  printf("PASS: %u native C/Rust CFG cases; 300 labels, repeated local names, "
         "initializer effects, native callback/TLS changes, implicit void "
         "returns.\n",
         count);
  return 0;
}
