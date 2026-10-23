/* Included after ordinary string helpers. Exact prefix CPU replay is separate;
 * these cases target continued checkpoints followed by a real RAM fault. */
static _Thread_local unsigned checkpoint_goal, checkpoint_seen, checkpoint_form;
static _Thread_local int checkpoint_stopped;
static int checkpoint_boundary(uint32_t pc, uint32_t bp) {
  assert(pc == 0x10000 && bp == 0xabcdef12);
  ++checkpoint_seen;
  return checkpoint_goal && checkpoint_seen == checkpoint_goal;
}
static void checkpoint_invoke(void) {
  const unsigned *f = forms[checkpoint_form];
  checkpoint_stopped = gamepkg_x86_string_controlled(
      f[0], f[1], f[2], f[3], f[4], 0x10000, 0xabcdef12, checkpoint_boundary);
}
static void checkpoint_fault_cases(void) {
  for (unsigned form = 0; form < sizeof(forms) / sizeof(*forms); ++form) {
    unsigned op = forms[form][0], width = forms[form][1], a16 = forms[form][2],
             rep = forms[form][3], fs = forms[form][4];
    if (op < 3 || !rep)
      continue;
    for (unsigned df = 0; df < 2; ++df) {
      initialize(a16, df, 7);
      uint32_t original = g_eflags;
      for (unsigned i = 0; i < sizeof(memory); ++i)
        memory[i] = (unsigned char)(i * 17);
      for (unsigned i = 0; i < 7; ++i) {
        uint32_t src = (progress(g_esi, width, i, a16, df) &
                        (a16 ? 65535u : UINT32_MAX)) +
                       (fs ? 0x2000u : 0),
                 dst = progress(g_edi, width, i, a16, df) &
                       (a16 ? 65535u : UINT32_MAX),
                 value = op == 3 ? 0x11223344u + i * 0x01010101u : g_eax;
        for (unsigned b = 0; b < width; ++b) {
          if (op == 3)
            memory[src + b] = (unsigned char)(value >> (8 * b));
          memory[dst + b] =
              (unsigned char)((value ^ (rep == 2 ? 1u : 0u)) >> (8 * b));
        }
      }
      uint32_t hole =
          progress(g_edi, width, 3, a16, df) & (a16 ? 65535u : UINT32_MAX);
      GamePkgX86Region regions[] = {
          {0, hole, 3, memory},
          {hole + 1, sizeof(memory) - hole - 1, 3, memory + hole + 1}};
      GamePkgX86Memory *map = gamepkg_x86_memory_create(regions, 2, 1);
      assert(map);
      gamepkg_x86_memory_bind(map);
      checkpoint_form = form;
      checkpoint_goal = checkpoint_seen = 0;
      checkpoint_stopped = -1;
      GamePkgX86Fault fault;
      assert(gamepkg_x86_run(checkpoint_invoke, &fault) == 1 &&
             checkpoint_seen == 3 && fault.vector == 14 &&
             fault.fault_address == hole && fault.error_code == 4 &&
             fault.eflags == original && g_eflags == original);
      initialize(a16, df, 7);
      checkpoint_goal = 1;
      checkpoint_seen = 0;
      assert(!gamepkg_x86_run(checkpoint_invoke, &fault) &&
             checkpoint_stopped == 1 && checkpoint_seen == 1);
      uint32_t resumed_flags = g_eflags;
      assert(resumed_flags != original);
      checkpoint_goal = checkpoint_seen = 0;
      assert(gamepkg_x86_run(checkpoint_invoke, &fault) == 1 &&
             checkpoint_seen == 2 && fault.vector == 14 &&
             fault.fault_address == hole && fault.eflags == resumed_flags &&
             g_eflags == resumed_flags);
      gamepkg_x86_memory_bind(NULL);
      gamepkg_x86_memory_release(map);
    }
  }
  gamepkg_x86_memory_bind(NULL);
  checkpoint_form = 0;
  for (unsigned form = 0; form < sizeof(forms) / sizeof(*forms); ++form)
    if (forms[form][3]) {
      initialize(forms[form][2], 0, 0);
      checkpoint_form = form;
      checkpoint_goal = 1;
      checkpoint_seen = 0;
      g_esi = g_edi = UINT32_MAX;
      GamePkgX86Fault fault;
      assert(!gamepkg_x86_run(checkpoint_invoke, &fault) &&
             !checkpoint_stopped && !checkpoint_seen);
    }
  puts("REP checkpoints: continued comparison faults restore entry flags; "
       "resumed faults "
       "restore checkpoint flags; DF/width/address/FS variants and zero counts "
       "passed");
}
