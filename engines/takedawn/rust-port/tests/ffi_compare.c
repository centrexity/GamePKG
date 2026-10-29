#include <pthread.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

struct Opaque {
  uint32_t value;
};
uint32_t native_opaque_probe(const struct Opaque *pointer) {
  return pointer ? pointer->value : 0;
}
uint32_t port_add(uint32_t *, uint32_t);
uint32_t ref_port_add(uint32_t *, uint32_t);
uint32_t port_cas(uint32_t *, uint32_t, uint32_t);
uint32_t ref_port_cas(uint32_t *, uint32_t, uint32_t);
bool port_cas_bool(uint32_t *, uint32_t, uint32_t);
bool ref_port_cas_bool(uint32_t *, uint32_t, uint32_t);
uint32_t port_opaque(const struct Opaque *);
uint32_t ref_port_opaque(const struct Opaque *);

typedef uint32_t (*Add)(uint32_t *, uint32_t);
typedef uint32_t (*Cas)(uint32_t *, uint32_t, uint32_t);
struct Job {
  uint32_t *cell;
  Add add;
  Cas cas;
  uint64_t sum;
};
static void *worker(void *argument) {
  struct Job *job = argument;
  for (unsigned i = 0; i < 4096; ++i) {
    uint32_t old;
    if (job->add) {
      old = job->add(job->cell, 1);
    } else {
      uint32_t expected = 0;
      for (;;) {
        old = job->cas(job->cell, expected, expected + 1);
        if (old == expected)
          break;
        expected = old;
      }
    }
    job->sum += old;
  }
  return NULL;
}
static void concurrent(Add add, Cas cas) {
  uint32_t cell = 0;
  pthread_t threads[4];
  struct Job jobs[4];
  for (unsigned i = 0; i < 4; ++i) {
    jobs[i] = (struct Job){&cell, add, cas, 0};
    if (pthread_create(&threads[i], NULL, worker, &jobs[i]))
      abort();
  }
  uint64_t sum = 0;
  for (unsigned i = 0; i < 4; ++i) {
    if (pthread_join(threads[i], NULL))
      abort();
    sum += jobs[i].sum;
  }
  if (cell != 16384 || sum != UINT64_C(16384) * 16383 / 2)
    abort();
}
int main(void) {
  const uint32_t values[] = {0,          1,          7,         0x7fffffff,
                             0x80000000, 0xfffffffe, 0xffffffff};
  unsigned cases = 0;
  for (unsigned i = 0; i < sizeof values / sizeof *values; ++i) {
    for (unsigned j = 0; j < sizeof values / sizeof *values; ++j) {
      uint32_t a = values[i], b = values[i];
      if (ref_port_add(&a, values[j]) != port_add(&b, values[j]) || a != b)
        abort();
      ++cases;
      for (unsigned match = 0; match < 2; ++match) {
        a = b = values[i];
        uint32_t expected = match ? values[i] : values[i] ^ 1;
        if (ref_port_cas(&a, expected, values[j]) !=
                port_cas(&b, expected, values[j]) ||
            a != b)
          abort();
        ++cases;
        a = b = values[i];
        if (ref_port_cas_bool(&a, expected, values[j]) !=
                port_cas_bool(&b, expected, values[j]) ||
            a != b)
          abort();
        ++cases;
      }
    }
    struct Opaque opaque = {values[i]};
    if (ref_port_opaque(&opaque) != port_opaque(&opaque))
      abort();
    ++cases;
  }
  if (ref_port_opaque(NULL) != port_opaque(NULL))
    abort();
  ++cases;
  concurrent(ref_port_add, NULL);
  concurrent(port_add, NULL);
  concurrent(NULL, ref_port_cas);
  concurrent(NULL, port_cas);
  printf("PASS: %u C/Rust atomic/opaque cases and four concurrent "
         "16,384-update gates; old values, wraparound, CAS success/failure and "
         "pointer ABI match.\n",
         cases);
  return 0;
}
