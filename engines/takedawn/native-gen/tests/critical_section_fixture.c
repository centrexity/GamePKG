/* Included in the real concrete-RAM fixture on every portability target. */
static uint32_t cs_get(uint32_t address) {
  uint32_t words[4];
  gamepkg_x86_memory_read(address, words, 4, 0x20000, 0x12345678);
  return words[0];
}
static void cs_put(uint32_t address, uint32_t value) {
  uint32_t words[4] = {value, 0, 0, 0};
  gamepkg_x86_memory_write(address, words, 4, 15, 0, 0x20000, 0x12345678);
}
static _Thread_local uint32_t cs_address, cs_op, cs_thread;
static _Thread_local int cs_result;
static void cs_access(void) {
  cs_result = gamepkg_x86_critical_section(cs_address, cs_thread, cs_op,
                                           0x20000, 0x12345678);
}
static int cs_call(uint32_t address, uint32_t thread, uint32_t op) {
  return gamepkg_x86_critical_section(address, thread, op, 0x20000, 0x12345678);
}
static void cs_basics(void) {
  unsigned char data[128], before[128];
  memset(data, 0xa5, sizeof(data));
  GamePkgX86Region region = {1000, sizeof(data), 3, data};
  GamePkgX86Memory *m = gamepkg_x86_memory_create(&region, 1, 0);
  assert(m);
  gamepkg_x86_memory_bind(m);
  assert(!cs_call(1000, 0, 0));
  const uint32_t expected[] = {0x00040001, 0, 1008, 1008, UINT32_MAX, 0, 0};
  for (unsigned i = 0; i < 7; ++i)
    assert(cs_get(1000 + i * 4) == expected[i]);
  assert(!cs_call(1000, 11, 2) && !cs_call(1000, 11, 1));
  assert(cs_get(1016) == 1 && cs_get(1020) == 2 && cs_get(1024) == 11);
  memcpy(before, data, sizeof(data));
  assert(cs_call(1000, 22, 1) == 1);
  assert(cs_call(1000, 22, 3) == -2);
  assert(cs_call(1000, 0, 2) == -1);
  assert(cs_call(1000, 11, 4) == -1);
  assert(cs_call(UINT32_MAX - 26, 11, 0) == -1);
  assert(!memcmp(data, before, sizeof(data)));
  assert(!cs_call(1000, 11, 3) && !cs_call(1000, 11, 3));
  assert(cs_get(1016) == UINT32_MAX && !cs_get(1020) && !cs_get(1024));
  assert(cs_call(1000, 11, 3) == -2);
  const uint32_t invalid[][3] = {
      {0, 1, 0},  {UINT32_MAX, 1, 11}, {0, 0, 11},
      {0, 2, 11}, {0x80000000, 1, 11}, {INT32_MAX, INT32_MAX, 11}};
  for (unsigned i = 0; i < sizeof(invalid) / sizeof(invalid[0]); ++i) {
    for (unsigned j = 0; j < 3; ++j)
      cs_put(1016 + j * 4, invalid[i][j]);
    memcpy(before, data, sizeof(data));
    assert(cs_call(1000, 11, 2) == -2);
    assert(!memcmp(data, before, sizeof(data)));
  }
  /* A second arena at the same guest VA has independent lock state. */
  unsigned char other[128] = {0};
  region.data = other;
  GamePkgX86Memory *m2 = gamepkg_x86_memory_create(&region, 1, 0);
  assert(m2);
  gamepkg_x86_memory_bind(m);
  assert(!cs_call(1000, 0, 0) && !cs_call(1000, 11, 2));
  gamepkg_x86_memory_bind(m2);
  assert(!cs_call(1000, 0, 0) && !cs_call(1000, 22, 2));
  assert(cs_get(1024) == 22 && !cs_call(1000, 22, 3));
  gamepkg_x86_memory_bind(m);
  assert(cs_get(1024) == 11 && !cs_call(1000, 11, 3));
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(m2);
  gamepkg_x86_memory_release(m);
}
static void cs_event_faults(void) {
  unsigned char data[128], before[128];
  for (unsigned releasing = 0; releasing < 2; ++releasing)
    for (unsigned cut = 4; cut < 8; ++cut)
      for (unsigned permission = 0; permission < 3; ++permission) {
        memset(data, 0, sizeof(data));
        GamePkgX86Region whole = {1000, sizeof(data), 3, data};
        GamePkgX86Memory *initial = gamepkg_x86_memory_create(&whole, 1, 1);
        assert(initial);
        gamepkg_x86_memory_bind(initial);
        assert(!cs_call(1000, 0, 0) && !cs_call(1000, 11, 2));
        if (releasing)
          cs_put(1016, 1); /* One registered waiter for the release path. */
        gamepkg_x86_memory_bind(NULL);
        gamepkg_x86_memory_release(initial);
        GamePkgX86Region regions[3] = {{1000, cut, 3, data}};
        unsigned count = 1;
        if (permission)
          regions[count++] =
              (GamePkgX86Region){1000 + cut, 1, permission, data + cut};
        regions[count++] =
            (GamePkgX86Region){1001 + cut, 127 - cut, 3, data + cut + 1};
        GamePkgX86Memory *m = gamepkg_x86_memory_create(regions, count, 1);
        assert(m);
        gamepkg_x86_memory_bind(m);
        memcpy(before, data, sizeof(data));
        cs_address = 1000;
        cs_op = releasing ? 3 : 2;
        cs_thread = releasing ? 11 : 22;
        GamePkgX86Fault f;
        assert(gamepkg_x86_run(cs_access, &f) == 1 && f.vector == 14 &&
               f.fault_address == 1000 + cut &&
               f.error_code == (6u | (permission ? 1u : 0u)));
        assert(!memcmp(data, before, sizeof(data)));
        assert(!cs_call(1064, 0, 0) && !cs_call(1064, 33, 2) &&
               !cs_call(1064, 33, 3));
        gamepkg_x86_memory_bind(NULL);
        gamepkg_x86_memory_release(m);
      }
}
static void cs_faults(void) {
  unsigned char data[128], before[128];
  GamePkgX86Fault f;
  for (unsigned op = 0; op <= 3; ++op) {
    unsigned begin = op == 0 ? 0 : 16, end = op == 0 ? 28 : 28;
    for (unsigned cut = begin; cut < end; ++cut)
      for (unsigned permission = 0; permission < 3; ++permission) {
        memset(data, 0xa5, sizeof(data));
        GamePkgX86Region whole = {1000, sizeof(data), 3, data};
        GamePkgX86Memory *initial = gamepkg_x86_memory_create(&whole, 1, 1);
        assert(initial);
        gamepkg_x86_memory_bind(initial);
        assert(!cs_call(1000, 0, 0));
        if (op == 3)
          assert(!cs_call(1000, 11, 2));
        gamepkg_x86_memory_bind(NULL);
        gamepkg_x86_memory_release(initial);
        GamePkgX86Region regions[3];
        unsigned count = 0;
        if (cut)
          regions[count++] = (GamePkgX86Region){1000, cut, 3, data};
        if (permission)
          regions[count++] =
              (GamePkgX86Region){1000 + cut, 1, permission, data + cut};
        regions[count++] =
            (GamePkgX86Region){1001 + cut, 127 - cut, 3, data + cut + 1};
        GamePkgX86Memory *m = gamepkg_x86_memory_create(regions, count, 1);
        assert(m);
        gamepkg_x86_memory_bind(m);
        memcpy(before, data, sizeof(data));
        cs_address = 1000;
        cs_op = op;
        cs_thread = 11;
        cs_result = 123;
        if (op == 0 && permission == 2) {
          assert(!gamepkg_x86_run(cs_access, &f) && cs_result == 0);
        } else {
          assert(gamepkg_x86_run(cs_access, &f) == 1 && f.vector == 14 &&
                 f.fault_address == 1000 + cut &&
                 f.error_code == (6u | (permission ? 1u : 0u)) &&
                 f.eip == 0x20000 && f.ebp == 0x12345678 && cs_result == 123);
          assert(!memcmp(before, data, sizeof(data)));
          /* A subsequent valid address must work: the fault released both
           * host synchronization locks, including the native condition lock. */
          assert(!cs_call(1064, 0, 0));
          assert(!cs_call(1064, 22, 2) && !cs_call(1064, 22, 3));
        }
        gamepkg_x86_memory_bind(NULL);
        gamepkg_x86_memory_release(m);
      }
  }
  /* Read-only event bytes do not prohibit uncontended acquisition/release. */
  memset(data, 0, sizeof(data));
  GamePkgX86Region regions[] = {{1000, 16, 1, data}, {1016, 112, 3, data + 16}};
  data[16] = data[17] = data[18] = data[19] = 0xff;
  GamePkgX86Memory *m = gamepkg_x86_memory_create(regions, 2, 0);
  assert(m);
  gamepkg_x86_memory_bind(m);
  assert(!cs_call(1000, 11, 2) && !cs_call(1000, 11, 3));
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(m);
}
static GamePkgX86Memory *cs_shared;
static atomic_uint cs_phase, cs_go;
static void cs_yield(void) {
#ifdef _WIN32
  SwitchToThread();
#else
  sched_yield();
#endif
}
static void cs_worker_body(unsigned id) {
  gamepkg_x86_memory_bind(cs_shared);
  if (!id) {
    atomic_store_explicit(&cs_phase, 1, memory_order_release);
    assert(!cs_call(1000, 22, 2));
    assert(cs_get(1020) == 1 && cs_get(1024) == 22);
    atomic_store_explicit(&cs_phase, 2, memory_order_release);
    while (!atomic_load_explicit(&cs_go, memory_order_acquire))
      cs_yield();
    assert(!cs_call(1000, 22, 3));
  } else {
    for (unsigned i = 0; i < 2000; ++i) {
      assert(!cs_call(1000, id, 2));
      assert(!cs_call(1000, id, 1));
      uint32_t value = cs_get(1120);
      cs_put(1120, value + 1);
      assert(!cs_call(1000, id, 3) && !cs_call(1000, id, 3));
    }
  }
  gamepkg_x86_memory_bind(NULL);
}
#ifdef _WIN32
static DWORD WINAPI cs_worker(LPVOID arg) {
  cs_worker_body((unsigned)(uintptr_t)arg);
  return 0;
}
typedef HANDLE CsWorker;
static CsWorker cs_spawn(unsigned id) {
  HANDLE t = CreateThread(NULL, 0, cs_worker, (LPVOID)(uintptr_t)id, 0, NULL);
  assert(t);
  return t;
}
static void cs_join(CsWorker t) {
  assert(WaitForSingleObject(t, INFINITE) == WAIT_OBJECT_0);
  assert(CloseHandle(t));
}
#else
static void *cs_worker(void *arg) {
  cs_worker_body((unsigned)(uintptr_t)arg);
  return NULL;
}
typedef pthread_t CsWorker;
static CsWorker cs_spawn(unsigned id) {
  pthread_t t;
  assert(!pthread_create(&t, NULL, cs_worker, (void *)(uintptr_t)id));
  return t;
}
static void cs_join(CsWorker t) { assert(!pthread_join(t, NULL)); }
#endif
static void cs_contention(void) {
  unsigned char data[128] = {0};
  GamePkgX86Region region = {1000, sizeof(data), 3, data};
  cs_shared = gamepkg_x86_memory_create(&region, 1, 0);
  assert(cs_shared);
  gamepkg_x86_memory_bind(cs_shared);
  assert(!cs_call(1000, 0, 0) && !cs_call(1064, 0, 0));
  assert(!cs_call(1000, 11, 2));
  atomic_init(&cs_phase, 0);
  atomic_init(&cs_go, 0);
  CsWorker blocked = cs_spawn(0);
  while (atomic_load_explicit(&cs_phase, memory_order_acquire) < 1 ||
         cs_get(1016) != 1)
    cs_yield();
  assert(cs_get(1020) == 1 && cs_get(1024) == 11);
  assert(cs_call(1000, 33, 1) == 1);
  assert(!cs_call(1064, 11, 2) && !cs_call(1064, 11, 3));
  assert(!cs_call(1000, 11, 3));
  while (atomic_load_explicit(&cs_phase, memory_order_acquire) != 2)
    cs_yield();
  assert(cs_call(1000, 11, 1) == 1);
  atomic_store_explicit(&cs_go, 1, memory_order_release);
  cs_join(blocked);
  CsWorker workers[8];
  for (unsigned i = 0; i < 8; ++i)
    workers[i] = cs_spawn(i + 1);
  for (unsigned i = 0; i < 8; ++i)
    cs_join(workers[i]);
  assert(cs_get(1120) == 16000 && cs_get(1016) == UINT32_MAX &&
         cs_get(1020) == 0 && cs_get(1024) == 0 && cs_get(1004) == 0);
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(cs_shared);
}
static void critical_sections(void) {
  cs_basics();
  cs_faults();
  cs_event_faults();
  cs_contention();
  puts(
      "Xbox critical sections: LE28-byte ABI, recursive/try/owner transitions, "
      "fault rollback/host-lock cleanup, independent arenas/locks, blocking "
      "handoff and16000 contended recursive updates passed");
}
