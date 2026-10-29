#include "gamepkg_x86_memory.h"
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_virtual_pages.h"
#include <limits.h>
#include <stdatomic.h>
#include <stdlib.h>
#ifdef GAMEPKG_VIRTUAL_ALLOCATOR_TEST
/* Maintainer-only failure injection; absent from ordinary runtime builds. */
extern void *gamepkg_virtual_test_calloc(size_t, size_t);
#define calloc gamepkg_virtual_test_calloc
#endif
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#endif
#if CHAR_BIT != 8
#error GamePKG memory services require eight-bit bytes
#endif
typedef struct VirtualMap VirtualMap;
static void virtual_destroy(VirtualMap *);
struct GamePkgX86Memory {
  atomic_uint references, lock;
#ifdef _WIN32
  SRWLOCK critical_mutex;
  CONDITION_VARIABLE critical_changed;
#else
  pthread_mutex_t critical_mutex;
  pthread_cond_t critical_changed;
#endif
  size_t count;
  uint32_t user;
  GamePkgX86Region regions[64];
  VirtualMap *virtual_map;
};
static _Thread_local GamePkgX86Memory *bound;
int gamepkg_x86_memory_is_bound(const GamePkgX86Memory *memory,
                                uint32_t user_fault) {
  return user_fault <= 1 && memory && bound == memory &&
         memory->user == user_fault;
}
static int width(uint32_t n) {
  return n == 1 || n == 2 || n == 4 || n == 8 || n == 10 || n == 16;
}
static int span(uint32_t n) { return n > 0 && n <= 512; }
GamePkgX86Memory *gamepkg_x86_memory_create(const GamePkgX86Region *r, size_t n,
                                            uint32_t user) {
  if (n > 64 || user > 1 || (n && !r))
    return 0;
  for (size_t i = 0; i < n; ++i) {
    uint64_t end = (uint64_t)r[i].base + r[i].length;
#if PTRDIFF_MAX < UINT32_MAX
    if (r[i].length > PTRDIFF_MAX)
      return 0;
#endif
    if (!r[i].length || !r[i].data ||
        (uintptr_t)r[i].data > UINTPTR_MAX - (r[i].length - 1u) ||
        !r[i].permissions || (r[i].permissions & ~3u) ||
        end > (UINT64_C(1) << 32))
      return 0;
    for (size_t j = 0; j < i; ++j)
      if ((uint64_t)r[i].base < (uint64_t)r[j].base + r[j].length &&
          (uint64_t)r[j].base < end)
        return 0;
  }
  GamePkgX86Memory *m = malloc(sizeof(*m));
  if (!m)
    return 0;
#ifdef _WIN32
  InitializeSRWLock(&m->critical_mutex);
  InitializeConditionVariable(&m->critical_changed);
#else
  if (pthread_mutex_init(&m->critical_mutex, NULL)) {
    free(m);
    return NULL;
  }
  if (pthread_cond_init(&m->critical_changed, NULL)) {
    pthread_mutex_destroy(&m->critical_mutex);
    free(m);
    return NULL;
  }
#endif
  atomic_init(&m->references, 1);
  atomic_init(&m->lock, 0);
  m->count = n;
  m->user = user;
  m->virtual_map = NULL;
  for (size_t i = 0; i < n; ++i) {
    m->regions[i] = r[i];
  }
  return m;
}
void gamepkg_x86_memory_release(GamePkgX86Memory *m) {
  if (!m)
    return;
  unsigned old =
      atomic_fetch_sub_explicit(&m->references, 1, memory_order_acq_rel);
  if (!old) {
    abort();
  }
  if (old == 1) {
    virtual_destroy(m->virtual_map);
#ifndef _WIN32
    if (pthread_cond_destroy(&m->critical_changed) ||
        pthread_mutex_destroy(&m->critical_mutex))
      abort();
#endif
    free(m);
  }
}
void gamepkg_x86_memory_bind(GamePkgX86Memory *m) {
  if (m == bound)
    return;
  if (m) {
    unsigned refs = atomic_load_explicit(&m->references, memory_order_relaxed);
    do {
      if (!refs || refs == UINT_MAX)
        abort();
    } while (!atomic_compare_exchange_weak_explicit(
        &m->references, &refs, refs + 1, memory_order_relaxed,
        memory_order_relaxed));
  }
  GamePkgX86Memory *old = bound;
  bound = m;
  gamepkg_x86_memory_release(old);
}
static GamePkgX86Memory *acquire(void) {
  GamePkgX86Memory *m = bound;
  if (!m)
    abort();
  unsigned zero;
  do {
    zero = 0;
  } while (!atomic_compare_exchange_weak_explicit(
      &m->lock, &zero, 1, memory_order_acquire, memory_order_relaxed));
  return m;
}
static void unlock(GamePkgX86Memory *m) {
  atomic_store_explicit(&m->lock, 0, memory_order_release);
}
#include "gamepkg_x86_virtual_pages.inc"
static const GamePkgX86Region *region(GamePkgX86Memory *m, uint32_t address,
                                      GamePkgX86Region *resolved) {
  for (size_t i = 0; i < m->count; ++i) {
    const GamePkgX86Region *r = &m->regions[i];
    if (address >= r->base && (uint64_t)address - r->base < r->length)
      return r;
  }
  return virtual_region(m, address, resolved);
}
static void admit_access(GamePkgX86Memory *m, uint32_t address,
                         unsigned char **pointers, uint32_t bytes,
                         uint32_t permission, uint32_t user, uint32_t ip,
                         uint32_t ebp) {
  for (uint32_t i = 0; i < bytes; ++i) {
    uint32_t at = address + i;
    GamePkgX86Region resolved;
    const GamePkgX86Region *r = region(m, at, &resolved);
    if (!r || (r->permissions & permission) != permission) {
      uint32_t error = (r ? 1u : 0u) |
                       ((permission & GAMEPKG_X86_WRITE) ? 2u : 0u) |
                       (user ? 4u : 0u);
      unlock(m);
      gamepkg_x86_page_fault(at, error, ip, ebp);
    }
    pointers[i] = r->data + (size_t)(at - r->base);
  }
}
static void admit(GamePkgX86Memory *m, uint32_t address,
                  unsigned char **pointers, uint32_t bytes, uint32_t permission,
                  uint32_t ip, uint32_t ebp) {
  admit_access(m, address, pointers, bytes, permission, m->user, ip, ebp);
}
void gamepkg_x86_memory_descriptor_read(uint32_t address, uint32_t words[2],
                                        uint32_t ip, uint32_t ebp) {
  if (!words)
    abort();
  GamePkgX86Memory *m = acquire();
  unsigned char *p[8];
  uint32_t staged[2] = {0};
  /* Processor descriptor accesses are implicit supervisor accesses even for
   * a transfer initiated at CPL3. Host RAM capabilities still apply. */
  admit_access(m, address, p, 8, GAMEPKG_X86_READ, 0, ip, ebp);
  for (uint32_t i = 0; i < 8; ++i)
    staged[i / 4] |= (uint32_t)*p[i] << (8 * (i % 4));
  unlock(m);
  words[0] = staged[0];
  words[1] = staged[1];
}
void gamepkg_x86_memory_descriptor_accessed(uint32_t address, uint32_t ip,
                                            uint32_t ebp) {
  GamePkgX86Memory *m = acquire();
  unsigned char *p[1];
  admit_access(m, address, p, 1, GAMEPKG_X86_READ, 0, ip, ebp);
  if (!(*p[0] & 1u)) {
    /* Check the conditional write before changing RAM. Keep the same shared
     * lock across read/admission/OR, preserving every other rights-byte bit.
     * An already set A bit needs no write permission in this P6 profile. */
    admit_access(m, address, p, 1, GAMEPKG_X86_WRITE, 0, ip, ebp);
    *p[0] |= 1u;
    atomic_thread_fence(memory_order_seq_cst);
  }
  unlock(m);
}
void gamepkg_x86_memory_read(uint32_t address, uint32_t words[4],
                             uint32_t bytes, uint32_t ip, uint32_t ebp) {
  if (!words || !width(bytes)) {
    abort();
  }
  GamePkgX86Memory *m = acquire();
  unsigned char *p[16];
  uint32_t staged[4] = {0};
  admit(m, address, p, bytes, GAMEPKG_X86_READ, ip, ebp);
  for (uint32_t i = 0; i < bytes; ++i) {
    staged[i / 4] |= (uint32_t)*p[i] << (8 * (i % 4));
  }
  unlock(m);
  for (unsigned i = 0; i < 4; ++i)
    words[i] = staged[i];
}
void gamepkg_x86_memory_check(uint32_t address, uint32_t bytes, uint32_t write,
                              uint32_t ip, uint32_t ebp) {
  if (!span(bytes) || write > 1)
    abort();
  GamePkgX86Memory *m = acquire();
  unsigned char *p[512];
  admit(m, address, p, bytes, write ? GAMEPKG_X86_WRITE : GAMEPKG_X86_READ, ip,
        ebp);
  unlock(m);
}
void gamepkg_x86_memory_bytes(uint32_t address, unsigned char *buffer,
                              uint32_t bytes, uint32_t write, uint32_t ip,
                              uint32_t ebp) {
  if (!buffer || !span(bytes) || write > 1)
    abort();
  unsigned char staged[512];
  if (write)
    for (uint32_t i = 0; i < bytes; ++i)
      staged[i] = buffer[i];
  GamePkgX86Memory *m = acquire();
  unsigned char *p[512];
  admit(m, address, p, bytes, write ? GAMEPKG_X86_WRITE : GAMEPKG_X86_READ, ip,
        ebp);
  for (uint32_t i = 0; i < bytes; ++i) {
    if (write)
      *p[i] = staged[i];
    else
      staged[i] = *p[i];
  }
  if (write)
    atomic_thread_fence(memory_order_seq_cst);
  unlock(m);
  if (!write)
    for (uint32_t i = 0; i < bytes; ++i)
      buffer[i] = staged[i];
}
/* Non-temporal is a cache hint, not permission to bypass transaction or store
 * ordering. Clang can retain the hint for legal host widths; other compilers
 * use ordinary RAM stores. No read/modify/write of unselected bytes occurs. */
#if defined(__has_builtin)
#if __has_builtin(__builtin_nontemporal_store)
#define GPK_NT_BYTE(p, v) __builtin_nontemporal_store((v), (p))
#endif
#endif
#ifndef GPK_NT_BYTE
#define GPK_NT_BYTE(p, v) (*(p) = (v))
#endif
void gamepkg_x86_memory_write(uint32_t address, const uint32_t words[4],
                              uint32_t bytes, uint32_t mask,
                              uint32_t non_temporal, uint32_t ip,
                              uint32_t ebp) {
  if (!words || !width(bytes) || non_temporal > 1 ||
      (mask & ~((1u << bytes) - 1))) {
    abort();
  }
  uint32_t staged[4];
  for (unsigned i = 0; i < 4; ++i)
    staged[i] = words[i];
  GamePkgX86Memory *m = acquire();
  unsigned char *p[16];
  admit(m, address, p, bytes, GAMEPKG_X86_WRITE, ip, ebp);
  for (uint32_t i = 0; i < bytes; ++i)
    if (mask & (1u << i)) {
      unsigned char value = (unsigned char)(staged[i / 4] >> (8 * (i % 4)));
      if (non_temporal) {
        GPK_NT_BYTE(p[i], value);
      } else
        *p[i] = value;
    }
  /* Complete possible host NT writes before publishing the transaction. */
  atomic_thread_fence(memory_order_seq_cst);
  unlock(m);
}
uint32_t gamepkg_x86_memory_rmw(uint32_t address, uint32_t bytes, uint32_t op,
                                uint32_t value, uint32_t expected, uint32_t ip,
                                uint32_t ebp) {
  if ((bytes != 1 && bytes != 2 && bytes != 4) || op > 2)
    abort();
  GamePkgX86Memory *m = acquire();
  unsigned char *p[16];
  /* A compare miss still performs a write cycle. Admit the complete write
   * span before reading, so neither success nor failure bypasses permissions.
   */
  admit(m, address, p, bytes, GAMEPKG_X86_READ | GAMEPKG_X86_WRITE, ip, ebp);
  uint32_t old = 0, mask = UINT32_MAX >> (32 - bytes * 8);
  for (uint32_t i = 0; i < bytes; ++i)
    old |= (uint32_t)*p[i] << (i * 8);
  uint32_t next = op == 0                    ? value
                  : op == 1                  ? old + value
                  : old == (expected & mask) ? value
                                             : old;
  for (uint32_t i = 0; i < bytes; ++i)
    *p[i] = (unsigned char)(next >> (i * 8));
  atomic_thread_fence(memory_order_seq_cst);
  unlock(m);
  return old;
}
void gamepkg_x86_store_fence(void) {
  atomic_thread_fence(memory_order_seq_cst);
}
void gamepkg_x86_prefetch(uint32_t address, uint32_t hint) {
  if (hint > 3) {
    abort();
  }
  GamePkgX86Memory *m = acquire();
  GamePkgX86Region resolved;
  const GamePkgX86Region *r = region(m, address, &resolved);
#if defined(__GNUC__) || defined(__clang__)
  if (r && (r->permissions & GAMEPKG_X86_READ)) {
    const unsigned char *p = r->data + (size_t)(address - r->base);
    switch (hint) {
    case 0:
      __builtin_prefetch(p, 0, 0);
      break;
    case 1:
      __builtin_prefetch(p, 0, 3);
      break;
    case 2:
      __builtin_prefetch(p, 0, 2);
      break;
    default:
      __builtin_prefetch(p, 0, 1);
      break;
    }
  }
#else
  (void)r;
#endif
  unlock(m);
}

/* Xbox guest ABI facts: nxdk lib/xboxkrnl/xboxdef.h RTL_CRITICAL_SECTION.
 * Host mutex/condition storage belongs to the RAM object, never guest bytes.
 * The condition is shared; woken workers recheck their own lock predicate.
 * No lock-address table, host pointer token, polling timeout or entry cap. */
typedef struct CriticalCall {
  struct CriticalCall *previous;
  uint32_t address, thread, op, ip, ebp;
  int registered, result, notify;
} CriticalCall;
static _Thread_local CriticalCall *critical_call;
static uint32_t critical_word(unsigned char **p) {
  return (uint32_t)*p[0] | ((uint32_t)*p[1] << 8) | ((uint32_t)*p[2] << 16) |
         ((uint32_t)*p[3] << 24);
}
static void critical_put(unsigned char **p, uint32_t value) {
  for (unsigned i = 0; i < 4; ++i)
    *p[i] = (unsigned char)(value >> (i * 8));
}
static void critical_transition(void) {
  CriticalCall *c = critical_call;
  GamePkgX86Memory *m = acquire();
  unsigned char *p[28], *event[4];
  c->result = -2;
  c->notify = 0;
  if (c->op == 0) {
    admit(m, c->address, p, 28, GAMEPKG_X86_WRITE, c->ip, c->ebp);
    const uint32_t image[7] = {
        0x00040001, 0, c->address + 8, c->address + 8, UINT32_MAX, 0, 0};
    for (unsigned i = 0; i < 7; ++i)
      critical_put(p + 4 * i, image[i]);
    c->result = 0;
    unlock(m);
    return;
  }
  admit(m, c->address + 16, p, 12, GAMEPKG_X86_READ | GAMEPKG_X86_WRITE, c->ip,
        c->ebp);
  uint32_t count = critical_word(p), depth = critical_word(p + 4),
           owner = critical_word(p + 8);
  int free_lock = count == UINT32_MAX && depth == 0 && owner == 0;
  int held = owner != 0 && depth > 0 && depth <= INT32_MAX &&
             count <= INT32_MAX && count >= depth - 1;
  int handoff = owner == 0 && depth == 0 && count <= INT32_MAX;
  if (!free_lock && !held && !handoff)
    goto done;
  if (c->op == 3) {
    if (!held || owner != c->thread)
      goto done;
    if (depth == 1 && count > 0) {
      admit(m, c->address + 4, event, 4, GAMEPKG_X86_READ | GAMEPKG_X86_WRITE,
            c->ip, c->ebp);
      critical_put(event, 1);
      c->notify = 1;
    }
    critical_put(p, count - 1);
    critical_put(p + 4, depth - 1);
    if (depth == 1)
      critical_put(p + 8, 0);
    c->result = 0;
    goto done;
  }
  if (c->registered) {
    if (free_lock || (held && owner == c->thread))
      goto done;
    if (held) {
      c->result = 2;
      goto done;
    }
    admit(m, c->address + 4, event, 4, GAMEPKG_X86_READ | GAMEPKG_X86_WRITE,
          c->ip, c->ebp);
    if (critical_word(event) != 1)
      goto done;
    critical_put(event, 0);
    critical_put(p + 4, 1);
    critical_put(p + 8, c->thread);
    c->result = 0;
    goto done;
  }
  if (free_lock) {
    critical_put(p, 0);
    critical_put(p + 4, 1);
    critical_put(p + 8, c->thread);
    c->result = 0;
  } else if (held && owner == c->thread) {
    if (count == INT32_MAX || depth == INT32_MAX)
      goto done;
    critical_put(p, count + 1);
    critical_put(p + 4, depth + 1);
    c->result = 0;
  } else if (c->op == 1) {
    c->result = 1;
  } else {
    if (count == INT32_MAX)
      goto done;
    admit(m, c->address + 4, event, 4, GAMEPKG_X86_READ | GAMEPKG_X86_WRITE,
          c->ip, c->ebp);
    if (critical_word(event) != (handoff ? 1u : 0u))
      goto done;
    critical_put(p, count + 1);
    c->registered = 1;
    /* An existing handoff token can be consumed by this registered waiter;
     * it must not sleep waiting for a future notification already delivered. */
    if (handoff) {
      critical_put(event, 0);
      critical_put(p + 4, 1);
      critical_put(p + 8, c->thread);
      c->result = 0;
    } else {
      c->result = 2;
    }
  }
done:
  unlock(m);
}
int gamepkg_x86_critical_section(uint32_t address, uint32_t thread, uint32_t op,
                                 uint32_t ip, uint32_t ebp) {
  if (op > 3 || (op && !thread) || address > UINT32_MAX - 27u)
    return -1;
  GamePkgX86Memory *m = bound;
  if (!m)
    abort();
#ifdef _WIN32
  AcquireSRWLockExclusive(&m->critical_mutex);
#else
  if (pthread_mutex_lock(&m->critical_mutex))
    abort();
#endif
  CriticalCall call = {.previous = critical_call,
                       .address = address,
                       .thread = thread,
                       .op = op,
                       .ip = ip,
                       .ebp = ebp};
  critical_call = &call;
  GamePkgX86Fault fault;
  int failed;
  for (;;) {
    failed = gamepkg_x86_run(critical_transition, &fault);
    if (failed || call.result != 2)
      break;
#ifdef _WIN32
    if (!SleepConditionVariableSRW(&m->critical_changed, &m->critical_mutex,
                                   INFINITE, 0))
      abort();
#else
    if (pthread_cond_wait(&m->critical_changed, &m->critical_mutex))
      abort();
#endif
  }
  critical_call = call.previous;
#ifdef _WIN32
  if (call.notify)
    WakeAllConditionVariable(&m->critical_changed);
  ReleaseSRWLockExclusive(&m->critical_mutex);
#else
  if (call.notify && pthread_cond_broadcast(&m->critical_changed))
    abort();
  if (pthread_mutex_unlock(&m->critical_mutex))
    abort();
#endif
  if (failed) {
    if (failed != 1 || fault.vector != 14)
      abort();
    gamepkg_x86_page_fault(fault.fault_address, fault.error_code, fault.eip,
                           fault.ebp);
  }
  return call.result;
}
