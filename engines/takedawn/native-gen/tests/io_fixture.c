/* Asset-free protocol fixture. Synthetic backend data tests serialization only;
 * actual filesystem confinement is covered independently by io_backend.rs. */
#include "gamepkg_x86_fault.h"
#include "gamepkg_x86_io.h"
#include "gamepkg_x86_irql.h"
#include "gamepkg_x86_memory.h"
#include "gamepkg_x86_rtl_string.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef GAMEPKG_REQUIRE_BIG_ENDIAN
#if __BYTE_ORDER__ != __ORDER_BIG_ENDIAN__
#error actual big endian required
#endif
_Static_assert(sizeof(void *) == 4, "actual32bit required");
#endif
_Thread_local uint32_t g_eax, g_ecx, g_edx, g_ebx, g_esp, g_ebp, g_esi, g_edi,
    g_eflags, g_fs_base;
_Thread_local int g_df;
static unsigned allocation_failure;
void *gamepkg_io_test_malloc(size_t n) {
  if (allocation_failure && !--allocation_failure)
    return NULL;
  return malloc(n);
}
static unsigned char bank[8192], long_bank[100001];
static uint32_t ansi_destination = 0x2400, ansi_source = 0x2500;
static uint32_t ansi_priority;
static unsigned equal_checks;
static uint32_t equal_mode;
#if defined(_WIN32) && defined(GAMEPKG_LEGACY_ANSI_WINE_ORACLE)
typedef struct {
  uint16_t length, maximum;
  const char *buffer;
} NativeAnsi;
__declspec(dllimport) void __stdcall RtlInitAnsiString(NativeAnsi *,
                                                       const char *);
__declspec(dllimport) unsigned char __stdcall RtlEqualString(const NativeAnsi *,
                                                             const NativeAnsi *,
                                                             unsigned char);
_Static_assert(sizeof(NativeAnsi) == 8, "native32 ANSI_STRING required");
#endif
static GamePkgX87State fp;
static uint32_t xmm[8][4];
static const GamePkgX86FpSource source = {
    &fp, {xmm[0], xmm[1], xmm[2], xmm[3], xmm[4], xmm[5], xmm[6], xmm[7]}};
static GamePkgX86Memory *memory;
static GamePkgX86Cpu *cpu;
static GamePkgX86CpuThread *thread;
static GamePkgX86Handles *handles;
static GamePkgX86Io *service;
static GamePkgX86IoResult result;
static unsigned checks, action, opens, volumes, closes, live, fail_close;
static uint32_t link_pointer = 0x2400;
static unsigned creates;
static uint32_t allocation_pointer, create_disposition = 3,
                                    create_information = 2;
static uint32_t desired_access = 0x100001, file_attributes = 0x80,
                backend_kind = 1;
static uint32_t expected_allocation[3];
static uint32_t handle, options = 0x800021, share = 3;
static uint32_t hp = 0x2000, io = 0x2010, output = 0x2200;
static uint32_t length = 24, info_class = 3, expected_parent;
static unsigned selected;
static const char *expected_name = "";
static uint32_t open_status;
static unsigned queries;
static uint32_t query_status, query_bad;
static int query_host;
static unsigned stages, unstages, transfers, active_buffers, stage_failure;
static uint32_t transfer_write, transfer_event, transfer_apc,
    transfer_apc_context, offset_pointer, expected_offset[2], transfer_bytes,
    transfer_status, transfer_complete, transfer_bad;
static int transfer_host;
static unsigned copy_fault_countdown;
void gamepkg_io_test_copy_fault(void) {
  if (copy_fault_countdown && !--copy_fault_countdown)
    gamepkg_x86_page_fault(0x2400, transfer_write ? 1 : 3, 0x1234, 0x5678);
}
static const uint32_t query_fields[13] = {
    0x01abcdef, 0x12345678, 0x10203040, 0x7fffffff, 0xdeadbeef, 1,   0xabcdef01,
    0x12345678, 0x89abcdef, 0x12345678, 0x76543210, 0x01234567, 0x24};
static void put(unsigned char *p, uint32_t v) {
  for (unsigned i = 0; i < 4; ++i)
    p[i] = (unsigned char)(v >> (8 * i));
}
static uint32_t get(const unsigned char *p) {
  return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) |
         ((uint32_t)p[3] << 24);
}
static int backend_open(void *context, uint32_t lo, uint32_t hi,
                        const unsigned char *name, uint32_t n, uint32_t access,
                        uint32_t sharing, uint32_t flags, uint32_t insensitive,
                        uint32_t out[4]) {
  assert(*(unsigned *)context == selected && access == desired_access &&
         sharing == share && flags == options && insensitive == 1);
  assert(lo == expected_parent && hi == (lo ? 0x12345678 : 0));
  assert(n == strlen(expected_name) && !memcmp(name, expected_name, n));
  /* Backend callbacks cannot reenter either service or shared table. */
  assert(gamepkg_x86_handles_prepare(handles, thread, 1, 2).status == -1);
  ++opens;
  out[0] = open_status;
  if (!open_status) {
    out[1] = 0xabcdef00u + opens;
    out[2] = 0x12345678;
    out[3] = backend_kind;
    ++live;
  }
  return 0;
}
static int backend_create(void *context, uint32_t lo, uint32_t hi,
                          const unsigned char *name, uint32_t n,
                          const uint32_t request[9], uint32_t out[5]) {
  assert(request[0] == desired_access && request[1] == share &&
         request[2] == options && request[3] == 1 &&
         request[4] == create_disposition && request[5] == file_attributes &&
         request[6] == expected_allocation[0] &&
         request[7] == expected_allocation[1] &&
         request[8] == expected_allocation[2]);
  ++creates;
  int rc = backend_open(context, lo, hi, name, n, request[0], request[1],
                        request[2], request[3], out);
  if (!rc && !out[0])
    out[4] = create_information;
  return rc;
}
static int backend_volume(void *context, uint32_t lo, uint32_t hi,
                          uint32_t out[7]) {
  assert(*(unsigned *)context == selected && lo && hi == 0x12345678);
  ++volumes;
  uint32_t words[7] = {0,          0x89abcdef, 0x12345678, 0x76543210,
                       0x01234567, 512,        32};
  memcpy(out, words, sizeof(words));
  return 0;
}
static int backend_query(void *context, uint32_t lo, uint32_t hi, uint32_t kind,
                         uint32_t out[14]) {
  assert(*(unsigned *)context == selected && lo && hi == 0x12345678 &&
         kind == 34);
  ++queries;
  if (query_host)
    return query_host;
  out[0] = query_status;
  if (!query_status) {
    memcpy(out + 1, query_fields, sizeof(query_fields));
    if (query_bad)
      out[10] = 0x80000000;
  }
  return 0;
}
static int backend_stage(void *context, uint32_t n, GamePkgX86IoBuffer *b,
                         uint32_t *status) {
  assert(*(unsigned *)context == selected);
  ++stages;
  if (stage_failure) {
    *status = 0xc0000017u;
    return 0;
  }
  unsigned char *data = calloc(n ? n : 1, 1);
  assert(data);
  *b = (GamePkgX86IoBuffer){data, data, n};
  *status = 0;
  ++active_buffers;
  return 0;
}
static int backend_unstage(void *context, GamePkgX86IoBuffer *b) {
  assert(*(unsigned *)context == selected && b->owner && b->data == b->owner &&
         active_buffers == 1);
  free(b->owner);
  *b = (GamePkgX86IoBuffer){0};
  --active_buffers;
  ++unstages;
  return 0;
}
static int backend_transfer(void *context, uint32_t lo, uint32_t hi,
                            GamePkgX86IoBuffer *b, const uint32_t request[4],
                            uint32_t out[3]) {
  assert(*(unsigned *)context == selected && lo && hi == 0x12345678 &&
         active_buffers == 1);
  assert(request[0] == transfer_write && request[1] == expected_offset[0] &&
         request[2] == expected_offset[1] &&
         request[3] == (offset_pointer != 0));
  ++transfers;
  if (transfer_host)
    return transfer_host;
  uint32_t n = transfer_bytes > b->length ? b->length : transfer_bytes;
  if (!transfer_complete)
    n = 0;
  for (uint32_t i = 0; i < n; ++i) {
    unsigned char value = (unsigned char)(i * 17 + 3);
    if (transfer_write)
      assert(b->data[i] == value);
    else {
      assert(!b->data[i]);
      b->data[i] = value;
    }
  }
  out[0] = transfer_status;
  out[1] = transfer_bad ? b->length + 1 : n;
  out[2] = transfer_complete;
  return 0;
}
static int backend_close(void *context, uint32_t lo, uint32_t hi,
                         uint32_t terminal, uint32_t *status) {
  (void)context;
  assert(lo && hi == 0x12345678 && terminal <= 1 && live);
  assert(gamepkg_x86_handles_owned(handles, service) == SIZE_MAX);
  ++closes;
  *status = fail_close ? 0xc0000022u : 0;
  if (!fail_close)
    --live;
  return 0;
}
static void equal_matrix(void) {
  put(bank + 0x1400, 1);
  put(bank + 0x1404, 0x2500);
  put(bank + 0x1420, 1);
  put(bank + 0x1424, 0x2600);
  for (unsigned mode = 0; mode < 2; ++mode)
    for (unsigned a = 0; a < 256; ++a)
      for (unsigned b = 0; b < 256; ++b) {
        bank[0x1500] = (unsigned char)a;
        bank[0x1600] = (unsigned char)b;
        unsigned expected = a == b || (mode && ((a ^ b) == 32) &&
                                       ((a | 32) >= 'a') && ((a | 32) <= 'z'));
        GamePkgX86RtlEqualResult r = gamepkg_x86_rtl_equal_ansi(
            cpu, thread, memory, 0x2400, 0x2420, mode, 0x1234, 0x5678);
        assert(!r.status && r.equal == expected);
#if defined(_WIN32) && defined(GAMEPKG_LEGACY_ANSI_WINE_ORACLE)
        NativeAnsi x = {1, 0, (const char *)bank + 0x1500},
                   y = {1, 0, (const char *)bank + 0x1600};
        assert((RtlEqualString(&x, &y, (unsigned char)mode) != 0) ==
               (r.equal != 0));
#endif
        ++equal_checks;
      }
}
static void body(void) {
  if (!action) {
    assert(
        !gamepkg_x86_irql_initialize(thread, memory, 0x1000, 0x1234, 0x5678));
  } else if (action == 1) {
    result = gamepkg_x86_io_open(service, thread, hp, 0x100001, 0x2100, io,
                                 share, options, 0x1234, 0x5678);
  } else if (action == 2) {
    result = gamepkg_x86_io_volume(service, thread, handle, io, output, length,
                                   info_class, 0x1234, 0x5678);
  } else if (action == 13) {
    result = gamepkg_x86_io_query(service, thread, handle, io, output, length,
                                  info_class, 0x1234, 0x5678);
  } else if (action == 14) {
    result = gamepkg_x86_io_transfer(service, thread, transfer_write, handle,
                                     transfer_event, transfer_apc,
                                     transfer_apc_context, io, output, length,
                                     offset_pointer, 0x1234, 0x5678);
  } else if (action == 3) {
    GamePkgX86HandleResult r =
        gamepkg_x86_handles_close(handles, thread, handle, 0x1234, 0x5678);
    result = (GamePkgX86IoResult){r.status, r.ntstatus, 0, 0};
  } else if (action == 4) {
    GamePkgX86HandleReservation r =
        gamepkg_x86_handles_prepare(handles, thread, 1, 2);
    assert(!r.status && r.ticket);
    assert(gamepkg_x86_handles_prepare(handles, thread, 1, 2).status == -1);
    assert(gamepkg_x86_handles_lookup(handles, thread, handle, 1, 2).status ==
           -1);
    assert(!gamepkg_x86_handles_cancel(handles, thread, r.ticket));
  } else if (action == 11) {
    equal_matrix();
  } else if (action == 12) {
    GamePkgX86RtlEqualResult r = gamepkg_x86_rtl_equal_ansi(
        cpu, thread, memory, 0x2400, 0x2420, equal_mode, 0x1234, 0x5678);
    result = (GamePkgX86IoResult){r.status, 0, 0, r.equal};
  } else if (action == 10) {
    assert(!gamepkg_x86_irql(thread, 1, ansi_priority, 0x1234, 0x5678).status);
    result = (GamePkgX86IoResult){
        gamepkg_x86_rtl_init_ansi(cpu, thread, memory, ansi_destination,
                                  ansi_source, 0x1234, 0x5678),
        0, 0, 0};
    GamePkgX86CpuIrql level;
    assert(!gamepkg_x86_cpu_snapshot_irql(thread, &level) &&
           level.level == ansi_priority && bank[0] == ansi_priority);
    assert(!gamepkg_x86_irql(thread, 2, 0, 0x1234, 0x5678).status);
  } else if (action == 9) {
    result = (GamePkgX86IoResult){
        gamepkg_x86_rtl_init_ansi(cpu, thread, memory, ansi_destination,
                                  ansi_source, 0x1234, 0x5678),
        0, 0, 0};
  } else if (action == 8) {
    result = gamepkg_x86_io_create_file(
        service, thread, hp, desired_access, 0x2100, io, allocation_pointer,
        file_attributes, share, create_disposition, options, 0x1234, 0x5678);
  } else if (action == 6) {
    result = gamepkg_x86_io_create_symbolic_link(service, thread, link_pointer,
                                                 0x2420, 0x1234, 0x5678);
  } else if (action == 7) {
    result = gamepkg_x86_io_delete_symbolic_link(service, thread, link_pointer,
                                                 0x1234, 0x5678);
  } else if (action == 5) {
    assert(!gamepkg_x86_irql(thread, 1, 1, 1, 2).status);
    assert(gamepkg_x86_handles_prepare(handles, thread, 1, 2).status == -1);
    assert(!gamepkg_x86_irql(thread, 2, 0, 1, 2).status);
  }
}
static void call(unsigned vector, unsigned error, uint32_t address) {
  GamePkgX86FpFault before, after, fault;
  assert(!gamepkg_x86_capture_registers(0x1234, g_ebp, &before, &source));
  int rc = gamepkg_x86_run_with_fp(body, &fault, &source);
  assert(rc == (vector ? 1 : 0));
  if (vector)
    assert(fault.core.vector == vector && fault.core.error_code == error &&
           fault.core.fault_address == address &&
           !memcmp(&before.fp, &fault.fp, sizeof(before.fp)));
  assert(!gamepkg_x86_capture_registers(0x1234, g_ebp, &after, &source));
  if (memcmp(&before, &after, sizeof(before))) {
    const unsigned char *a = (const unsigned char *)&before;
    const unsigned char *b = (const unsigned char *)&after;
    for (size_t i = 0; i < sizeof(before); ++i)
      if (a[i] != b[i])
        fprintf(stderr, "action=%u vector=%u byte=%zu before=%u after=%u\n",
                action, vector, i, a[i], b[i]);
    assert(0);
  }
  ++checks;
}
static void name(const char *text, uint32_t parent) {
  size_t n = strlen(text);
  assert(n < 1000);
  put(bank + 0x1100, parent);
  put(bank + 0x1104, 0x2120);
  put(bank + 0x1108, 0x40);
  put(bank + 0x1120, (uint32_t)n | ((uint32_t)(n + 1) << 16));
  put(bank + 0x1124, 0x2300);
  memcpy(bank + 0x1300, text, n + 1);
}
static void setup_access(unsigned denied, unsigned permission) {
  memset(bank, 0, sizeof(bank));
  GamePkgX86Region r[4] = {
      {0x1000, denied, 3, bank},
      {0x1000 + denied, 1, permission, bank + denied},
      {0x1001 + denied, 8191 - denied, 3, bank + denied + 1}};
  if (denied == 8192)
    r[0] = (GamePkgX86Region){0x1000, 8192, 3, bank};
  if (denied == 8192)
    r[1] = (GamePkgX86Region){0x100000, sizeof(long_bank), 3, long_bank};
  else
    r[3] = (GamePkgX86Region){0x100000, sizeof(long_bank), 3, long_bank};
  memory = gamepkg_x86_memory_create(r, denied == 8192 ? 2 : 4, 0);
  assert(memory);
  gamepkg_x86_memory_bind(memory);
  cpu = gamepkg_x86_cpu_create();
  GamePkgX86Tables tables = {{0, 0}, {0, 0}};
  assert(cpu && !gamepkg_x86_cpu_require_irql(cpu) &&
         !gamepkg_x86_cpu_init_tables(cpu, &tables));
  GamePkgX86InterruptState initial = {0x202, 1, 0, 0, 0, 0};
  thread = gamepkg_x86_cpu_thread_create(cpu, &initial);
  assert(thread && !gamepkg_x86_cpu_enter(thread));
  GamePkgX86CpuCodeState code = {8, 0, UINT32_MAX, 0xc9b, 0, 0, 0, 0, 0};
  assert(!gamepkg_x86_cpu_init_code(thread, &code));
  memset(&fp, 0, sizeof(fp));
  fp.control = 0x37e;
  fp.status = 0x8081;
  memset(fp.significand, 0xa5, sizeof(fp.significand));
  memset(xmm, 0x5a, sizeof(xmm));
  g_eflags = 0x202;
  g_ebp = 0x5678;
  action = 0;
  call(0, 0, 0);
  handles = gamepkg_x86_handles_create(cpu, memory);
  assert(handles);
  static unsigned ids[2] = {0, 1};
  static const unsigned char p0[] = "\\Device\\Harddisk0\\Partition1";
  static const unsigned char p1[] = "\\Device\\Harddisk0\\Partition1\\Cache";
  GamePkgX86IoMount mounts[2] = {
      {p0,
       sizeof(p0) - 1,
       {ids, backend_open, backend_volume, backend_close, backend_create,
        backend_query, backend_stage, backend_unstage, backend_transfer}},
      {p1,
       sizeof(p1) - 1,
       {ids + 1, backend_open, backend_volume, backend_close, NULL, NULL, NULL,
        NULL, NULL}}};
  service = gamepkg_x86_io_create(cpu, memory, handles, mounts, 2);
  assert(service);
  opens = volumes = closes = live = fail_close = 0;
  hp = 0x2000;
  io = 0x2010;
  output = 0x2200;
  options = 0x800021;
  share = 3;
  length = 24;
  info_class = 3;
  expected_parent = 0;
  selected = 0;
  expected_name = "";
  open_status = 0;
  queries = query_status = query_bad = 0;
  query_host = 0;
  stages = unstages = transfers = active_buffers = stage_failure = 0;
  transfer_write = transfer_event = transfer_apc = transfer_apc_context =
      offset_pointer = 0;
  expected_offset[0] = expected_offset[1] = 0;
  transfer_bytes = UINT32_MAX;
  transfer_status = transfer_bad = 0;
  transfer_complete = 1;
  transfer_host = 0;
  copy_fault_countdown = 0;
  link_pointer = 0x2400;
  ansi_destination = 0x2400;
  ansi_source = 0x2500;
  creates = 0;
  allocation_pointer = 0;
  create_disposition = 3;
  create_information = 2;
  desired_access = 0x100001;
  file_attributes = 0x80;
  backend_kind = 1;
  memset(expected_allocation, 0, sizeof(expected_allocation));
}
static void setup(unsigned denied) { setup_access(denied, 1); }
static void symbolic_names(const char *a, const char *b) {
  size_t na = strlen(a), nb = strlen(b);
  assert(na < 256 && nb < 256);
  put(bank + 0x1400, (uint32_t)na | ((uint32_t)(na + 1) << 16));
  put(bank + 0x1404, 0x2500);
  put(bank + 0x1420, (uint32_t)nb | ((uint32_t)(nb + 1) << 16));
  put(bank + 0x1424, 0x2600);
  memcpy(bank + 0x1500, a, na + 1);
  memcpy(bank + 0x1600, b, nb + 1);
}
static void create_link(const char *a, const char *b) {
  symbolic_names(a, b);
  action = 6;
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus);
}
static void teardown(void) {
  assert(!active_buffers);
  assert(!live && !gamepkg_x86_io_release(service));
  assert(!gamepkg_x86_handles_release(handles, 0));
  assert(!gamepkg_x86_cpu_block(thread) &&
         !gamepkg_x86_cpu_thread_destroy(thread));
  gamepkg_x86_cpu_release(cpu);
  gamepkg_x86_memory_bind(NULL);
  gamepkg_x86_memory_release(memory);
}

static void symbolic_link_suite(void) {
  setup(8192);
  create_link("\\??\\D:", "\\Device\\Harddisk0\\Partition1");
  symbolic_names("\\??\\d:", "\\Device\\Other");
  action = 6;
  call(0, 0, 0);
  assert(result.ntstatus == 0xc0000035u && !opens);
  name("\\??\\d:\\Child", 0);
  put(bank + 0x1108, 0);
  action = 1;
  call(0, 0, 0);
  assert(result.status == -2 && !opens);
  name("\\??\\d:\\Child", 0);
  expected_name = "Child";
  action = 1;
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus && live == 1);
  handle = result.handle;
  symbolic_names("\\??\\d:", "");
  action = 7;
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus);
  action = 1;
  call(0, 0, 0);
  assert(result.status == -2 && opens == 1);
  action = 3;
  call(0, 0, 0);
  assert(!result.ntstatus && !live);
  action = 7;
  call(0, 0, 0);
  assert(result.ntstatus == 0xc0000034u);
  create_link("\\??\\D:", "\\Device\\Harddisk0\\Partition1\\Cache");
  name("\\??\\D:", 0);
  expected_name = "";
  selected = 1;
  action = 1;
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus && live == 1);
  handle = result.handle;
  action = 3;
  call(0, 0, 0);
  /* Long chains have no historical32-hop or arbitrary execution quota. */
  char a[40], b[40];
  for (unsigned i = 0; i < 80; ++i) {
    snprintf(a, sizeof(a), "\\Chain%u", i);
    snprintf(b, sizeof(b), "\\Chain%u", i + 1);
    create_link(a, b);
  }
  create_link("\\Chain80", "\\??\\D:");
#ifdef GAMEPKG_IO_ALLOCATOR_TEST
  unsigned before_allocated = opens;
  for (unsigned fail = 1; fail <= 12; ++fail) {
    name("\\Chain0", 0);
    action = 1;
    allocation_failure = fail;
    call(0, 0, 0);
    assert(!allocation_failure && !result.status &&
           result.ntstatus == 0xc0000017u && opens == before_allocated &&
           !live);
  }
  symbolic_names("\\Allocated", "\\??\\D:");
  action = 6;
  allocation_failure = 1;
  call(0, 0, 0);
  assert(!result.status && result.ntstatus == 0xc0000017u);
  create_link("\\Allocated", "\\??\\D:");
  action = 7;
  allocation_failure = 1;
  call(0, 0, 0);
  assert(!result.status && result.ntstatus == 0xc0000017u);
  action = 7;
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus);
#endif

  name("\\Chain0", 0);
  action = 1;
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus && live == 1);
  handle = result.handle;
  action = 3;
  call(0, 0, 0);
  /* Reusing a link while consuming suffix components is terminating. */
  create_link("\\Device\\Repeat", "\\Device");
  create_link("\\Device\\Leaf", "\\??\\D:");
  name("\\Device\\Repeat\\Repeat\\Repeat\\Leaf", 0);
  action = 1;
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus && live == 1);
  handle = result.handle;
  action = 3;
  call(0, 0, 0);
  create_link("\\CycleA", "\\CycleB");
  create_link("\\CycleB", "\\CycleA");
  unsigned saved = opens;
  name("\\CycleA", 0);
  action = 1;
  call(0, 0, 0);
  assert(result.status == -2 && opens == saved);
  create_link("\\Self", "\\Self");
  name("\\Self", 0);
  action = 1;
  call(0, 0, 0);
  assert(result.status == -2 && opens == saved);
  char growth[240] = "\\Grow\\";
  memset(growth + 6, 'x', 200);
  growth[206] = 0;
  create_link("\\Grow", growth);
  name("\\Grow", 0);
  action = 1;
  call(0, 0, 0);
  assert(!result.status && result.ntstatus == 0xc0000106u && opens == saved);
  /* Targets are counted, checked virtual names, never native host paths. */
  const char *bad[] = {"\\Device\\..\\Outside", "\\Device\\.\\Outside",
                       "\\Device\\\\Outside", "\\Device/Outside"};
  for (unsigned i = 0; i < sizeof(bad) / sizeof(*bad); ++i) {
    symbolic_names("\\Bad", bad[i]);
    action = 6;
    call(0, 0, 0);
    assert(result.ntstatus == 0xc0000033u);
  }
  symbolic_names("\\Bad", "/tmp");
  call(0, 0, 0);
  assert(result.status == -2);
  symbolic_names("\\Bad", "\\Device\\End\\");
  call(0, 0, 0);
  assert(result.status == -2);
  symbolic_names("\\Device\\Harddisk0\\Partition1", "\\Other");
  call(0, 0, 0);
  assert(result.ntstatus == 0xc0000035u);
  symbolic_names("\\Device\\Harddisk0", "\\Other");
  call(0, 0, 0);
  assert(result.ntstatus == 0xc0000035u);
  symbolic_names("\\Device\\Harddisk0\\Partition1\\Link", "\\Other");
  call(0, 0, 0);
  assert(result.status == -2);
  symbolic_names("\\Device\\Missing\\Link", "\\Other");
  call(0, 0, 0);
  assert(result.ntstatus == 0xc000003au);
  symbolic_names("\\Chain0\\Child", "\\Other");
  call(0, 0, 0);
  assert(result.status == -2);
  symbolic_names("\\??", "\\Other");
  call(0, 0, 0);
  assert(result.ntstatus == 0xc0000035u);
  symbolic_names("", "\\Device\\CdRom0");
  call(0, 0, 0);
  assert(result.ntstatus == 0xc000000du);
  symbolic_names("\\Bad", "\\Device\\CdRom0");
  put(bank + 0x1400, 1);
  call(0, 0, 0);
  assert(result.status == -1);
  /* Embedded NUL is not a string terminator in this counted ABI. */
  symbolic_names("\\Bad", "\\Device\\CdRom0");
  bank[0x1603] = 0;
  call(0, 0, 0);
  assert(result.ntstatus == 0xc0000033u);
  teardown();
  /* Every input descriptor/name byte can fault before namespace mutation. */
  for (unsigned part = 0; part < 4; ++part) {
    const unsigned starts[4] = {0x1400, 0x1420, 0x1500, 0x1600};
    const unsigned sizes[4] = {8, 8, 6, 14};
    for (unsigned i = 0; i < sizes[part]; ++i) {
      setup_access(starts[part] + i, 2);
      symbolic_names("\\??\\D:", "\\Device\\CdRom0");
      action = 6;
      call(14, 1, 0x1000 + starts[part] + i);
      assert(!opens && !live);
      /* Independent readable input proves no publication/failed-delete loss. */
      put(bank + 0x1800, 6u | (7u << 16));
      put(bank + 0x1804, 0x2900);
      memcpy(bank + 0x1900, "\\??\\D:", 6);
      link_pointer = 0x2800;
      action = 7;
      call(0, 0, 0);
      assert(result.ntstatus == 0xc0000034u);
      if (part == 0 || part == 2) {
        action = 6;
        call(0, 0, 0);
        assert(!result.status && !result.ntstatus);
        link_pointer = 0x2400;
        action = 7;
        call(14, 1, 0x1000 + starts[part] + i);
        link_pointer = 0x2800;
        action = 7;
        call(0, 0, 0);
        assert(!result.status && !result.ntstatus);
      }
      teardown();
    }
  }
}

static void directory_create_suite(void) {
  setup(8192);
  options = 0x4021;
  expected_name = "TDATA";
  name("\\Device\\Harddisk0\\Partition1\\TDATA", 0);
  action = 8;
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus && result.handle == 4 &&
         result.information == 2 && get(bank + 0x1014) == 2 && creates == 1 &&
         live == 1);
  uint32_t parent = result.handle;
  name("Title", parent);
  expected_name = "Title";
  expected_parent = 0xabcdef01;
  create_information = 1;
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus && result.handle == 8 &&
         result.information == 1 && get(bank + 0x1014) == 1 && creates == 2 &&
         live == 2);
  handle = result.handle;
  action = 3;
  call(0, 0, 0);
  handle = parent;
  call(0, 0, 0);
  name("\\Device\\Harddisk0\\Partition1\\Cache\\New", 0);
  action = 8;
  call(0, 0, 0);
  assert(result.status == -2 && creates == 2 &&
         !live); /* Optional backend extension. */
  name("\\Device\\Harddisk0\\Partition1\\TDATA", 0);
  expected_name = "TDATA";
  expected_parent = 0;
  allocation_pointer = 0x2700;
  expected_allocation[0] = 0x89abcdef;
  expected_allocation[1] = 0x12345678;
  expected_allocation[2] = 1;
  put(bank + 0x1700, expected_allocation[0]);
  put(bank + 0x1704, expected_allocation[1]);
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus && creates == 3 && live == 1);
  handle = result.handle;
  action = 3;
  call(0, 0, 0);
  allocation_pointer = 0xfffffffc;
  action = 8;
  call(0, 0, 0);
  assert(result.status == -1 && creates == 3 && !live);
  teardown();
  for (unsigned k = 0; k < 12; ++k) {
    unsigned denied = k < 4 ? 0x1000 + k : 0x1010 + k - 4;
    setup(denied);
    options = 0x4021;
    expected_name = "TDATA";
    name("\\Device\\Harddisk0\\Partition1\\TDATA", 0);
    action = 8;
    call(14, 3, 0x1000 + denied);
    assert(!creates && !opens && !live);
    teardown();
  }
  for (unsigned k = 0; k < 8; ++k) {
    setup_access(0x1700 + k, 2);
    options = 0x4021;
    expected_name = "TDATA";
    allocation_pointer = 0x2700;
    name("\\Device\\Harddisk0\\Partition1\\TDATA", 0);
    action = 8;
    call(14, 1, 0x2700 + k);
    assert(!creates && !opens && !live);
    allocation_pointer = 0;
    call(0, 0, 0);
    assert(!result.status && !result.ntstatus && result.handle == 4 &&
           live == 1);
    handle = result.handle;
    action = 3;
    call(0, 0, 0);
    teardown();
  }
}
static void regular_setup(unsigned denied) {
  setup(denied);
  options = 0x22;
  share = 1;
  desired_access = 0x40100000;
  file_attributes = 4;
  backend_kind = 2;
  expected_name = "UDATA\\4541005b\\TitleMeta.xbx";
  name("\\Device\\Harddisk0\\Partition1\\UDATA\\4541005b\\TitleMeta.xbx", 0);
  action = 8;
}
static void regular_create_suite(void) {
  regular_setup(8192);
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus && result.handle == 4 &&
         result.information == 2 && creates == 1 && live == 1);
  GamePkgX86HandleLookup found = gamepkg_x86_handles_lookup(
      handles, thread, result.handle, 0x1234, 0x5678);
  assert(!found.status && !found.ntstatus &&
         found.value.kind == GAMEPKG_X86_HANDLE_FILE &&
         found.value.access == desired_access && found.value.data[3] == 0x22);
  handle = result.handle;
  action = 3;
  call(0, 0, 0);
  action = 8;
  create_information = 1;
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus && result.information == 1 &&
         live == 1);
  handle = result.handle;
  action = 3;
  call(0, 0, 0);
  action = 8;
  options |= 8; /* No-buffering has not been qualified. */
  call(0, 0, 0);
  assert(result.status == -2 && creates == 2 && !live);
  teardown();
  for (unsigned k = 0; k < 12; ++k) {
    unsigned denied = k < 4 ? 0x1000 + k : 0x1010 + k - 4;
    regular_setup(denied);
    call(14, 3, 0x1000 + denied);
    assert(!creates && !opens && !live);
    teardown();
  }
}
static void query_setup(unsigned denied) {
  regular_setup(denied);
  io = 0x2800;
  call(0, 0, 0);
  io = 0x2010;
  handle = result.handle;
  action = 13;
  length = 56;
  info_class = 34;
  memset(bank + 0x1200, 0xab, 64);
}
static void query_suite(void) {
  query_setup(8192);
  for (unsigned n = 0; n <= 64; ++n) {
    length = n;
    memset(bank + 0x1200, 0xab, 64);
    unsigned before = queries;
    call(0, 0, 0);
    assert(!result.status);
    if (n < 56) {
      assert(result.ntstatus == 0xc0000004u && !result.information &&
             queries == before);
      for (unsigned j = 0; j < 64; ++j)
        assert(bank[0x1200 + j] == 0xab);
    } else {
      assert(!result.ntstatus && result.information == 56 &&
             queries == before + 1);
      for (unsigned j = 0; j < 13; ++j)
        assert(get(bank + 0x1200 + 4 * j) == query_fields[j]);
      for (unsigned j = 52; j < 64; ++j)
        assert(bank[0x1200 + j] == 0xab);
    }
    assert(get(bank + 0x1010) == result.ntstatus &&
           get(bank + 0x1014) == result.information);
  }
  length = 56;
  memset(bank + 0x1200, 0xab, 64);
  query_status = 0xc0000022u;
  call(0, 0, 0);
  assert(!result.status && result.ntstatus == query_status &&
         !result.information);
  for (unsigned j = 0; j < 64; ++j)
    assert(bank[0x1200 + j] == 0xab);
  query_status = 0;
  query_host = -2;
  put(bank + 0x1010, 0xfeed1234);
  call(0, 0, 0);
  assert(result.status == -2 && get(bank + 0x1010) == 0xfeed1234);
  query_host = 0;
  query_bad = 1;
  call(0, 0, 0);
  assert(result.status == -1 && get(bank + 0x1010) == 0xfeed1234);
  query_bad = 0;
  unsigned before = queries;
  info_class = 999;
  call(0, 0, 0);
  assert(result.status == -2 && queries == before);
  info_class = 34;
  output = 0xfffffff0;
  call(0, 0, 0);
  assert(result.status == -1 && queries == before);
  output = 0x2200;
  io = output + 8; /* Defined ordered alias: payload first, IOSB last. */
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus && get(bank + 0x1208) == 0 &&
         get(bank + 0x120c) == 56);
  action = 3;
  call(0, 0, 0);
  action = 13;
  io = 0x2010;
  before = queries;
  call(0, 0, 0);
  assert(!result.status && result.ntstatus == 0xc0000008u && queries == before);
  teardown();
  for (unsigned k = 0; k < 64; ++k) {
    unsigned denied = k < 8 ? 0x1010 + k : 0x1200 + k - 8;
    query_setup(denied);
    call(14, 3, 0x1000 + denied);
    assert(!queries && live == 1);
    action = 3;
    call(0, 0, 0);
    teardown();
  }
  setup(8192);
  selected = 1;
  name("\\Device\\Harddisk0\\Partition1\\Cache", 0);
  action = 1;
  call(0, 0, 0);
  handle = result.handle;
  action = 13;
  length = 56;
  info_class = 34;
  call(0, 0, 0);
  assert(result.status == -2 && !queries && live == 1);
  action = 3;
  call(0, 0, 0);
  teardown();
}
static void transfer_setup(unsigned denied, unsigned permission,
                           unsigned write) {
  setup_access(denied, permission);
  options = 0x22;
  share = 1;
  desired_access = write ? 0x40100000u : 0x80100000u;
  file_attributes = 4;
  backend_kind = 2;
  expected_name = "Data";
  name("\\Device\\Harddisk0\\Partition1\\Data", 0);
  memcpy(bank + 0x1900, bank + 0x1300, 64);
  put(bank + 0x1124, 0x2900);
  action = 8;
  io = 0x2800;
  call(0, 0, 0);
  handle = result.handle;
  assert(!result.status && !result.ntstatus && live == 1);
  io = 0x2010;
  action = 14;
  transfer_write = write;
  length = 1025;
  put(bank + 0x1010, 0x12345678);
  put(bank + 0x1014, 0xabcdef01);
  for (unsigned i = 0; i < 1025; ++i)
    bank[0x1200 + i] = write ? (unsigned char)(i * 17 + 3) : 0xab;
}
static void transfer_teardown(void) {
  action = 3;
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus);
  teardown();
}
static void transfer_suite(void) {
  const unsigned sizes[] = {0, 1, 44, 511, 512, 513, 1025};
  for (unsigned write = 0; write < 2; ++write)
    for (unsigned k = 0; k < sizeof(sizes) / sizeof(*sizes); ++k) {
      transfer_setup(8192, 1, write);
      length = sizes[k];
      if (!length)
        output = 0;
      call(0, 0, 0);
      assert(!result.status && !result.ntstatus &&
             result.information == length && stages == 1 && transfers == 1 &&
             unstages == 1 && !active_buffers);
      assert(get(bank + 0x1010) == 0 && get(bank + 0x1014) == length);
      if (!write)
        for (unsigned i = 0; i < 1025; ++i)
          assert(bank[0x1200 + i] ==
                 (i < length ? (unsigned char)(i * 17 + 3) : 0xab));
      transfer_teardown();
    }
  transfer_setup(8192, 1, 0);
  transfer_bytes = 33;
  offset_pointer = 0x2700;
  expected_offset[0] = 0x89abcdef;
  expected_offset[1] = 0x12345678;
  put(bank + 0x1700, expected_offset[0]);
  put(bank + 0x1704, expected_offset[1]);
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus && result.information == 33 &&
         !active_buffers);
  for (unsigned i = 0; i < 1025; ++i)
    assert(bank[0x1200 + i] == (i < 33 ? (unsigned char)(i * 17 + 3) : 0xab));
  transfer_teardown();
  transfer_setup(8192, 1, 0);
  transfer_bytes = 0;
  transfer_status = 0xc0000011u;
  call(0, 0, 0);
  assert(!result.status && result.ntstatus == transfer_status &&
         !result.information && !active_buffers &&
         get(bank + 0x1010) == transfer_status);
  transfer_teardown();
  transfer_setup(8192, 1, 1);
  transfer_complete = 0;
  transfer_status = 0xc000000du;
  call(0, 0, 0);
  assert(!result.status && result.ntstatus == transfer_status &&
         !active_buffers && get(bank + 0x1010) == 0x12345678 &&
         get(bank + 0x1014) == 0xabcdef01);
  transfer_teardown();
  for (unsigned mode = 0; mode < 3; ++mode) {
    transfer_setup(8192, 1, 1);
    if (!mode)
      stage_failure = 1;
    else if (mode == 1)
      transfer_host = -2;
    else
      transfer_bad = 1;
    call(0, 0, 0);
    assert(!active_buffers && stages == 1 && get(bank + 0x1010) == 0x12345678);
    if (!mode)
      assert(!result.status && result.ntstatus == 0xc0000017u && !transfers &&
             !unstages);
    else
      assert(result.status == (mode == 1 ? -2 : -1) && transfers == 1 &&
             unstages == 1);
    transfer_teardown();
  }
  transfer_setup(8192, 1, 1);
  for (unsigned mode = 0; mode < 3; ++mode) {
    transfer_event = mode == 0;
    transfer_apc = mode == 1;
    transfer_apc_context = mode == 2;
    call(0, 0, 0);
    assert(result.status == -2 && !stages);
  }
  transfer_event = transfer_apc = transfer_apc_context = 0;
  transfer_write = 0;
  call(0, 0, 0);
  assert(!result.status && result.ntstatus == 0xc0000022u && !stages &&
         get(bank + 0x1010) == 0x12345678);
  transfer_teardown();
  for (unsigned write = 0; write < 2; ++write) {
    for (unsigned k = 0; k < 1033; ++k) {
      unsigned denied = k < 8 ? 0x1010 + k : 0x1200 + k - 8;
      transfer_setup(denied, k < 8 || !write ? 1 : 2, write);
      call(14, k < 8 || !write ? 3 : 1, 0x1000 + denied);
      assert(!stages && !transfers && !active_buffers);
      transfer_teardown();
    }
    for (unsigned byte = 0; byte < 8; ++byte) {
      transfer_setup(0x1700 + byte, 2, write);
      offset_pointer = 0x2700;
      call(14, 1, 0x2700 + byte);
      assert(!stages && !transfers && !active_buffers);
      transfer_teardown();
    }
    transfer_setup(8192, 1, write);
    copy_fault_countdown = 2;
    call(14, write ? 1 : 3, 0x2400);
    assert(stages == 1 && unstages == 1 && !active_buffers &&
           transfers == (write ? 0u : 1u));
    assert(get(bank + 0x1010) == 0x12345678 &&
           get(bank + 0x1014) == 0xabcdef01);
    transfer_teardown();
  }
}
static void ansi_suite(void) {
  setup(8192);
  action = 9;
  const unsigned sizes[] = {0, 1, 43, 255, 65533, 65534, 65535, 65536, 100000};
  for (unsigned i = 0; i < sizeof(sizes) / sizeof(*sizes); ++i) {
    unsigned n = sizes[i];
    memset(long_bank, 'x', n);
    long_bank[n] = 0;
    ansi_source = 0x100000;
#if defined(_WIN32) && defined(GAMEPKG_LEGACY_ANSI_WINE_ORACLE)
    NativeAnsi native = {0};
    RtlInitAnsiString(&native, (const char *)long_bank);
    assert(native.buffer == (const char *)long_bank &&
           native.length == (n & 65535u) &&
           native.maximum == ((n + 1) & 65535u));
#endif
    call(0, 0, 0);
    assert(!result.status &&
           get(bank + 0x1400) == ((n & 65535u) | (((n + 1) & 65535u) << 16)) &&
           get(bank + 0x1404) == ansi_source);
  }
#if defined(_WIN32) && defined(GAMEPKG_LEGACY_ANSI_WINE_ORACLE)
  NativeAnsi null_native = {1, 1, (const char *)1};
  RtlInitAnsiString(&null_native, NULL);
  assert(!null_native.length && !null_native.maximum && !null_native.buffer);
#endif
  memcpy(bank + 0x1500, "abc", 4);
  ansi_source = 0x2500;
  action = 10;
  const uint32_t levels[] = {1, 2, 31};
  for (unsigned i = 0; i < 3; ++i) {
    ansi_priority = levels[i];
    call(0, 0, 0);
    assert(!result.status);
  }
  action = 9;
  ansi_source = 0;
  call(0, 0, 0);
  assert(!result.status && !get(bank + 0x1400) && !get(bank + 0x1404));
  memcpy(bank + 0x1400, "Alias", 6);
  ansi_source = ansi_destination;
  call(0, 0, 0);
  assert(!result.status && get(bank + 0x1400) == (5u | (6u << 16)) &&
         get(bank + 0x1404) == ansi_source);
  ansi_destination = 0xfffffff9;
  call(0, 0, 0);
  assert(result.status == -1);
  teardown();
  for (unsigned i = 0; i < 8; ++i) {
    setup(0x1400 + i);
    memcpy(bank + 0x1500, "abc", 4);
    memset(bank + 0x1400, 0xa5, 8);
    action = 9;
    call(14, 3, 0x2400 + i);
    for (unsigned j = 0; j < 8; ++j)
      assert(bank[0x1400 + j] == 0xa5);
    teardown();
  }
  setup_access(0x1700, 2);
  memcpy(bank + 0x16fc, "abc", 4);
  ansi_source = 0x26fc;
  action = 9;
  call(0, 0, 0);
  assert(!result.status && get(bank + 0x1400) == (3u | (4u << 16)));
  teardown();
  setup_access(0x1503, 2);
  memcpy(bank + 0x1500, "abcd", 5);
  memset(bank + 0x1400, 0xa5, 8);
  action = 9;
  call(14, 1, 0x2503);
  for (unsigned j = 0; j < 8; ++j)
    assert(bank[0x1400 + j] == 0xa5);
  teardown();
}
static void equal_input(unsigned n, const char *a, const char *b) {
  put(bank + 0x1400, n);
  put(bank + 0x1404, 0x2500);
  put(bank + 0x1420, n);
  put(bank + 0x1424, 0x2600);
  memcpy(bank + 0x1500, a, n);
  memcpy(bank + 0x1600, b, n);
  action = 12;
  equal_mode = 0;
}
static void equal_suite(void) {
  setup(8192);
  action = 11;
  call(0, 0, 0);
  assert(equal_checks == 131072);
  memset(long_bank, 'A', 65536);
  put(bank + 0x1400, 65535);
  put(bank + 0x1404, 0x100000);
  put(bank + 0x1420, 65535);
  put(bank + 0x1424, 0x100000);
  action = 12;
  equal_mode = 0;
  call(0, 0, 0);
  assert(!result.status && result.information);
  long_bank[65535] = 'B';
  put(bank + 0x1424, 0x100001);
  call(0, 0, 0);
  assert(!result.status && !result.information);
#if defined(_WIN32) && defined(GAMEPKG_LEGACY_ANSI_WINE_ORACLE)
  NativeAnsi la = {65535, 0, (const char *)long_bank},
             lb = {65535, 0, (const char *)long_bank + 1};
  assert(!RtlEqualString(&la, &lb, 0));
#endif
  equal_input(3, "A\0x", "A\0y");
  call(0, 0, 0);
  assert(!result.status && !result.information);
#if defined(_WIN32) && defined(GAMEPKG_LEGACY_ANSI_WINE_ORACLE)
  NativeAnsi x = {3, 0, (const char *)bank + 0x1500},
             y = {3, 0, (const char *)bank + 0x1600};
  assert(!RtlEqualString(&x, &y, 0));
#endif
  equal_mode = 1;
  call(0, 0, 0);
  assert(!result.status && !result.information);
#if defined(_WIN32) && defined(GAMEPKG_LEGACY_ANSI_WINE_ORACLE)
  assert(!RtlEqualString(&x, &y, 1));
#endif
  equal_input(1, "a", "A");
  equal_mode = 255;
  call(0, 0, 0);
  assert(!result.status && result.information);
  equal_mode = 256;
  call(0, 0, 0);
  assert(!result.status && !result.information);
  teardown();
  setup_access(0x1404, 2);
  put(bank + 0x1400, 1);
  put(bank + 0x1420, 2);
  action = 12;
  call(0, 0, 0);
  assert(!result.status && !result.information);
  put(bank + 0x1400, 0);
  put(bank + 0x1420, 0);
  call(0, 0, 0);
  assert(!result.status && result.information);
  teardown();
  setup_access(0x1402, 2);
  equal_input(1, "a", "a");
  call(0, 0, 0);
  assert(!result.status && result.information);
  teardown();
  setup_access(0x1700, 2);
  equal_input(2, "ab", "ax");
  put(bank + 0x1424, 0x26ff);
  bank[0x16ff] = 'x';
  call(0, 0, 0);
  assert(!result.status && !result.information);
  bank[0x16ff] = 'a';
  call(14, 1, 0x2700);
  teardown();
  for (unsigned side = 0; side < 2; ++side)
    for (unsigned byte = 0; byte < 2; ++byte) {
      unsigned off = (side ? 0x1420 : 0x1400) + byte;
      setup_access(off, 2);
      equal_input(1, "a", "a");
      call(14, 1, 0x1000 + off);
      teardown();
    }
  for (unsigned side = 0; side < 2; ++side)
    for (unsigned byte = 0; byte < 4; ++byte) {
      unsigned off = (side ? 0x1424 : 0x1404) + byte;
      setup_access(off, 2);
      equal_input(1, "a", "a");
      call(14, 1, 0x1000 + off);
      teardown();
    }
  for (unsigned side = 0; side < 2; ++side)
    for (unsigned byte = 0; byte < 3; ++byte) {
      unsigned off = (side ? 0x1600 : 0x1500) + byte;
      setup_access(off, 2);
      equal_input(3, "abc", "abc");
      call(14, 1, 0x1000 + off);
      teardown();
    }
  printf("ANSI equality: %u byte-pair comparisons, including embedded NUL and "
         "metadata/fault cases\n",
         equal_checks);
}
int main(void) {
  setup(8192);
  name("\\Device\\Harddisk0\\partition1\\", 0);
  action = 1;
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus && result.handle == 4);
  handle = result.handle;
  assert(opens == 1 && get(bank + 0x1000) == handle &&
         get(bank + 0x1010) == 0 && get(bank + 0x1014) == 1);
  action = 4;
  call(0, 0, 0);
  action = 5;
  call(0, 0, 0);
  action = 2;
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus && volumes == 1);
  const uint32_t want[6] = {0x89abcdef, 0x12345678, 0x76543210,
                            0x01234567, 32,         512};
  for (unsigned i = 0; i < 6; ++i)
    assert(get(bank + 0x1200 + 4 * i) == want[i]);
  assert(get(bank + 0x1014) == 24);
  length = 23;
  memset(bank + 0x1200, 0xa5, 24);
  call(0, 0, 0);
  assert(result.ntstatus == 0xc0000004u && volumes == 1 &&
         bank[0x1200] == 0xa5);
  length = 24;
  info_class = 4;
  call(0, 0, 0);
  assert(result.status == -2 && volumes == 1);
  info_class = 3;
  name("Child", handle);
  expected_parent = 0xabcdef01;
  expected_name = "Child";
  action = 1;
  call(0, 0, 0);
  assert(result.handle == 8 && live == 2);
  uint32_t child = result.handle;
  const char *bad[] = {"..",       ".\\Child", "Child\\..",
                       "Child//x", "Child:x",  "Child\\\\x"};
  unsigned saved = opens;
  for (unsigned i = 0; i < sizeof(bad) / sizeof(*bad); ++i) {
    name(bad[i], handle);
    call(0, 0, 0);
    assert(result.ntstatus == 0xc0000033u && opens == saved);
  }
  name("Child", 0x123);
  call(0, 0, 0);
  assert(result.ntstatus == 0xc0000008u);
  name("\\Device\\Harddisk0\\Partition1\\\\", 0);
  call(0, 0, 0);
  assert(result.ntstatus == 0xc0000033u && opens == saved);
  name("\\Device\\Harddisk0\\Partition12", 0);
  call(0, 0, 0);
  assert(result.status == -2);
  name("\\Device\\Harddisk0\\Partition1\\Cache\\", 0);
  expected_name = "";
  expected_parent = 0;
  selected = 1;
  call(0, 0, 0);
  assert(result.handle == 12 && live == 3);
  uint32_t cache = result.handle;
  action = 3;
  fail_close = 1;
  call(0, 0, 0);
  assert(result.ntstatus == 0xc0000022u && live == 3);
  fail_close = 0;
  call(0, 0, 0);
  assert(!result.ntstatus && live == 2);
  call(0, 0, 0);
  assert(result.ntstatus == 0xc0000008u && live == 2);
  handle = child;
  call(0, 0, 0);
  handle = cache;
  call(0, 0, 0);
  assert(!live);
  action = 1;
  selected = 0;
  open_status = 0xc0000022;
  name("\\Device\\Harddisk0\\Partition1", 0);
  call(0, 0, 0);
  assert(result.ntstatus == open_status && !live);
  open_status = 0;
  put(bank + 0x1120, 2); /* Length greater than MaximumLength. */
  unsigned before_open = opens;
  call(0, 0, 0);
  assert(result.status == -1 && opens == before_open);
  name("\\Device\\Harddisk0\\Partition1", 0);
  bank[0] = 1;
  call(0, 0, 0);
  assert(result.status == -1 && opens == before_open);
  bank[0] = 0;
  hp = io;
  call(0, 0, 0);
  assert(!result.status && !result.ntstatus && result.handle &&
         get(bank + 0x1010) == 0 && get(bank + 0x1014) == 1 && live == 1);
  assert(gamepkg_x86_io_release(service) == -1);
  fail_close = 1;
  assert(gamepkg_x86_handles_drain(handles) == -1 && live == 1);
  fail_close = 0;
  assert(!gamepkg_x86_handles_drain(handles) && !live);
  teardown();
  /* Every byte of both open outputs is checked before native object creation.
   */
  for (unsigned k = 0; k < 12; ++k) {
    unsigned denied = k < 4 ? 0x1000 + k : 0x1010 + k - 4;
    setup(denied);
    name("\\Device\\Harddisk0\\Partition1", 0);
    action = 1;
    call(14, 3, 0x1000 + denied);
    assert(!opens && !live);
    teardown();
  }
  /* Every byte of volume output is checked before querying external storage. */
  for (unsigned k = 0; k < 24; ++k) {
    setup(0x1200 + k);
    name("\\Device\\Harddisk0\\Partition1", 0);
    action = 1;
    call(0, 0, 0);
    handle = result.handle;
    action = 2;
    call(14, 3, 0x2200 + k);
    assert(!volumes && live == 1);
    action = 3;
    call(0, 0, 0);
    teardown();
  }
  symbolic_link_suite();
  directory_create_suite();
  regular_create_suite();
  query_suite();
  transfer_suite();
  ansi_suite();
  equal_suite();
  printf("I/O protocol: %u calls/fault boundaries, x87/MMX/XMM/GPR preserved\n",
         checks);
  return 0;
}
