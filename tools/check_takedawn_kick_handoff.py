#!/usr/bin/env python3
"""Maintainer-only native kick handoff checks. No game or ROM assets required."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
from check_takedawn_trace_policy import AFTER as TRACE_AFTER, BEFORE as TRACE_BEFORE
from check_takedawn_trace_policy import REPORT_AFTER, REPORT_BEFORE


def run(command, **kwargs):
    return subprocess.run(command, capture_output=True, text=True, check=True, **kwargs)


def ack_block(source):
    start = source.index('                if (*kr & NV2A_KICK_BIT) {')
    return source[start:source.index('                if (now - t0 >=', start)]


def ordering_fixture(original, adapted):
    text = '''#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#define NV2A_KICK_BIT 0x10000u
#define NV2A_KICK_REG 0x100410u
#define NV2A_USER_DMA_PUT 0x800040u
static uint32_t g_scanned_put;
static int acknowledged, early;
static volatile uint32_t *active_regs;
static void kick_cleared(uint64_t now) {
    (void)now;
    acknowledged++;
    if (*(volatile uint32_t *)((char *)active_regs + NV2A_USER_DMA_PUT) != g_scanned_put)
        early++;
}
'''
    for name, source in [('original', original), ('candidate', adapted)]:
        text += f'''static void {name}(volatile uint32_t *regs) {{
    volatile uint32_t *kr = (volatile uint32_t *)((char *)regs + NV2A_KICK_REG);
    uint64_t now = 1, last_active = 0;
    do {{
{ack_block(source)}
    }} while (0);
    (void)last_active;
}}
'''
    text += '''int main(void) {
    void *mem = calloc(1, NV2A_USER_DMA_PUT + 4);
    if (!mem) return 3;
    active_regs = mem;
    volatile uint32_t *put = (volatile uint32_t *)((char *)mem + NV2A_USER_DMA_PUT);
    volatile uint32_t *kr = (volatile uint32_t *)((char *)mem + NV2A_KICK_REG);
    /* Publish between the worker's earlier PUT read and later busy-bit read. */
    g_scanned_put = 0; *put = 1; *kr = NV2A_KICK_BIT;
    original(mem);
    if (early != 1 || acknowledged != 1 || (*kr & NV2A_KICK_BIT)) return 10;
    acknowledged = early = 0; g_scanned_put = 0; *put = 1; *kr = NV2A_KICK_BIT;
    candidate(mem);
    if (early || acknowledged || !(*kr & NV2A_KICK_BIT)) return 11;
    g_scanned_put = 1;
    candidate(mem);
    if (early || acknowledged != 1 || (*kr & NV2A_KICK_BIT)) return 12;
    free(mem);
    puts("PASS original negative control, retained pending flush, acknowledgement after scan");
    return 0;
}
'''
    return text


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--toolkit', required=True, type=Path, help='Pinned xboxrecomp source root')
    parser.add_argument('--cc', default='clang-18', help='Native compiler with 32-bit multilib')
    parser.add_argument('--windows-output', type=Path, help='Also preserve a cross-built Win32 stress fixture here')
    parser.add_argument('--windows-cc', default='x86_64-w64-mingw32-gcc')
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    bridge = project / 'engines/takedawn/media-bridge'
    kernel = args.toolkit.resolve() / 'src/kernel'
    checks = []
    with tempfile.TemporaryDirectory(prefix='gamepkg-kick-handoff-') as temp:
        work = Path(temp)
        inputs = {}
        outputs = {}
        for kind, filename in [('memory', 'xbox_memory_layout.c'), ('pacing', 'kernel_pacing.c')]:
            data = (kernel / filename).read_bytes()
            source = work / ('original ' + filename)
            source.write_bytes(data)
            staged = work / ('staged ' + filename)
            adapter = bridge / ('adapt_' + kind + '.cmake')

            def stage(src, dst, windows=True):
                return subprocess.run(['cmake', '-DINPUT=' + str(src), '-DOUTPUT=' + str(dst),
                                       '-DWINDOWS_LAYOUT=' + ('ON' if windows else 'OFF'),
                                       '-P', str(adapter)], capture_output=True, text=True)

            result = stage(source, staged)
            assert result.returncode == 0, result.stderr
            assert source.read_bytes() == data
            inputs[kind] = data.decode()
            outputs[kind] = staged.read_text()
            assert stage(source, source).returncode != 0
            assert source.read_bytes() == data
            if os.name != 'nt':
                alias = work / ('alias ' + filename)
                alias.symlink_to(source)
                assert stage(source, alias).returncode != 0
                assert source.read_bytes() == data
            corrupted = work / ('changed ' + filename)
            corrupted.write_bytes(data + b'\n/* changed upstream */\n')
            rejected = work / ('rejected ' + filename)
            result = stage(corrupted, rejected)
            assert result.returncode != 0 and 'pinned hash differs' in result.stderr
            assert not rejected.exists()
            if kind == 'memory':
                linux = work / 'nonWindows.c'
                result = stage(source, linux, False)
                assert result.returncode == 0, result.stderr
                # Linux keeps the mapping guard and explicit trace policy;
                # the Windows-only wake helper must not enter this output.
                expected = data.decode().replace(
                    '            XR_ERROR( "  WARNING: tiled aperture at 0x%08X failed"\n'
                    '                    " (error %lu); rendering writes will fault\\n",\n'
                    '                    XBOX_TILED_BASE, GetLastError());',
                    '            XR_ERROR( "GamePKG: required tiled memory mapping at 0x%08X failed"\n'
                    '                    " (error %lu); stopping before game startup.\\n",\n'
                    '                    XBOX_TILED_BASE, (unsigned long)GetLastError());\n'
                    '            return FALSE;')
                assert expected.count(TRACE_BEFORE) == 1
                expected = expected.replace(TRACE_BEFORE, TRACE_AFTER)
                assert expected.count(REPORT_BEFORE) == 1
                expected = expected.replace(REPORT_BEFORE, REPORT_AFTER)
                prefix = '/* GamePKG: stop cleanly when the required tiled alias cannot map. */\n'
                assert linux.read_text() == prefix + expected
            checks.append({'name': kind + '-pinned-staging-and-corruption-guards', 'result': 'pass'})
        # Pacing notification must occur only after the spin-sleep policy allows sleeping.
        pacing = outputs['pacing']
        assert pacing.index('    if (!g_xbox_spin_sleep)\n        return;') < pacing.index('xbox_Nv2aKickWake();')
        assert pacing.count('xbox_Nv2aKickWake();') == 1
        fixture = work / 'ordering.c'
        fixture.write_text(ordering_fixture(inputs['memory'], outputs['memory']))
        for name, flags in [('64bit-sanitized', ['-fsanitize=address,undefined', '-fno-omit-frame-pointer']),
                            ('32bit', ['-m32'])]:
            binary = work / name
            run([args.cc, '-std=c11', '-O2', '-Wall', '-Wextra', '-Werror', *flags,
                 str(fixture), '-o', str(binary)])
            result = run([str(binary)], env=dict(os.environ, ASAN_OPTIONS='detect_leaks=0'))
            checks.append({'name': 'exact-ack-block-' + name, 'result': 'pass', 'stdout': result.stdout.strip()})
        if args.windows_output:
            output = args.windows_output.resolve()
            output.mkdir(parents=True, exist_ok=True)
            host = (args.toolkit / 'src/platform/host_time.c').read_text()
            host = host[host.index('void xbox_HostSleepNs(uint64_t ns)'):host.index('/* ── The wakeable sleep')]
            memory = outputs['memory']
            start = memory.index('/* Private candidate: an auto-reset event')
            end = memory.index('/* The bit was seen clear', start)
            (output / 'gamepkg_test_host_sleep.inc').write_text(host)
            (output / 'gamepkg_test_kick_helper.inc').write_text(memory[start:end])
            binary = output / 'takedawn-kick-wake-fixture.exe'
            run([args.windows_cc, '-O2', '-std=c11', '-Wall', '-Wextra', '-Werror', '-I', str(output),
                 str(project / 'tools/fixtures/takedawn_kick_wake_win32.c'), '-o', str(binary), '-lwinmm'])
            checks.append({'name': 'real-helper-Win32-stress-fixture-crossbuild', 'result': 'pass',
                           'path': str(binary), 'sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                           'execution': 'Run on physical Windows separately; cross-build is not a runtime pass.'})
    print(json.dumps({'result': 'pass-native-ordering-and-overlay-guards',
                      'scope': 'Maintainer regression checks; no game/media/FPS/physical input qualification',
                      'checks': checks}, indent=2))


if __name__ == '__main__':
    main()
