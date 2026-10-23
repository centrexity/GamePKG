#include <stdbool.h>
#include <stdint.h>

uint32_t port_add(uint32_t *pointer, uint32_t value) {
  return __sync_fetch_and_add(pointer, value);
}
uint32_t port_cas(uint32_t *pointer, uint32_t expected, uint32_t value) {
  return __sync_val_compare_and_swap(pointer, expected, value);
}
bool port_cas_bool(uint32_t *pointer, uint32_t expected, uint32_t value) {
  return __sync_bool_compare_and_swap(pointer, expected, value);
}

struct Opaque;
extern uint32_t native_opaque_probe(const struct Opaque *pointer);
uint32_t port_opaque(const struct Opaque *pointer) {
  return native_opaque_probe(pointer) + 7u;
}
