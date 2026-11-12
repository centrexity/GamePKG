/* Replay independent ring0 SGDT/SIDT bytes against explicit virtual CPU data.
 * Player/runtime code never executes a native descriptor-table instruction. */
#include "gamepkg_x86_cpu.h"
#include "gamepkg_x86_table_store.h"
#include "gamepkg_x86_memory.h"
#include "gamepkg_x86_fault.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <pthread.h>
#endif
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error big-endian compiler required
#endif
_Static_assert(sizeof(void *) == 4, "actual big-endian32 required");
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
static unsigned char ram[16];
static GamePkgX86CpuThread *current;
static unsigned op, operand16;
static int store_result;
static void store(void) {
  store_result = gamepkg_x86_cpu_store_table(current, op, operand16, 0x1003,
                                             0x123456, 0x789abc);
}
static uint32_t word(FILE *f) {
  uint32_t v = 0;
  for (unsigned i = 0; i < 4; ++i) {
    int c = fgetc(f);
    assert(c != EOF);
    v |= (uint32_t)c << (i * 8);
  }
  return v;
}
static const GamePkgX86InterruptState initial = {0x202, 1, 0, 0, 0, 0};
static void bind_memory(unsigned size, unsigned permissions) {
  GamePkgX86Region region = {0x1000, size, permissions, ram};
  GamePkgX86Memory *memory = gamepkg_x86_memory_create(&region, 1, 0);
  assert(memory);
  gamepkg_x86_memory_bind(memory);
  gamepkg_x86_memory_release(memory);
}
static void reject_configuration(void) {
  GamePkgX86Tables good = {{0x89abcdef, 0x1234}, {0xfedcba98, 0x5678}}, out,
                   sentinel;
  memset(&sentinel, 0xa5, sizeof(sentinel));
  GamePkgX86Cpu *cpu = gamepkg_x86_cpu_create();
  assert(cpu);
  assert(gamepkg_x86_cpu_init_tables(NULL, &good) == -1);
  assert(gamepkg_x86_cpu_init_tables(cpu, NULL) == -1);
  assert(gamepkg_x86_cpu_init_tables(cpu, (void *)cpu) == -1);
  _Alignas(GamePkgX86Tables) unsigned char unaligned[sizeof(good) + 4];
  assert(gamepkg_x86_cpu_init_tables(cpu, (void *)(unaligned + 1)) == -1);
  for (unsigned which = 0; which < 2; ++which) {
    GamePkgX86Tables bad = good;
    if (which)
      bad.idtr.limit = 0x10000;
    else
      bad.gdtr.limit = UINT32_MAX;
    assert(gamepkg_x86_cpu_init_tables(cpu, &bad) == -1);
  }
  assert(gamepkg_x86_cpu_init_tables(cpu, &good) == 0);
  assert(gamepkg_x86_cpu_init_tables(cpu, &good) == -1);
  GamePkgX86CpuThread *thread = gamepkg_x86_cpu_thread_create(cpu, &initial);
  assert(thread);
  out = sentinel;
  assert(gamepkg_x86_cpu_snapshot_tables(thread, &out) == -1 &&
         !memcmp(&out, &sentinel, sizeof(out)));
  assert(gamepkg_x86_cpu_enter(thread) == 0);
  assert(gamepkg_x86_cpu_snapshot_tables(thread, NULL) == -1);
  assert(gamepkg_x86_cpu_snapshot_tables(thread, (void *)cpu) == -1);
  assert(gamepkg_x86_cpu_snapshot_tables(thread, (void *)thread) == -1);
  assert(gamepkg_x86_cpu_snapshot_tables(thread, (void *)(unaligned + 1)) ==
         -1);
  assert(gamepkg_x86_cpu_snapshot_tables(thread, &out) == 0 &&
         !memcmp(&out, &good, sizeof(out)));
  assert(gamepkg_x86_cpu_block(thread) == 0 &&
         gamepkg_x86_cpu_thread_destroy(thread) == 0);
  gamepkg_x86_cpu_release(cpu);
  /* Creating a thread permanently closes setup, even after its destruction. */
  cpu = gamepkg_x86_cpu_create();
  assert(cpu);
  thread = gamepkg_x86_cpu_thread_create(cpu, &initial);
  assert(thread);
  assert(gamepkg_x86_cpu_init_tables(cpu, &good) == -1);
  assert(gamepkg_x86_cpu_enter(thread) == 0);
  out = sentinel;
  assert(gamepkg_x86_cpu_snapshot_tables(thread, &out) == -1 &&
         !memcmp(&out, &sentinel, sizeof(out)));
  /* Unconfigured tables must not fall through to an invented zero register. */
  assert(gamepkg_x86_cpu_store_table(thread, 0, 0, 0x1003, 0, 0) == -1);
  assert(gamepkg_x86_cpu_block(thread) == 0 &&
         gamepkg_x86_cpu_thread_destroy(thread) == 0);
  assert(gamepkg_x86_cpu_init_tables(cpu, &good) == -1);
  gamepkg_x86_cpu_release(cpu);
  cpu = gamepkg_x86_cpu_create();
  assert(cpu);
  good = (GamePkgX86Tables){{0, 0}, {0, 0}};
  assert(gamepkg_x86_cpu_init_tables(cpu, &good) == 0);
  thread = gamepkg_x86_cpu_thread_create(cpu, &initial);
  assert(thread && gamepkg_x86_cpu_enter(thread) == 0);
  out = sentinel;
  assert(gamepkg_x86_cpu_snapshot_tables(thread, &out) == 0 &&
         !memcmp(&out, &good, sizeof(out)));
  assert(gamepkg_x86_cpu_block(thread) == 0 &&
         gamepkg_x86_cpu_thread_destroy(thread) == 0);
  gamepkg_x86_cpu_release(cpu);
}
typedef struct {
  GamePkgX86CpuThread *thread;
  GamePkgX86Tables expected;
} Worker;
#ifdef _WIN32
static DWORD WINAPI worker(void *raw) {
#else
static void *worker(void *raw) {
#endif
  Worker *w = raw;
  for (unsigned i = 0; i < 128; ++i) {
    GamePkgX86Tables out, sentinel;
    memset(&sentinel, 0x5a, sizeof(sentinel));
    out = sentinel;
    assert(gamepkg_x86_cpu_snapshot_tables(w->thread, &out) == -1 &&
           !memcmp(&out, &sentinel, sizeof(out)));
    assert(gamepkg_x86_cpu_enter(w->thread) == 0);
    assert(gamepkg_x86_cpu_snapshot_tables(w->thread, &out) == 0 &&
           !memcmp(&out, &w->expected, sizeof(out)));
    assert(gamepkg_x86_cpu_block(w->thread) == 0);
  }
  return 0;
}
static void shared_processor(void) {
  GamePkgX86Cpu *cpu = gamepkg_x86_cpu_create();
  assert(cpu);
  GamePkgX86Tables good = {{0x89abcdef, 0x1234}, {0xfedcba98, 0x5678}};
  assert(gamepkg_x86_cpu_init_tables(cpu, &good) == 0);
  Worker jobs[8];
#ifdef _WIN32
  HANDLE handles[8];
#else
  pthread_t handles[8];
#endif
  for (unsigned i = 0; i < 8; ++i) {
    jobs[i] = (Worker){gamepkg_x86_cpu_thread_create(cpu, &initial), good};
    assert(jobs[i].thread);
#ifdef _WIN32
    handles[i] = CreateThread(NULL, 0, worker, jobs + i, 0, NULL);
    assert(handles[i]);
#else
    assert(!pthread_create(handles + i, NULL, worker, jobs + i));
#endif
  }
  for (unsigned i = 0; i < 8; ++i) {
#ifdef _WIN32
    assert(WaitForSingleObject(handles[i], INFINITE) == WAIT_OBJECT_0);
    assert(CloseHandle(handles[i]));
#else
    assert(!pthread_join(handles[i], NULL));
#endif
    assert(gamepkg_x86_cpu_thread_destroy(jobs[i].thread) == 0);
  }
  gamepkg_x86_cpu_release(cpu);
}
static void faults_and_modes(void) {
  GamePkgX86Cpu *cpu = gamepkg_x86_cpu_create();
  assert(cpu);
  GamePkgX86Tables good = {{0x89abcdef, 0x1234}, {0xfedcba98, 0x5678}}, before,
                   after;
  assert(gamepkg_x86_cpu_init_tables(cpu, &good) == 0);
  current = gamepkg_x86_cpu_thread_create(cpu, &initial);
  assert(current && gamepkg_x86_cpu_enter(current) == 0);
  assert(gamepkg_x86_cpu_snapshot_tables(current, &before) == 0);
  for (unsigned mode = 0; mode < 3; ++mode) {
    memset(ram, 0xa5, sizeof(ram));
    unsigned char saved[16];
    memcpy(saved, ram, sizeof(ram));
    bind_memory(mode ? 16 : 8, mode == 1 ? 1 : 3);
    GamePkgX86InterruptState old, next;
    assert(gamepkg_x86_cpu_snapshot(current, &old) == 0);
    for (op = 0; op < 2; ++op)
      for (operand16 = 0; operand16 < 2; ++operand16) {
        GamePkgX86Fault fault;
        store_result = 123;
        assert(gamepkg_x86_run(store, &fault) == (mode == 2 ? 0 : 1));
        if (mode != 2) {
          assert(store_result == 123 && fault.vector == 14 &&
                 fault.eip == 0x123456 && fault.ebp == 0x789abc &&
                 fault.fault_address == (mode == 0 ? 0x1008u : 0x1003u));
          assert(!memcmp(saved, ram, sizeof(ram)));
        } else {
          assert(store_result == 0);
          for (unsigned i = 0; i < 16; ++i)
            if (i < 3 || i >= 9)
              assert(ram[i] == 0xa5);
        }
        assert(gamepkg_x86_cpu_snapshot_tables(current, &after) == 0 &&
               !memcmp(&before, &after, sizeof(before)));
        assert(gamepkg_x86_cpu_snapshot(current, &next) == 0 &&
               !memcmp(&old, &next, sizeof(old)));
      }
  }
  unsigned char saved[16];
  memcpy(saved, ram, sizeof(saved));
  assert(gamepkg_x86_cpu_store_table(current, 2, 0, 0, 0, 0) == -1);
  assert(gamepkg_x86_cpu_store_table(current, 0, 2, 0, 0, 0) == -1);
  for (unsigned vm = 0; vm < 2; ++vm) {
    GamePkgX86InterruptState state = initial;
    if (vm) {
      state.flags |= 0x20000;
      state.cpl = 3;
    } else
      state.protected_mode = 0;
    assert(gamepkg_x86_cpu_restore(current, &state) == 0);
    assert(gamepkg_x86_cpu_store_table(current, 0, 0, 0, 0, 0) == -1 &&
           !memcmp(saved, ram, sizeof(saved)));
  }
  assert(gamepkg_x86_cpu_restore(current, &initial) == 0);
  assert(gamepkg_x86_cpu_block(current) == 0);
  assert(gamepkg_x86_cpu_store_table(current, 0, 0, 0, 0, 0) == -1);
  assert(gamepkg_x86_cpu_thread_destroy(current) == 0);
  gamepkg_x86_cpu_release(cpu);
  gamepkg_x86_memory_bind(NULL);
}
int main(int argc, char **argv) {
  assert(argc == 2);
  g_eflags = 0x202;
  FILE *f = fopen(argv[1], "rb");
  assert(f);
  unsigned char magic[8];
  assert(fread(magic, 1, 8, f) == 8 && !memcmp(magic, "GPKTBL01", 8));
  uint32_t count = word(f);
  assert(count);
  bind_memory(sizeof(ram), 3);
  for (uint32_t i = 0; i < count; ++i) {
    op = word(f);
    operand16 = word(f);
    uint32_t base = word(f), limit = word(f);
    assert(op < 2 && operand16 < 2 && limit <= 65535);
    unsigned char expected[8];
    assert(fread(expected, 1, 8, f) == 8 && expected[6] == 0x5a &&
           expected[7] == 0x5a);
    GamePkgX86Tables image = {{base, limit}, {base, limit}}, snapshot;
    /* Deliberately different other register detects wrong GDTR/IDTR choice. */
    if (op) {
      image.gdtr.base ^= UINT32_MAX;
      image.gdtr.limit ^= 65535;
    } else {
      image.idtr.base ^= UINT32_MAX;
      image.idtr.limit ^= 65535;
    }
    GamePkgX86Cpu *cpu = gamepkg_x86_cpu_create();
    assert(cpu && gamepkg_x86_cpu_init_tables(cpu, &image) == 0);
    GamePkgX86InterruptState state = initial;
    state.cpl = i % 4;
    state.sti_shadow = i % 2;
    current = gamepkg_x86_cpu_thread_create(cpu, &state);
    assert(current && gamepkg_x86_cpu_enter(current) == 0);
    memset(ram, 0x5a, sizeof(ram));
    GamePkgX86Fault fault;
    assert(gamepkg_x86_run(store, &fault) == 0 && store_result == 0);
    assert(!memcmp(expected, ram + 3, 6));
    for (unsigned k = 0; k < 16; ++k)
      if (k < 3 || k >= 9)
        assert(ram[k] == 0x5a);
    assert(gamepkg_x86_cpu_snapshot_tables(current, &snapshot) == 0 &&
           !memcmp(&image, &snapshot, sizeof(image)));
    GamePkgX86InterruptState after;
    assert(gamepkg_x86_cpu_snapshot(current, &after) == 0 &&
           !memcmp(&state, &after, sizeof(state)));
    state.sti_shadow = 0;
    assert(gamepkg_x86_cpu_restore(current, &state) == 0 &&
           gamepkg_x86_cpu_block(current) == 0 &&
           gamepkg_x86_cpu_thread_destroy(current) == 0);
    gamepkg_x86_cpu_release(cpu);
  }
  assert(fgetc(f) == EOF && !ferror(f) && !fclose(f));
  gamepkg_x86_memory_bind(NULL);
  reject_configuration();
  shared_processor();
  faults_and_modes();
  printf(
      "Descriptor tables: %u independent records, frozen shared configuration, "
      "1024 worker leases and checked faults passed; pointer_bits=%u\n",
      count, (unsigned)(sizeof(void *) * 8));
  return 0;
}
