/* Exact generated virtual CLI/STI. Never execute native privileged opcodes. */
#include <stdatomic.h>
#ifndef _WIN32
#include <sched.h>
#endif
static GamePkgX86Cpu *serial_cpu;
static GamePkgX86CpuThread *serial_visitor;
static atomic_uint serial_phase, serial_entered;
#ifdef _WIN32
static HANDLE serial_handle;
#else
static pthread_t serial_handle;
#endif
static void serial_visit(void) {
  assert(!gamepkg_x86_cpu_enter(serial_visitor));
  assert(atomic_load(&serial_phase) == 2);
  assert(atomic_fetch_add(&serial_entered, 1) == 0);
  assert(!gamepkg_x86_cpu_block(serial_visitor));
}
#ifdef _WIN32
static DWORD WINAPI serial_worker(void *unused) {
  (void)unused;
  serial_visit();
  return 0;
}
#else
static void *serial_worker(void *unused) {
  (void)unused;
  serial_visit();
  return NULL;
}
#endif
static void enqueue_visitor(uint32_t pc) {
  assert(pc == 0x2000 && !(g_eflags & 0x200));
  assert(!atomic_load(&serial_phase) && !atomic_load(&serial_entered));
  atomic_store(&serial_phase, 1);
#ifdef _WIN32
  serial_handle = CreateThread(NULL, 0, serial_worker, NULL, 0, NULL);
  assert(serial_handle);
#else
  assert(!pthread_create(&serial_handle, NULL, serial_worker, NULL));
#endif
  while (!gamepkg_x86_cpu_waiters(serial_cpu)) {
#ifdef _WIN32
    SwitchToThread();
#else
    sched_yield();
#endif
  }
  assert(!atomic_load(&serial_entered));
  uint32_t target = gamepkg_x86_dispatch_ret(4, 0, pc, g_ebp);
  assert(!gamepkg_x86_dispatch_retire(g_ebp));
  gamepkg_x86_dispatch_transfer(target, g_ebp);
}
static int serial_poll(void *opaque, const GamePkgX86InterruptState *state,
                       uint32_t kind, uint32_t pc, uint32_t bp) {
  (void)opaque;
  (void)bp;
  assert(gamepkg_x86_interrupt_eligible(state) == 1 &&
         kind == GAMEPKG_X86_CHECKPOINT_INSTRUCTION);
  if (pc == 0x12099) {
    assert(atomic_load(&serial_phase) == 1 && !atomic_load(&serial_entered));
    assert(g_ebx == 1 && g_edx == 1 && !g_esi &&
           gamepkg_x86_cpu_waiters(serial_cpu) == 1);
    atomic_store(&serial_phase, 2);
  } else if (pc == 0x1209a)
    assert(atomic_load(&serial_entered) == 1);
  return 0;
}
static int trace_cpu_step(void *opaque, uint32_t op, uint32_t *flags) {
  ControlTrace *trace = opaque;
  assert(gamepkg_x86_dispatch_checkpoint_kind() == -1);
  if (trace->mutate)
    trace->mutate->interrupt_step = NULL;
  if (trace->bad_action == 7) {
    assert(gamepkg_x86_dispatch_interrupt(op, g_ebp) == 1);
    return -1;
  }
  if (trace->bad_action == 8)
    return -1;
  int result = gamepkg_x86_cpu_step(trace->cpu, g_eflags, op);
  if (!result) {
    GamePkgX86InterruptState state;
    assert(!gamepkg_x86_cpu_snapshot(trace->cpu, &state));
    *flags = state.flags;
  }
  return result;
}
static void privileged_prepare(GamePkgX86CpuThread *thread,
                               GamePkgX86InterruptState *state, uint32_t flags,
                               uint32_t cpl, uint32_t pvi, uint32_t shadow) {
  prepare_compare();
  g_eflags = flags;
  *state = (GamePkgX86InterruptState){flags, 1, cpl, pvi, 0, shadow};
  assert(!gamepkg_x86_cpu_restore(thread, state));
}
static void privileged_cases(const GamePkgX86FpSource *source) {
  prepare_compare();
  g_esi = g_edi = 0;
  GamePkgX86Cpu *cpu = gamepkg_x86_cpu_create();
  assert(cpu);
  GamePkgX86InterruptState state = {g_eflags, 1, 0, 0, 0, 0};
  GamePkgX86CpuThread *thread = gamepkg_x86_cpu_thread_create(cpu, &state);
  assert(thread && !gamepkg_x86_cpu_enter(thread));
  GamePkgX87State old_fp = x87;
  x87.control = 0x37e;
  x87.status = 0x81;
  GamePkgX87State raw_fp = x87;
  uint32_t raw_xmm[8][4];
  memcpy(raw_xmm, xmm, sizeof(raw_xmm));
  GamePkgX86DispatchExit out;
  CpuEventTrace event = {.thread = thread, .pending = 1, .stop_kind = 0};
  GamePkgX86CpuEventSource events = {&event, poll_event};
  /* A request already pending at CLI is observed BEFORE the instruction. */
  assert(gamepkg_x86_dispatch_run_cpu_events(controlled_map, controlled_count,
                                             0x12000, &out, source, thread,
                                             &events) == 4 &&
         out.pc == 0x12000 && !g_ebx && g_eflags == 0x203);
  event.stop_pc = 0x12004;
  assert(gamepkg_x86_dispatch_run_cpu_events(controlled_map, controlled_count,
                                             out.pc, &out, source, thread,
                                             &events) == 4 &&
         out.pc == 0x12004 && g_ebx == 1 && g_edx == 1 && !g_esi &&
         event.requests == 2 && event.polls == 3);
  assert(!gamepkg_x86_cpu_snapshot(thread, &state) && !state.sti_shadow &&
         state.flags == g_eflags);
  event.pending = 0;
  assert(!gamepkg_x86_dispatch_run_cpu_events(controlled_map, controlled_count,
                                              out.pc, &out, source, thread,
                                              &events) &&
         g_esi == 1);
  /* STI;CLI keeps the pending request inhibited throughout. */
  privileged_prepare(thread, &state, 2, 0, 0, 0);
  event = (CpuEventTrace){.thread = thread, .pending = 1, .stop_kind = 0};
  assert(!gamepkg_x86_dispatch_run_cpu_events(controlled_map, controlled_count,
                                              0x12010, &out, source, thread,
                                              &events) &&
         !event.polls && !event.requests && g_ebx == 1);
  assert(!gamepkg_x86_cpu_snapshot(thread, &state) && !(state.flags & 0x200) &&
         !state.sti_shadow);
  /* A second STI with IF already1 does not extend the previous shadow. */
  privileged_prepare(thread, &state, 2, 0, 0, 0);
  event = (CpuEventTrace){.thread = thread, .pending = 1, .stop_kind = 0};
  assert(gamepkg_x86_dispatch_run_cpu_events(controlled_map, controlled_count,
                                             0x12020, &out, source, thread,
                                             &events) == 4 &&
         out.pc == 0x12022 && !g_ebx && event.polls == 1 &&
         event.requests == 1 && g_eflags == 0x202);
  /* Shadow covers the entire REP, not its first element or callback. */
  privileged_prepare(thread, &state, 2, 0, 0, 0);
  for (unsigned i = 0; i < 4; ++i)
    ram[0x100 + i] = (unsigned char)(i + 1);
  event = (CpuEventTrace){
      .thread = thread, .pending = 1, .stop_kind = 0, .stop_pc = 0x12033};
  assert(gamepkg_x86_dispatch_run_cpu_events(controlled_map, controlled_count,
                                             0x12030, &out, source, thread,
                                             &events) == 4 &&
         out.pc == 0x12033 && !g_ecx && g_esi == 0x100104 &&
         g_edi == 0x100204 && !g_ebx && event.polls == 1 &&
         event.flags.saved_flags == 0x202 && event.flags.handler_flags == 2);
  for (unsigned i = 0; i < 4; ++i)
    assert(ram[0x200 + i] == i + 1);
  /* Real GP from insufficient privilege preserves the instruction flags and
   * banks. Detected exception notification ends a preexisting shadow. */
  privileged_prepare(thread, &state, 0x203, 3, 0, 1);
  event = (CpuEventTrace){.thread = thread};
  assert(gamepkg_x86_dispatch_run_cpu_events(controlled_map, controlled_count,
                                             0x12040, &out, source, thread,
                                             &events) == 1 &&
         out.pc == 0x12040 && out.fault.core.vector == 13 &&
         out.fault.core.eflags == 0x203 && g_eflags == 0x203);
  assert(!gamepkg_x86_cpu_snapshot(thread, &state) && !state.sti_shadow &&
         state.flags == 0x203);
  /* PVI modifies VIF, never physical IF or the physical STI shadow. VIP can
   * reject STI while CLI still clears VIF. These are protected32 cases. */
  const uint32_t virtual_flags[] = {0x180002, 2, 0x100002};
  const uint32_t virtual_pc[] = {0x12040, 0x12050, 0x12050};
  const uint32_t virtual_expected[] = {0x100002, 0x80002, 0x100002};
  for (unsigned i = 0; i < 3; ++i) {
    privileged_prepare(thread, &state, virtual_flags[i], 3, 1, 0);
    event = (CpuEventTrace){.thread = thread, .pending = 1, .stop_kind = 0};
    int reason = gamepkg_x86_dispatch_run_cpu_events(
        controlled_map, controlled_count, virtual_pc[i], &out, source, thread,
        &events);
    assert(reason == (i == 2 ? 1 : 0) && g_eflags == virtual_expected[i] &&
           !event.polls);
    if (i == 2)
      assert(out.pc == 0x12050 && out.fault.core.vector == 13 &&
             out.fault.core.eflags == virtual_flags[i]);
    assert(!gamepkg_x86_cpu_snapshot(thread, &state) && !state.sti_shadow &&
           state.flags == g_eflags);
  }
  /* A fault in the next instruction wins over an inhibited host request and
   * retains the newly enabled IF in the real fault snapshot. */
  privileged_prepare(thread, &state, 2, 0, 0, 0);
  g_ecx = 0;
  g_eax = 21;
  event = (CpuEventTrace){.thread = thread, .pending = 1, .stop_kind = 0};
  assert(gamepkg_x86_dispatch_run_cpu_events(controlled_map, controlled_count,
                                             0x12060, &out, source, thread,
                                             &events) == 1 &&
         out.pc == 0x12061 && out.fault.core.vector == 0 &&
         out.fault.core.eflags == 0x202 && g_eax == 21 && !event.polls);
  assert(!gamepkg_x86_cpu_snapshot(thread, &state) && !state.sti_shadow &&
         state.flags == 0x202);
  /* Without the explicit CPU provider, the valid opcode is a host protocol
   * stop, not a fabricated guest fault or a silently ignored instruction. */
  privileged_prepare(thread, &state, 0x203, 0, 0, 0);
  ControlTrace trace = {0};
  GamePkgX86DispatchControl control = {&trace, trace_boundary, trace_retire,
                                       NULL};
  assert(gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                             0x12040, &out, source,
                                             &control) == 3 &&
         !trace.retired && !trace.events && g_eflags == 0x203);
  GamePkgX86FpFault zero = {0};
  assert(!memcmp(&out.fault, &zero, sizeof(zero)));
  /* All four controller fields are frozen before entry. */
  privileged_prepare(thread, &state, 0x203, 0, 0, 0);
  trace =
      (ControlTrace){.cpu = thread, .shadow_pc = 0x12082, .mutate = &control};
  control = (GamePkgX86DispatchControl){&trace, trace_boundary, trace_retire,
                                        trace_cpu_step};
  assert(!gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                              0x12080, &out, source,
                                              &control) &&
         trace.retired == 3 && !trace.events && !control.boundary &&
         !control.retire && !control.interrupt_step && g_eflags == 0x203);
  assert(!gamepkg_x86_cpu_snapshot(thread, &state) && !state.sti_shadow);
  for (unsigned bad = 7; bad <= 8; ++bad) {
    privileged_prepare(thread, &state, 0x203, 0, 0, 0);
    trace = (ControlTrace){.cpu = thread, .bad_action = bad};
    control = (GamePkgX86DispatchControl){&trace, trace_boundary, trace_retire,
                                          trace_cpu_step};
    assert(gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                               0x12040, &out, source,
                                               &control) == 3 &&
           !trace.retired && !trace.events && g_eflags == 0x203);
    assert(!gamepkg_x86_cpu_snapshot(thread, &state) && state.flags == 0x203);
  }
  privileged_prepare(thread, &state, 2, 0, 0, 0);
  trace = (ControlTrace){.cpu = thread, .bad_action = 2};
  control = (GamePkgX86DispatchControl){&trace, trace_boundary, trace_retire,
                                        trace_cpu_step};
  assert(gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                             0x12050, &out, source,
                                             &control) == 3 &&
         trace.retired == 1 && g_eflags == 0x202);
  assert(!gamepkg_x86_cpu_snapshot(thread, &state) && state.sti_shadow == 1 &&
         state.flags == 0x202);
  /* Preserve every unrelated status/IOPL/DF bit through the exact CLI/STI
   * path; virtual direction never becomes the native host's DF. */
  privileged_prepare(thread, &state, 0xed7, 0, 0, 0);
  g_df = 1;
  event = (CpuEventTrace){.thread = thread};
  assert(!gamepkg_x86_dispatch_run_cpu_events(controlled_map, controlled_count,
                                              0x12080, &out, source, thread,
                                              &events) &&
         g_eflags == 0xed7 && g_df == 1);
  /* Queue a real second native worker while translated CLI holds the CPU.
   * It may enter only after STI and its following INC retire. Polling the
   * pending source precedes that queued-worker handoff. */
  privileged_prepare(thread, &state, 0x203, 0, 0, 0);
  g_esi = 0;
  serial_cpu = cpu;
  GamePkgX86InterruptState visitor_state = {0x202, 1, 0, 0, 0, 0};
  serial_visitor = gamepkg_x86_cpu_thread_create(cpu, &visitor_state);
  assert(serial_visitor);
  atomic_store(&serial_phase, 0);
  atomic_store(&serial_entered, 0);
  void (*old_stop)(uint32_t) = controlled_map[0].block;
  controlled_map[0].block = enqueue_visitor;
  const GamePkgX86CpuEventSource serial_events = {NULL, serial_poll};
  assert(!gamepkg_x86_dispatch_run_cpu_events(controlled_map, controlled_count,
                                              0x12090, &out, source, thread,
                                              &serial_events));
  assert(atomic_load(&serial_phase) == 2 && atomic_load(&serial_entered) == 1 &&
         g_ebx == 1 && g_edx == 1 && g_esi == 1 && g_eflags == 0x203);
#ifdef _WIN32
  assert(WaitForSingleObject(serial_handle, INFINITE) == WAIT_OBJECT_0 &&
         CloseHandle(serial_handle));
#else
  assert(!pthread_join(serial_handle, NULL));
#endif
  assert(!gamepkg_x86_cpu_thread_destroy(serial_visitor));
  controlled_map[0].block = old_stop;
  serial_visitor = NULL;
  serial_cpu = NULL;
  assert(!memcmp(&raw_fp, &x87, sizeof(x87)) &&
         !memcmp(raw_xmm, xmm, sizeof(raw_xmm)));
  x87 = old_fp;
  state.sti_shadow = 0;
  assert(!gamepkg_x86_cpu_restore(thread, &state));
  assert(!gamepkg_x86_cpu_block(thread) &&
         !gamepkg_x86_cpu_thread_destroy(thread));
  gamepkg_x86_cpu_release(cpu);
  puts("Generated CLI/STI: CPU provider, IF/VIF/VIP/GP, "
       "STI-CLI/STI-STI/STI-REP/STI-fault ordering, pending request exclusion, "
       "frozen provider and raw FP preservation passed; no native privileged "
       "opcodes or fabricated IRQ handler");
}
