/* Included after ordinary dispatcher helpers; all instruction bytes synthetic.
 */
static GamePkgX86DispatchEntry controlled_map[256];
static size_t controlled_count;
typedef struct ControlTrace {
  uint32_t before[256], after[256], bp, event_pc, stop;
  unsigned boundaries, retired, events;
  uint32_t kinds[256];
  unsigned capture, snapshots, nested_scopes;
  GamePkgX86FpFault snapshot;
  GamePkgX86FpFault *bad_output;
  GamePkgX86FpSource *mutate_fp;
  unsigned bad_action;
  unsigned stop_n, matches, shadow_checks;
  uint32_t shadow_pc;
  GamePkgX86CpuThread *cpu;
  GamePkgX86DispatchControl *mutate;
} ControlTrace;
static void scope_stop(uint32_t pc) {
  (void)pc;
  GamePkgX86FpFault context, before;
  memset(&context, 0xa5, sizeof(context));
  before = context;
  assert(gamepkg_x86_dispatch_checkpoint_kind() == -1 &&
         gamepkg_x86_dispatch_capture_checkpoint(&context) == -1 &&
         !memcmp(&context, &before, sizeof(context)));
  gamepkg_x86_dispatch_stop(g_ebp);
}
static int trace_boundary(void *opaque, uint32_t pc, uint32_t bp) {
  ControlTrace *t = opaque;
  assert(t->boundaries < 256);
  int kind = gamepkg_x86_dispatch_checkpoint_kind();
  assert(kind == GAMEPKG_X86_CHECKPOINT_INSTRUCTION ||
         kind == GAMEPKG_X86_CHECKPOINT_REP);
  t->kinds[t->boundaries] = (uint32_t)kind;
  t->before[t->boundaries++] = pc;
  if (t->mutate_fp)
    memset(t->mutate_fp, 0, sizeof(*t->mutate_fp));
  if (t->capture) {
    assert(!gamepkg_x86_dispatch_capture_checkpoint(&t->snapshot));
    assert(t->snapshot.core.eip == pc && t->snapshot.core.ebp == bp &&
           t->snapshot.core.ecx == g_ecx && t->snapshot.core.esi == g_esi &&
           t->snapshot.core.edi == g_edi &&
           t->snapshot.core.eflags == g_eflags && !t->snapshot.core.vector &&
           !t->snapshot.core.error_code && !t->snapshot.core.fault_address);
    assert(!memcmp(&t->snapshot.fp.x87, &x87, sizeof(x87)) &&
           !memcmp(t->snapshot.fp.xmm, xmm, sizeof(xmm)));
    ++t->snapshots;
  }
  if (t->bad_output)
    assert(gamepkg_x86_dispatch_capture_checkpoint(t->bad_output) == -1);
  if (t->bad_action == 3)
    assert(gamepkg_x86_dispatch_retire(bp) == 1);
  if (t->bad_action == 4)
    assert(gamepkg_x86_dispatch_rep_checkpoint(pc, bp) == 1);
  if (t->bad_action == 5)
    gamepkg_x86_general_protection(pc, bp);
  if (t->bad_action == 6) {
    const GamePkgX86DispatchEntry inner = {0x2300, scope_stop};
    GamePkgX86DispatchExit out;
    assert(!gamepkg_x86_dispatch_run(&inner, 1, 0x2300, &out, NULL));
    assert(gamepkg_x86_dispatch_checkpoint_kind() == kind);
    ++t->nested_scopes;
  }
  t->bp = bp;
  if (t->cpu) {
    GamePkgX86InterruptState image;
    assert(!gamepkg_x86_cpu_checkpoint(t->cpu, g_eflags) &&
           !gamepkg_x86_cpu_snapshot(t->cpu, &image));
    assert(image.flags == g_eflags && image.sti_shadow == (pc == t->shadow_pc));
    if (pc == t->shadow_pc)
      ++t->shadow_checks;
  }
  if (t->bad_action == 1)
    gamepkg_x86_dispatch_stop(bp);
  if (t->mutate) {
    t->mutate->boundary = NULL;
    t->mutate->retire = NULL;
  }
  if (pc != t->stop)
    return 0;
  ++t->matches;
  return !t->stop_n || t->matches == t->stop_n;
}
static int trace_retire(void *opaque, uint32_t pc, uint32_t bp,
                        uint32_t event) {
  ControlTrace *t = opaque;
  assert(gamepkg_x86_dispatch_checkpoint_kind() == -1);
  GamePkgX86FpFault context, before;
  memset(&context, 0xa5, sizeof(context));
  before = context;
  assert(gamepkg_x86_dispatch_capture_checkpoint(&context) == -1 &&
         !memcmp(&context, &before, sizeof(context)));
  t->bp = bp;
  if (t->cpu && event < 2)
    assert(!gamepkg_x86_cpu_retire_other(t->cpu, g_eflags));
  if (t->bad_action == 2)
    gamepkg_x86_dispatch_stop(bp);
  assert(event <= 3);
  if (event == 1) {
    ++t->events;
    t->event_pc = pc;
  } else {
    assert(t->retired < 256);
    t->after[t->retired++] = pc;
  }
  return 0;
}
/* CPU_EVENT_CASES */
static void controlled_cases(void) {
  GamePkgX86FpFault outside, outside_before;
  memset(&outside, 0xa5, sizeof(outside));
  outside_before = outside;
  assert(gamepkg_x86_dispatch_checkpoint_kind() == -1 &&
         gamepkg_x86_dispatch_capture_checkpoint(&outside) == -1 &&
         !memcmp(&outside, &outside_before, sizeof(outside)));
  controlled_map[0] = (GamePkgX86DispatchEntry){0x2000, stop};
  controlled_count = 1;
  assert(gamepkg_x86_cpu_generated_count +
             sizeof(privileged_entries) / sizeof(privileged_entries[0]) + 2 <
         256);
  for (size_t i = 0; i < gamepkg_x86_cpu_generated_count; ++i)
    controlled_map[controlled_count++] = gamepkg_x86_cpu_generated_entries[i];
  for (size_t i = 0;
       i < sizeof(privileged_entries) / sizeof(privileged_entries[0]); ++i)
    controlled_map[controlled_count++] = privileged_entries[i];
  controlled_map[controlled_count++] =
      (GamePkgX86DispatchEntry){0xf0000000, stop};
  reset();
  GamePkgX86Memory *memory = bind_ram(1048576, 3);
  GamePkgX86DispatchExit out;
  ControlTrace trace = {0};
  GamePkgX86DispatchControl control = {&trace, trace_boundary, trace_retire,
                                       NULL};
  assert(!gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                              0x11000, &out, NULL, &control));
  const uint32_t call_order[] = {0x11000, 0x11002, 0x11004, 0x11100,
                                 0x11104, 0x11108, 0x11009, 0x1100c};
  assert(trace.boundaries == 8 && trace.retired == 8 && !trace.events &&
         g_eax == 13);
  assert(!memcmp(trace.before, call_order, sizeof(call_order)) &&
         !memcmp(trace.after, call_order, sizeof(call_order)));
  reset();
  trace = (ControlTrace){0};
  g_ecx = 2;
  assert(!gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                              0x10000, &out, NULL, &control));
  const uint32_t branches[] = {0x10000, 0x10001, 0x10003, 0x10000,
                               0x10001, 0x10008, 0x10008};
  assert(trace.retired == 7 && trace.boundaries == 7 && !g_ecx);
  assert(!memcmp(trace.before, branches, sizeof(branches)) &&
         !memcmp(trace.after, branches, sizeof(branches)));
  reset();
  trace = (ControlTrace){0};
  g_eax = 21;
  g_ecx = 0;
  g_ebx = 7;
  g_esi = 9;
  assert(gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                             0x11500, &out, NULL,
                                             &control) == 1);
  assert(out.pc == 0x11501 && trace.boundaries == 2 && trace.retired == 1 &&
         trace.events == 1 && trace.event_pc == 0x11501 && g_ebx == 8 &&
         g_esi == 9);
  g_ecx = 3;
  assert(!gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                              out.pc, &out, NULL, &control));
  assert(trace.boundaries == 5 && trace.retired == 4 && trace.events == 1 &&
         g_eax == 7 && g_ebx == 8 && g_esi == 10);
  reset();
  trace = (ControlTrace){.stop = 0x11503};
  g_eax = 21;
  g_ecx = 3;
  g_ebx = 7;
  g_esi = 9;
  assert(gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                             0x11500, &out, NULL,
                                             &control) == 4);
  assert(out.pc == 0x11503 && trace.retired == 2 && !trace.events &&
         g_eax == 7 && g_ebx == 8 && g_esi == 9);
  GamePkgX86FpFault zero = {0};
  assert(!memcmp(&out.fault, &zero, sizeof(zero)));
  trace.stop = 0;
  assert(!gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                              out.pc, &out, NULL, &control));
  assert(trace.retired == 4 && g_ebx == 8 && g_esi == 10);
  reset();
  trace = (ControlTrace){.stop = 0x11e00, .stop_n = 3};
  g_ecx = 7;
  g_esi = 0x100100;
  g_edi = 0x100200;
  g_ebx = 7;
  for (unsigned i = 0; i < 7; ++i)
    ram[0x100 + i] = (unsigned char)(i + 1);
  assert(gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                             0x11e00, &out, NULL,
                                             &control) == 4 &&
         out.pc == 0x11e00);
  assert(!trace.retired && !trace.events && g_ecx == 5 && g_esi == 0x100102 &&
         g_edi == 0x100202 && g_ebx == 7 && ram[0x200] == 1 &&
         ram[0x201] == 2 && ram[0x202] == 0x5a);
  assert(trace.kinds[0] == GAMEPKG_X86_CHECKPOINT_INSTRUCTION &&
         trace.kinds[1] == GAMEPKG_X86_CHECKPOINT_REP &&
         trace.kinds[2] == GAMEPKG_X86_CHECKPOINT_REP);
  trace.stop = 0;
  assert(!gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                              out.pc, &out, NULL, &control));
  assert(trace.kinds[3] == GAMEPKG_X86_CHECKPOINT_INSTRUCTION);
  assert(trace.retired == 3 && !g_ecx && g_esi == 0x100107 &&
         g_edi == 0x100207 && g_ebx == 8);
  for (unsigned i = 0; i < 7; ++i)
    assert(ram[0x200 + i] == i + 1);
  reset();
  trace = (ControlTrace){.stop = 0x11e10, .stop_n = 2};
  g_ecx = 4;
  g_esi = 0x100100;
  g_edi = 0x100200;
  g_eflags = 0x203;
  assert(
      gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                          0x11e10, &out, NULL, &control) == 4 &&
      out.pc == 0x11e10 && g_ecx == 3 && g_eflags == 0x246 && !trace.retired);
  trace.stop = 0;
  assert(!gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                              out.pc, &out, NULL, &control) &&
         !g_ecx && trace.retired == 2 && g_eflags == 0x246);
  reset();
  trace = (ControlTrace){0};
  assert(gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                             0x11c00, &out, NULL,
                                             &control) == 1);
  assert(out.fault.core.vector == 3 && trace.retired == 1 &&
         trace.events == 1 && trace.event_pc == 0x11c02 &&
         trace.before[1] == 0x11c01);
  reset();
  trace = (ControlTrace){.mutate = &control};
  assert(!gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                              0x11310, &out, NULL, &control));
  assert(trace.retired == 2 && trace.boundaries == 2 && g_eax == 0x1234 &&
         !control.boundary && !control.retire);
  reset();
  g_eax = 21;
  assert(gamepkg_x86_dispatch_run(controlled_map, controlled_count, 0x11310,
                                  &out, NULL) == 3 &&
         g_eax == 21); /* No missing-provider execution. */
  GamePkgX86DispatchExit before = out;
  assert(gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                             0x11310, &out, NULL,
                                             &control) == -1 &&
         !memcmp(&before, &out, sizeof(out)));
  assert(gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                             0x11310, &out, NULL, NULL) == -1);
  reset();
  trace = (ControlTrace){0};
  control =
      (GamePkgX86DispatchControl){&trace, trace_boundary, trace_retire, NULL};
  trace.bad_action = 1;
  g_eax = 21;
  assert(gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                             0x11310, &out, NULL,
                                             &control) == 3 &&
         g_eax == 21);
  reset();
  trace = (ControlTrace){.bad_action = 2};
  assert(gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                             0x11310, &out, NULL,
                                             &control) == 3 &&
         g_eax == 0x1234 && g_esp == 0x1fff00);
  reset();
  trace = (ControlTrace){.bad_action = 2};
  g_eax = 21;
  g_ecx = 0;
  assert(gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                             0x11501, &out, NULL,
                                             &control) == 3 &&
         out.fault.core.eip == 0x11501);
  reset();
  trace = (ControlTrace){0};
  g_eax = 0x100080;
  GamePkgX86Memory *hole = bind_ram(0x80, 0);
  assert(gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                             0x11900, &out, NULL,
                                             &control) == 1);
  assert(out.pc == 0x11905 && trace.retired == 1 && trace.events == 1 &&
         trace.bp == 0x12345678 && out.fault.core.ebp == 0x12345678);
  unbind_ram(hole);
  gamepkg_x86_memory_bind(memory);
  /* Rejected output/descriptor aliases preserve every byte before execution. */
  union {
    GamePkgX86DispatchExit output;
    GamePkgX86DispatchControl control;
  } alias, old_alias;
  memset(&alias, 0xa5, sizeof(alias));
  alias.control = control;
  memcpy(&old_alias, &alias, sizeof(alias));
  assert(gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                             0x11310, &alias.output, NULL,
                                             &alias.control) == -1 &&
         !memcmp(&alias, &old_alias, sizeof(alias)));
  reset();
  /* Real shared CPU adapter: old shadow ends at successful retirement and
   * fault delivery; raw flags stay synchronized and CPU remains reserved. */
  GamePkgX86Cpu *cpu = gamepkg_x86_cpu_create();
  GamePkgX86InterruptState state = {g_eflags, 1, 0, 0, 0, 1};
  GamePkgX86CpuThread *thread = gamepkg_x86_cpu_thread_create(cpu, &state);
  assert(cpu && thread);
  assert(gamepkg_x86_dispatch_run_cpu(controlled_map, controlled_count, 0x11500,
                                      &out, NULL, thread) == -1);
  assert(!gamepkg_x86_cpu_enter(thread));
  GamePkgX86InterruptState mismatch = state;
  mismatch.flags ^= 1;
  assert(!gamepkg_x86_cpu_restore(thread, &mismatch));
  before = out;
  assert(gamepkg_x86_dispatch_run_cpu(controlled_map, controlled_count, 0x11500,
                                      &out, NULL, thread) == -1 &&
         !memcmp(&before, &out, sizeof(out)));
  assert(!gamepkg_x86_cpu_restore(thread, &state));
  g_eax = 21;
  g_ecx = 0;
  g_ebx = 7;
  memset(&x87, 0, sizeof(x87));
  GamePkgX86FpSource fp_source = {.x87 = &x87};
  for (unsigned i = 0; i < 8; ++i) {
    x87.significand[i] = UINT64_C(0x7fff800000001234) ^ i;
    x87.sign_exponent[i] = (uint16_t)(0x7fff + i);
    fp_source.xmm[i] = xmm[i];
    for (unsigned j = 0; j < 4; ++j)
      xmm[i][j] = 0x7f800001u ^ (i << 4) ^ j;
  }
  x87.tags = 0xaaaa;
  GamePkgX87State original_fp = x87;
  assert(gamepkg_x86_dispatch_run_cpu(controlled_map, controlled_count, 0x11501,
                                      &out, &fp_source, thread) == 1 &&
         out.pc == 0x11501 && g_ebx == 7);
  assert(!memcmp(&out.fault.fp.x87, &original_fp, sizeof(original_fp)) &&
         !memcmp(out.fault.fp.xmm, xmm, sizeof(xmm)));
  assert(!gamepkg_x86_cpu_snapshot(thread, &state) && !state.sti_shadow);
  state.sti_shadow = 1;
  assert(!gamepkg_x86_cpu_restore(thread, &state));
  assert(gamepkg_x86_dispatch_run_cpu(controlled_map, controlled_count, 0x11500,
                                      &out, NULL, thread) == 1 &&
         out.pc == 0x11501);
  assert(!gamepkg_x86_cpu_snapshot(thread, &state) && !state.sti_shadow &&
         state.flags == g_eflags && !gamepkg_x86_cpu_check_owner(thread));
  g_ecx = 3;
  assert(!gamepkg_x86_dispatch_run_cpu(controlled_map, controlled_count, out.pc,
                                       &out, NULL, thread));
  assert(!gamepkg_x86_cpu_snapshot(thread, &state) && state.flags == g_eflags);
  reset();
  g_eflags = 0x203;
  g_ecx = 7;
  g_esi = 0x100100;
  g_edi = 0x100200;
  state.flags = g_eflags;
  state.sti_shadow = 1;
  assert(!gamepkg_x86_cpu_restore(thread, &state));
  trace = (ControlTrace){.cpu = thread, .shadow_pc = 0x11e10};
  control =
      (GamePkgX86DispatchControl){&trace, trace_boundary, trace_retire, NULL};
  assert(!gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                              0x11e10, &out, NULL, &control) &&
         trace.shadow_checks == 7 && trace.retired == 2);
  assert(!gamepkg_x86_cpu_snapshot(thread, &state) && state.flags == 0x246 &&
         !state.sti_shadow);
  assert(!gamepkg_x86_cpu_block(thread));
  assert(!gamepkg_x86_cpu_thread_destroy(thread));
  gamepkg_x86_cpu_release(cpu);
  /* Snapshot scope, nested dispatch and protected host output aliases. */
  reset();
  g_eflags = 0x203;
  g_ecx = 4;
  g_esi = 0x100100;
  g_edi = 0x100200;
  trace = (ControlTrace){
      .capture = 1, .stop = 0x11e10, .stop_n = 2, .bad_action = 6};
  control =
      (GamePkgX86DispatchControl){&trace, trace_boundary, trace_retire, NULL};
  assert(gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                             0x11e10, &out, &fp_source,
                                             &control) == 4);
  assert(trace.snapshots == 2 && trace.nested_scopes == 2 &&
         trace.kinds[0] == 0 && trace.kinds[1] == 1 &&
         trace.snapshot.core.ecx == 3 && trace.snapshot.core.eflags == 0x246 &&
         !trace.retired);
  assert(gamepkg_x86_dispatch_checkpoint_kind() == -1);
  GamePkgX86FpSource frozen_source = fp_source;
  fp_source = frozen_source;
  prepare_compare();
  trace = (ControlTrace){
      .capture = 1, .stop = 0x11e10, .stop_n = 2, .mutate_fp = &fp_source};
  assert(gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                             0x11e10, &out, &fp_source,
                                             &control) == 4 &&
         trace.snapshots == 2 && !fp_source.x87 &&
         trace.snapshot.core.ecx == 3);
  fp_source = frozen_source;
  GamePkgX86FpFault captured = trace.snapshot;
  assert(gamepkg_x86_dispatch_capture_checkpoint(&trace.snapshot) == -1 &&
         !memcmp(&captured, &trace.snapshot, sizeof(captured)));
  GamePkgX86DispatchEntry old_map[256];
  memcpy(old_map, controlled_map, sizeof(old_map));
  for (unsigned alias = 0; alias < 5; ++alias) {
    reset();
    trace = (ControlTrace){.stop = 0x11310};
    trace.bad_output = alias == 0   ? (GamePkgX86FpFault *)controlled_map
                       : alias == 1 ? (GamePkgX86FpFault *)&out
                       : alias == 2 ? (GamePkgX86FpFault *)&control
                       : alias == 3 ? (GamePkgX86FpFault *)&fp_source
                                    : (GamePkgX86FpFault *)&x87;
    GamePkgX86FpSource old_source = fp_source;
    GamePkgX86DispatchControl old_control = control;
    GamePkgX87State old_fp = x87;
    assert(gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                               0x11310, &out, &fp_source,
                                               &control) == 4 &&
           trace.boundaries == 1 && !trace.retired);
    assert(!memcmp(old_map, controlled_map, sizeof(old_map)) &&
           !memcmp(&old_source, &fp_source, sizeof(fp_source)) &&
           !memcmp(&old_control, &control, sizeof(control)) &&
           !memcmp(&old_fp, &x87, sizeof(x87)));
  }
  for (unsigned bad = 3; bad <= 5; ++bad) {
    reset();
    trace = (ControlTrace){.bad_action = bad};
    assert(gamepkg_x86_dispatch_run_controlled(controlled_map, controlled_count,
                                               0x11310, &out, &fp_source,
                                               &control) == 3 &&
           !g_eax && !trace.retired && !trace.events);
    assert(gamepkg_x86_dispatch_checkpoint_kind() == -1);
    if (bad == 5)
      assert(out.fault.core.vector == 13 && out.pc == 0x11310);
  }
  cpu_event_cases(&fp_source);
  privileged_cases(&fp_source);
  unbind_ram(memory);
  puts("Controlled dispatch: instruction ordering, CALL/RET/branches, "
       "fault/event "
       "retirement, boundary stop/resume, frozen callbacks and CPU ownership "
       "passed");
}
