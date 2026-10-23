/* Included in the synthetic controlled-dispatch fixture. Requests remain host
 * pending notifications; no vector, IRQ acknowledgement or handler is faked. */
typedef struct CpuEventTrace {
  GamePkgX86CpuThread *thread;
  unsigned polls, requests, pending, stop_kind, error;
  uint32_t stop_pc;
  GamePkgX86CpuEventSource *mutate;
  GamePkgX86InterruptState image;
  GamePkgX86FpFault snapshot;
  GamePkgX86IrqFlags flags;
} CpuEventTrace;
static int poll_event(void *opaque, const GamePkgX86InterruptState *state,
                      uint32_t kind, uint32_t pc, uint32_t bp) {
  CpuEventTrace *trace = opaque;
  assert(!gamepkg_x86_cpu_check_owner(trace->thread) &&
         gamepkg_x86_interrupt_eligible(state) == 1 &&
         (int)kind == gamepkg_x86_dispatch_checkpoint_kind());
  ++trace->polls;
  if (trace->mutate)
    trace->mutate->poll = NULL;
  if (trace->error)
    return (int)trace->error;
  if (!trace->pending || (trace->stop_pc && trace->stop_pc != pc) ||
      kind != trace->stop_kind)
    return 0;
  trace->image = *state;
  assert(!gamepkg_x86_dispatch_capture_checkpoint(&trace->snapshot));
  assert(trace->snapshot.core.eip == pc && trace->snapshot.core.ebp == bp &&
         trace->snapshot.core.eflags == state->flags &&
         !memcmp(&trace->snapshot.fp.x87, &x87, sizeof(x87)) &&
         !memcmp(trace->snapshot.fp.xmm, xmm, sizeof(xmm)));
  assert(gamepkg_x86_interrupt_plan_irq(
             state, kind == GAMEPKG_X86_CHECKPOINT_REP, 0, &trace->flags) == 1);
  ++trace->requests;
  return 1;
}
static void prepare_compare(void) {
  reset();
  g_ecx = 4;
  g_esi = 0x100100;
  g_edi = 0x100200;
  g_eflags = 0x203;
}
static void cpu_event_cases(const GamePkgX86FpSource *source) {
  prepare_compare();
  GamePkgX86Cpu *cpu = gamepkg_x86_cpu_create();
  assert(cpu);
  GamePkgX86InterruptState state = {g_eflags, 1, 0, 0, 0, 0};
  GamePkgX86CpuThread *thread = gamepkg_x86_cpu_thread_create(cpu, &state);
  assert(thread);
  CpuEventTrace trace = {.thread = thread,
                         .pending = 1,
                         .stop_kind = GAMEPKG_X86_CHECKPOINT_REP,
                         .stop_pc = 0x11e10};
  GamePkgX86CpuEventSource events = {&trace, poll_event};
  GamePkgX86DispatchExit out, before;
  memset(&out, 0xa5, sizeof(out));
  before = out;
  assert(gamepkg_x86_dispatch_run_cpu_events(controlled_map, controlled_count,
                                             0x11e10, &out, source, thread,
                                             &events) == -1 &&
         !memcmp(&out, &before, sizeof(out)) && !trace.polls);
  assert(!gamepkg_x86_cpu_enter(thread));
  GamePkgX87State old_fp = x87;
  x87.control = 0x37e;
  x87.status = 0x81; /* Raw pending exception: capture must not WAIT. */
  GamePkgX87State pending_fp = x87;
  assert(gamepkg_x86_dispatch_run_cpu_events(controlled_map, controlled_count,
                                             0x11e10, &out, source, thread,
                                             &events) == 4 &&
         out.pc == 0x11e10);
  assert(trace.polls == 2 && trace.requests == 1 && trace.pending == 1 &&
         trace.snapshot.core.ecx == 3 && trace.snapshot.core.eflags == 0x246 &&
         trace.flags.saved_flags == 0x10246 &&
         trace.flags.handler_flags == 0x46 &&
         !memcmp(&pending_fp, &x87, sizeof(x87)));
  GamePkgX86FpFault zero = {0};
  assert(!memcmp(&out.fault, &zero, sizeof(zero)));
  assert(!gamepkg_x86_cpu_check_owner(thread));
  trace.pending = 0;
  assert(!gamepkg_x86_dispatch_run_cpu_events(controlled_map, controlled_count,
                                              out.pc, &out, source, thread,
                                              &events) &&
         !g_ecx && trace.polls == 6 && trace.requests == 1);
  x87 = old_fp;
  /* IF0 never polls; a preceding STI shadow covers the ENTIRE repeated
   * instruction and only permits the pending notification before RET. */
  for (unsigned shadow = 0; shadow < 2; ++shadow) {
    prepare_compare();
    if (!shadow)
      g_eflags = 2;
    state = (GamePkgX86InterruptState){g_eflags, 1, 0, 0, 0, shadow};
    assert(!gamepkg_x86_cpu_restore(thread, &state));
    trace = (CpuEventTrace){.thread = thread,
                            .pending = 1,
                            .stop_kind = GAMEPKG_X86_CHECKPOINT_INSTRUCTION};
    int result = gamepkg_x86_dispatch_run_cpu_events(
        controlled_map, controlled_count, 0x11e10, &out, source, thread,
        &events);
    if (shadow) {
      assert(result == 4 && out.pc == 0x11e12 && trace.polls == 1 &&
             trace.requests == 1 && !trace.snapshot.core.ecx &&
             trace.flags.saved_flags == 0x246 &&
             trace.flags.handler_flags == 0x46 && g_esp == 0x1fff00);
    } else
      assert(!result && !trace.polls && !trace.requests && !g_ecx);
    assert(!gamepkg_x86_cpu_snapshot(thread, &state) && !state.sti_shadow);
  }
  prepare_compare();
  state.flags = g_eflags;
  assert(!gamepkg_x86_cpu_restore(thread, &state));
  trace = (CpuEventTrace){.thread = thread, .mutate = &events};
  assert(!gamepkg_x86_dispatch_run_cpu_events(controlled_map, controlled_count,
                                              0x11e10, &out, source, thread,
                                              &events) &&
         trace.polls == 5 && !events.poll);
  events.poll = poll_event;
  prepare_compare();
  state.flags = g_eflags;
  assert(!gamepkg_x86_cpu_restore(thread, &state));
  trace = (CpuEventTrace){.thread = thread, .pending = 1, .error = 7};
  assert(gamepkg_x86_dispatch_run_cpu_events(controlled_map, controlled_count,
                                             0x11e10, &out, source, thread,
                                             &events) == 3 &&
         trace.polls == 1 && !trace.requests && g_ecx == 4);
  before = out;
  events.poll = NULL;
  assert(gamepkg_x86_dispatch_run_cpu_events(controlled_map, controlled_count,
                                             0x11e10, &out, source, thread,
                                             &events) == -1 &&
         !memcmp(&out, &before, sizeof(out)));
  union {
    GamePkgX86CpuEventSource source;
    GamePkgX86DispatchExit result;
  } alias, old_alias;
  memset(&alias, 0xa5, sizeof(alias));
  alias.source = (GamePkgX86CpuEventSource){&trace, poll_event};
  memcpy(&old_alias, &alias, sizeof(alias));
  assert(gamepkg_x86_dispatch_run_cpu_events(controlled_map, controlled_count,
                                             0x11e10, &alias.result, source,
                                             thread, &alias.source) == -1 &&
         !memcmp(&alias, &old_alias, sizeof(alias)));
  assert(!gamepkg_x86_cpu_block(thread) &&
         !gamepkg_x86_cpu_thread_destroy(thread));
  gamepkg_x86_cpu_release(cpu);
  puts(
      "CPU event checkpoints: eligible REP request stop/resume, saved RF plan, "
      "raw pending x87/MMX/XMM capture, IF0/shadow exclusion, frozen sources "
      "and invalid descriptor guards passed; IRQ delivery remains separate");
}

/* PRIVILEGED_CASES */
