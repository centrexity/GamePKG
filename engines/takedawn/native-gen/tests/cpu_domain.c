#include "gamepkg_x86_cpu.h"
#include <assert.h>
#include <stdatomic.h>
#include <stdio.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#include <sched.h>
#endif
static GamePkgX86Cpu *cpu;
static GamePkgX86CpuThread *a, *b;
static atomic_uint phase;
static unsigned total;
static const GamePkgX86InterruptState enabled = {0x202, 1, 0, 0, 0, 0};
static void yield_host(void) {
#ifdef _WIN32
  SwitchToThread();
#else
  sched_yield();
#endif
}
static void worker_body(unsigned mode) {
  if (mode == 0) {
    assert(gamepkg_x86_cpu_step(a, 2, 0) == -1);
    assert(gamepkg_x86_cpu_suspend(a) == -1);
    assert(gamepkg_x86_cpu_enter(a) == -1);
    atomic_store_explicit(&phase, 1, memory_order_release);
    assert(!gamepkg_x86_cpu_enter(b));
    assert(!gamepkg_x86_cpu_step(b, 0x202, 0));
    assert(gamepkg_x86_cpu_block(b) == 1);
    assert(!gamepkg_x86_cpu_step(b, 2, 1));
    assert(gamepkg_x86_cpu_block(b) == 1);
    assert(!gamepkg_x86_cpu_retire_other(b, 0x202));
    atomic_store_explicit(&phase, 2, memory_order_release);
    assert(!gamepkg_x86_cpu_block(b));
  } else {
    GamePkgX86CpuThread *t = gamepkg_x86_cpu_thread_create(cpu, &enabled);
    assert(t);
    for (unsigned i = 0; i < 2000; ++i) {
      assert(!gamepkg_x86_cpu_enter(t));
      ++total; /* Ordinary host memory: virtual CPU ownership serializes it. */
      assert(!gamepkg_x86_cpu_block(t));
    }
    assert(!gamepkg_x86_cpu_thread_destroy(t));
  }
}
#ifdef _WIN32
typedef HANDLE Thread;
static DWORD WINAPI worker(LPVOID p) {
  worker_body((unsigned)(uintptr_t)p);
  return 0;
}
static Thread spawn(unsigned mode) {
  HANDLE t = CreateThread(NULL, 0, worker, (LPVOID)(uintptr_t)mode, 0, NULL);
  assert(t);
  return t;
}
static void join(Thread t) {
  assert(WaitForSingleObject(t, INFINITE) == WAIT_OBJECT_0);
  assert(CloseHandle(t));
}
#else
typedef pthread_t Thread;
static void *worker(void *p) {
  worker_body((unsigned)(uintptr_t)p);
  return NULL;
}
static Thread spawn(unsigned mode) {
  pthread_t t;
  assert(!pthread_create(&t, NULL, worker, (void *)(uintptr_t)mode));
  return t;
}
static void join(Thread t) { assert(!pthread_join(t, NULL)); }
#endif
int main(void) {
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
  uint32_t probe = 1;
  assert(sizeof(void *) == 4 && *(unsigned char *)&probe == 0);
#endif
  cpu = gamepkg_x86_cpu_create();
  assert(cpu);
  GamePkgX86InterruptState disabled = enabled;
  disabled.flags = 2;
  a = gamepkg_x86_cpu_thread_create(cpu, &disabled);
  b = gamepkg_x86_cpu_thread_create(cpu, &enabled);
  assert(a && b);
  assert(!gamepkg_x86_cpu_enter(a));
  assert(gamepkg_x86_cpu_enter(b) ==
         -1); /* Cannot queue behind this same worker. */
  assert(gamepkg_x86_cpu_thread_destroy(a) == -1);
  assert(gamepkg_x86_cpu_snapshot(a, (GamePkgX86InterruptState *)a) == -1);
  assert(!gamepkg_x86_cpu_suspend(a));
  assert(gamepkg_x86_cpu_thread_destroy(a) == -1);
  assert(!gamepkg_x86_cpu_enter(a));
  Thread blocked = spawn(0);
  while (atomic_load_explicit(&phase, memory_order_acquire) < 1 ||
         gamepkg_x86_cpu_waiters(cpu) != 1)
    yield_host();
  assert(gamepkg_x86_cpu_yield(a) == 1 && gamepkg_x86_cpu_block(a) == 1);
  assert(atomic_load_explicit(&phase, memory_order_acquire) == 1);
  assert(!gamepkg_x86_cpu_suspend(
      a)); /* IF0 reservation survives host inspection. */
  assert(gamepkg_x86_cpu_waiters(cpu) == 1);
  assert(!gamepkg_x86_cpu_enter(a));
  assert(!gamepkg_x86_cpu_step(a, 2, 1));
  assert(!gamepkg_x86_cpu_checkpoint(a, 0x246));
  GamePkgX86InterruptState checkpoint_image;
  assert(!gamepkg_x86_cpu_snapshot(a, &checkpoint_image) &&
         checkpoint_image.flags == 0x246 && checkpoint_image.sti_shadow == 1);
  assert(gamepkg_x86_cpu_yield(a) == 1 && gamepkg_x86_cpu_block(a) == 1);
  assert(atomic_load_explicit(&phase, memory_order_acquire) == 1);
  assert(!gamepkg_x86_cpu_retire_other(a, 0x202));
  assert(
      !gamepkg_x86_cpu_yield(a)); /* B runs and releases before A reacquires. */
  assert(atomic_load_explicit(&phase, memory_order_acquire) == 2);
  join(blocked);
  GamePkgX86InterruptState image;
  assert(!gamepkg_x86_cpu_snapshot(a, &image) &&
         !memcmp(&image, &enabled, sizeof(image)));
  GamePkgX86InterruptState ring3 = enabled;
  ring3.cpl = 3;
  assert(!gamepkg_x86_cpu_restore(a, &ring3));
  assert(gamepkg_x86_cpu_step(a, 0x202, 0) == 1);
  assert(!gamepkg_x86_cpu_snapshot(a, &image) &&
         !memcmp(&image, &ring3, sizeof(image)));
  assert(!gamepkg_x86_cpu_restore(a, &enabled));
  assert(!gamepkg_x86_cpu_block(a));
  Thread threads[8];
  for (unsigned i = 0; i < 8; ++i)
    threads[i] = spawn(i + 1);
  for (unsigned i = 0; i < 8; ++i)
    join(threads[i]);
  assert(total == 16000 && !gamepkg_x86_cpu_waiters(cpu));
  gamepkg_x86_cpu_release(cpu); /* Remaining context references keep it live. */
  assert(!gamepkg_x86_cpu_enter(a) && !gamepkg_x86_cpu_block(a));
  assert(!gamepkg_x86_cpu_thread_destroy(a));
  assert(!gamepkg_x86_cpu_thread_destroy(b));
  puts("CPU ownership: IF0/STI shadow exclusion, FIFO handoff, suspend/resume, "
       "owner rejection, GP/restore, lifetime and16000 eight-worker entries "
       "passed");
  return 0;
}
