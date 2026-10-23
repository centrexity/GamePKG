/* Maintainer-only comparison of the recovered native C/Rust bodies. No media. */
#include "recomp_types.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
ptrdiff_t g_xbox_mem_offset;
uint32_t g_xbox_code_lo, g_xbox_code_hi;
volatile uint32_t g_icall_trace[ICALL_TRACE_SIZE], g_icall_trace_idx;
volatile uint64_t g_icall_count;
RECOMP_TLS uint32_t g_eax, g_ecx, g_edx, g_esp, g_ebx, g_esi, g_edi;
RECOMP_TLS uint32_t g_ebp, g_seh_ebp, g_fs_base, g_eflags;
RECOMP_TLS int g_df, g_fp_top, g_fp_cmp;
RECOMP_TLS uint16_t g_fp_cc, g_fp_control_word;
RECOMP_TLS double g_fp_stack[8];
RECOMP_TLS RecompXmm g_xmm0;
static uint32_t choice, calls, arguments[6];
static void selected(void) {
    assert(g_ecx == 0x57BCFC);
    assert(MEM32(g_esp) == 0x966FD);
    g_eax = choice;
    g_esp += 4;
}
recomp_func_t recomp_lookup(uint32_t va) { assert(va == 0x12340); return selected; }
recomp_func_t recomp_lookup_kernel(uint32_t va) { (void)va; abort(); }
recomp_func_t recomp_lookup_manual(uint32_t va) { (void)va; return NULL; }
void recomp_icall_fail_log(uint32_t va) { (void)va; abort(); }
void recomp_icall_not_code_log(uint32_t va) { (void)va; abort(); }
void sub_00141010(void) {
    assert(MEM32(g_esp) == 0x967C5);
    for (unsigned i = 0; i < 6; i++) arguments[i] = MEM32(g_esp + 4 + i * 4);
    calls++;
    g_esp += 28;
}
void sub_000966F0(void), sub_00249AB6(void);
void c_sub_000966F0(void), c_sub_00249AB6(void);
void *gamepkg_tls_g_eax(void) { return &g_eax; }
void *gamepkg_tls_g_ecx(void) { return &g_ecx; }
void *gamepkg_tls_g_edx(void) { return &g_edx; }
void *gamepkg_tls_g_esp(void) { return &g_esp; }
void *gamepkg_tls_g_fp_stack(void) { return &g_fp_stack; }
void *gamepkg_tls_g_fp_top(void) { return &g_fp_top; }
void *gamepkg_tls_g_xmm0(void) { return &g_xmm0; }
struct result {
    uint32_t registers[4], state[4], call_count, args[6], trace[3];
    uint8_t flags[2];
    RecompXmm vector;
};
static struct result run(void (*fn)(void), uint32_t value, unsigned flag, int8_t mode) {
    memset((void *)g_xbox_mem_offset, 0xA5, 0x800000);
    choice = value; calls = 0; memset(arguments, 0, sizeof arguments);
    g_eax = g_ecx = g_edx = 0;
    g_esp = 0x80000;
    memset(&g_xmm0, 0xA5, sizeof g_xmm0);
    g_icall_trace_idx = 0; g_icall_count = 0;
    MEM32(0x57BCFC) = 0x600000; MEM32(0x60000C) = 0x12340;
    MEM8(0x411571) = flag >> 1; MEM8(0x411560) = flag & 1;
    MEM8(0x4AED44) = mode; MEMF(0x557838) = 12.5f;
    fn();
    assert(g_esp == 0x80004);
    assert(g_icall_trace_idx == 1 && g_icall_count == 1 && g_icall_trace[0] == 0x12340);
    int changes = value == 1 || value == 2;
    assert(calls == (unsigned)(changes && flag < 2));
    if (changes) {
        assert(MEM32(0x557A64) == 3);
        assert(MEM32(0x557A68) == (value == 1 ? 0x15u : 1u));
        assert(MEMF(0x557A5C) == 12.5f && MEM8(0x55927F) == 0);
        if (value == 2) assert(MEM32(0x557A6C) == (uint32_t)(int32_t)mode);
    }
    if (calls) {
        assert(arguments[0] == 0xA9E00000 && arguments[1] == 0xB701A7E2);
        assert(arguments[2] == 0x3ECCCCCD && arguments[3] == 0x3F800000);
        assert(arguments[4] == (flag & 1) && arguments[5] == 0);
    }
    struct result out = {0};
    out.registers[0] = g_eax; out.registers[1] = g_ecx;
    out.registers[2] = g_edx; out.registers[3] = g_esp;
    for (unsigned i = 0; i < 4; i++) out.state[i] = MEM32(0x557A60 + i * 4);
    out.call_count = calls; memcpy(out.args, arguments, sizeof arguments);
    out.trace[0] = g_icall_trace[0]; out.trace[1] = g_icall_trace_idx; out.trace[2] = g_icall_count;
    out.flags[0] = MEM8(0x411571); out.flags[1] = MEM8(0x55927F); out.vector = g_xmm0;
    return out;
}
int main(void) {
    g_xbox_mem_offset = (ptrdiff_t)malloc(0x800000);
    assert(g_xbox_mem_offset);
    const uint32_t values[] = {0, 1, 2, 3, 4, UINT32_MAX};
    const int8_t modes[] = {-128, -1, 0, 1, 127};
    unsigned count = 0;
    for (unsigned v = 0; v < sizeof values / sizeof *values; v++)
        for (unsigned f = 0; f < 4; f++)
            for (unsigned m = 0; m < sizeof modes / sizeof *modes; m++) {
                struct result a = run(c_sub_000966F0, values[v], f, modes[m]);
                struct result b = run(sub_000966F0, values[v], f, modes[m]);
                assert(memcmp(&a, &b, sizeof a) == 0); count++;
            }
    for (int top = 0; top < 8; top++) {
        double expected[8];
        for (int variant = 0; variant < 2; variant++) {
            for (int i = 0; i < 8; i++) g_fp_stack[i] = i + 1.25;
            g_fp_top = top; g_esp = 0x80000;
            (variant ? sub_00249AB6 : c_sub_00249AB6)();
            assert(g_fp_top == top && g_esp == 0x80004 && g_fp_stack[top] == 0);
            if (!variant) memcpy(expected, g_fp_stack, sizeof expected);
            else assert(memcmp(expected, g_fp_stack, sizeof expected) == 0);
        }
        count++;
    }
    free((void *)g_xbox_mem_offset);
    printf("PASS: %u recovered native C/Rust cases, including state branches, signed mode, callbacks, arguments, stack balance and every floating-point stack position.\n", count);
}
