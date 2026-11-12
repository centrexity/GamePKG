"""Exact pinned host interrupt-gate patch; no decoder or game media required."""
import hashlib

KERNEL_SHA256 = '48dca5f35f6dfeeabc337d45d6b28a162c1407abe4141c7508094e40e4cda9d6'


def patch_kernel(original, implementation):
    if hashlib.sha256(original.encode()).hexdigest() != KERNEL_SHA256:
        raise RuntimeError('native interrupt patch requires the pinned kernel_hal.c')

    def replace(old, new, expected=1):
        nonlocal original
        if original.count(old) != expected:
            raise RuntimeError('unexpected native interrupt patch site: ' + old)
        original = original.replace(old, new)

    replace('static volatile LONG g_irql_raised_count = 0;',
            'static volatile LONG g_irql_raised_count = 0;\n'
            'static volatile LONG g_native_irq_masked_count;\n'
            'static XBOX_THREAD_LOCAL int t_native_irq_masked;')
    replace('return InterlockedCompareExchange(&g_irql_raised_count, 0, 0) != 0;',
            'return InterlockedCompareExchange(&g_irql_raised_count, 0, 0) != 0\n'
            '        || InterlockedCompareExchange(&g_native_irq_masked_count, 0, 0) != 0;')
    replace('/* ============================================================================\n * Device interrupts on the gate holder',
            implementation + '\n\n/* ============================================================================\n * Device interrupts on the gate holder')
    replace('if (t_in_isr || g_current_irql > DISPATCH_LEVEL)',
            'if (t_native_irq_masked || t_in_isr || g_current_irql > DISPATCH_LEVEL)', expected=2)
    # Recursive acquisition by a posting holder must not bypass CLI either.
    start = original.index('int xbox_IrqPost(int line)\n')
    end = original.index('\nvoid xbox_IrqStatsGet(', start)
    post = original[start:end]
    if post.count('if (gate_try())') != 2:
        raise RuntimeError('unexpected interrupt posting gate sites')
    post = post.replace('if (gate_try())', 'if (!t_native_irq_masked && gate_try())')
    post = post.replace('if (!xbox_IrqSafePointsOn())',
                        'if (!xbox_IrqSafePointsOn() && !InterlockedCompareExchange(&g_native_irq_masked_count, 0, 0))')
    post = post.replace('if (s_irq_force && (was & bit)',
                        'if (s_irq_force && !InterlockedCompareExchange(&g_native_irq_masked_count, 0, 0) && (was & bit)')
    original = original[:start] + post + original[end:]
    return original

EXPORTS = ['gamepkg_takedawn_cli', 'gamepkg_takedawn_sti_after_shadow', 'gamepkg_takedawn_wbinvd']
ADAPTED_UNITS = {'recomp_0023_011': ['00353DB5'], 'recomp_0023_018': ['00367C86', '00367C97']}
PAIR = '''RECOMP_UNIMPL("cli", 0x00367C86u); /* TODO: cli */
    eax = MEM32(ecx + 8);
    eax = ZX16(MEM16(eax + 0x80));
    ebx = MMIO_RD32(-25157620);
    RECOMP_UNIMPL("sti", 0x00367C97u); /* TODO: sti */
    esi = eax;'''
PAIR_NATIVE = '''gamepkg_takedawn_cli();
    eax = MEM32(ecx + 8);
    eax = ZX16(MEM16(eax + 0x80));
    ebx = MMIO_RD32(-25157620);
    /* STI: deliver only after its original following MOV (interrupt shadow). */
    esi = eax;
    gamepkg_takedawn_sti_after_shadow();'''
CACHE = 'RECOMP_UNIMPL("wbinvd", 0x00353DB5u); /* TODO: wbinvd */'


def adapt(text):
    seen = []
    for marker in [PAIR, CACHE]:
        if text.count(marker) > 1:
            raise RuntimeError('duplicate native privileged adaptation site')
    if 'RECOMP_UNIMPL("cli", 0x00367C86u)' in text or 'RECOMP_UNIMPL("sti", 0x00367C97u)' in text:
        if text.count(PAIR) != 1:
            raise RuntimeError('incomplete or changed native interrupt-shadow region')
        text = text.replace(PAIR, PAIR_NATIVE)
        seen += ['00367C86', '00367C97']
    if CACHE in text:
        text = text.replace(CACHE, 'gamepkg_takedawn_wbinvd();')
        seen += ['00353DB5']
    if seen:
        text = ''.join('extern void ' + name + '(void);\n' for name in EXPORTS) + text
    return text, seen


def receipt(project, adapted_units):
    if adapted_units != ADAPTED_UNITS:
        raise RuntimeError('expected the complete reviewed native cache/interrupt sites')
    def sha(path):
        return hashlib.sha256(path.read_bytes()).hexdigest()
    return {'format': 'gamepkg-takedawn-native-privileged-v1',
            'implementation_sha256': sha(project / 'engines/takedawn/rust-port/native_privileged.rs'),
            'host_interrupt_sha256': sha(project / 'engines/takedawn/rust-port/native_irq.inc.h'),
            'host_patch_sha256': sha(project / 'tools/takedawn_native_privileged.py'),
            'original_kernel_sha256': KERNEL_SHA256,
            'adapted_units': adapted_units, 'native_exports': EXPORTS}
