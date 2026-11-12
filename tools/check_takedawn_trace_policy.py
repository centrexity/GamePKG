#!/usr/bin/env python3
"""Asset-free maintainer checks for opt-in native register diagnostics."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile


BEFORE = '''        s_nv2a_trace = recomp_env(RENV_NV2A_TRACE) != NULL
                    || recomp_env(RENV_PB_SCAN) != NULL
                    || recomp_env(RENV_PB_EXEC) != NULL;'''
AFTER = '''        s_nv2a_trace = recomp_env(RENV_NV2A_TRACE) != NULL
                    || recomp_env(RENV_PB_SCAN) != NULL;'''
REPORT_BEFORE = 'if (s_nv2a_trace && now_ms - last_report > (uint64_t)report_ms)'
REPORT_AFTER = 'if ((s_nv2a_trace || s_pb_exec) && now_ms - last_report > (uint64_t)report_ms)'


def initializer(source):
    executor = re.findall(r'        s_pb_exec = recomp_env\(RENV_PB_EXEC\) != NULL;', source)
    trace = re.findall(r'        s_nv2a_trace = .*?;', source, re.S)
    if len(executor) != 1 or len(trace) != 1:
        raise RuntimeError('ambiguous native initialization sites')
    return executor[0] + '\n' + trace[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--toolkit', type=Path, required=True)
    parser.add_argument('--cc', default='cc')
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    source = args.toolkit.resolve() / 'src/kernel/xbox_memory_layout.c'
    original = source.read_text()
    source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
    adapter = project / 'engines/takedawn/media-bridge/adapt_memory.cmake'
    work = Path(tempfile.mkdtemp(prefix='gamepkg-explicit-trace.'))
    outputs = {}
    for mode in ['linux', 'windows']:
        output = work / (mode + '.c')
        subprocess.run(['cmake', '-DINPUT=' + str(source), '-DOUTPUT=' + str(output),
                        '-DWINDOWS_LAYOUT=' + ('ON' if mode == 'windows' else 'OFF'),
                        '-P', str(adapter)], check=True, capture_output=True, text=True)
        outputs[mode] = output.read_text()
        assert outputs[mode].count(AFTER) == 1 and BEFORE not in outputs[mode]
        assert outputs[mode].count(REPORT_AFTER) == 1 and REPORT_BEFORE not in outputs[mode]
        # This policy changes no other byte of startup, the worker, diagnostics,
        # fences, or memory access in the already reviewed adapter output.
        policy_only = adapter.read_text()
        start = policy_only.index('# The executor is enabled during ordinary play.')
        end = policy_only.index('if(WINDOWS_LAYOUT)', start)
        control_adapter = work / 'control.cmake'
        control_adapter.write_text(policy_only[:start] + policy_only[end:])
        control = work / (mode + '-control.c')
        subprocess.run(['cmake', '-DINPUT=' + str(source), '-DOUTPUT=' + str(control),
                        '-DWINDOWS_LAYOUT=' + ('ON' if mode == 'windows' else 'OFF'),
                        '-P', str(control_adapter)], check=True, capture_output=True, text=True)
        assert outputs[mode].replace(AFTER, BEFORE).replace(REPORT_AFTER, REPORT_BEFORE) == control.read_text()
        assert initializer(outputs[mode]).splitlines()[0] == initializer(original).splitlines()[0]

    prefix = '''#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
enum {RENV_PB_EXEC, RENV_NV2A_TRACE, RENV_PB_SCAN};
static unsigned present;
static const char *recomp_env(unsigned key) { return present & (1u << key) ? "1" : NULL; }
static int s_pb_exec, s_nv2a_trace;
'''
    fixture = prefix
    for mode, text in [('original', original), *outputs.items()]:
        fixture += 'static unsigned ' + mode + '(unsigned flags) { present=flags;\n'
        fixture += initializer(text) + '\nreturn s_pb_exec | (s_nv2a_trace << 1); }\n'
        report = REPORT_BEFORE if mode == 'original' else REPORT_AFTER
        fixture += 'static unsigned ' + mode + '_health(unsigned flags, uint64_t elapsed) { present=flags;\n'
        fixture += initializer(text) + '\nuint64_t now_ms=elapsed, last_report=0; long report_ms=10000;\n'
        fixture += 'return ' + report[4:-1] + '; }\n'
    fixture += '''int main(void) {
    for (unsigned flags=0; flags<8; flags++) {
        unsigned exec=flags&1u, trace=(flags&6u)!=0;
        unsigned want=exec|(trace<<1);
        if (linux(flags)!=want || windows(flags)!=want) return 17;
        if (original(flags)!=(exec|((exec||trace)<<1))) return 18;
        const unsigned elapsed[] = {0, 9999, 10000, 10001, 20000};
        for (unsigned i=0; i<5; i++) {
            unsigned health=(flags!=0 && elapsed[i]>10000);
            if (original_health(flags,elapsed[i])!=health ||
                linux_health(flags,elapsed[i])!=health ||
                windows_health(flags,elapsed[i])!=health) return 19;
        }
    }
    puts("PASS eight executor/register-trace/survey combinations and 40 health-report deadline cases; executor, explicit diagnostics and periodic renderer health retained");
    return 0;
}
'''
    fixture_path = work / 'policy.c'
    fixture_path.write_text(fixture)
    checks = []
    for name, flags in [('sanitized64', ['-fsanitize=address,undefined']), ('native32', ['-m32'])]:
        binary = work / name
        subprocess.run([args.cc, '-std=c11', '-O2', '-Ulinux', '-Wall', '-Wextra', '-Werror',
                        *flags, str(fixture_path), '-o', str(binary)], check=True, capture_output=True, text=True)
        result = subprocess.run([str(binary)], check=True, capture_output=True, text=True,
                                env=dict(os.environ, ASAN_OPTIONS='detect_leaks=0'))
        checks.append({'name': name, 'result': 'pass', 'stdout': result.stdout.strip()})
    # A broken policy that suppresses requested diagnostics must fail the oracle.
    corrupt = work / 'corrupt.c'
    corrupt.write_text(fixture.replace(AFTER, '        s_nv2a_trace = 0;'))
    binary = work / 'negative'
    subprocess.run([args.cc, '-std=c11', '-O2', '-Ulinux', str(corrupt), '-o', str(binary)], check=True)
    negative = subprocess.run([str(binary)], capture_output=True, text=True)
    assert negative.returncode == 17
    corrupt.write_text(fixture.replace(REPORT_AFTER[4:-1], '0'))
    subprocess.run([args.cc, '-std=c11', '-O2', '-Ulinux', str(corrupt), '-o', str(binary)], check=True)
    health_negative = subprocess.run([str(binary)], capture_output=True, text=True)
    assert health_negative.returncode == 19
    assert hashlib.sha256(source.read_bytes()).hexdigest() == source_hash
    print(json.dumps({'result': 'pass-explicit-native-trace-policy', 'evidence': str(work),
                      'source_unchanged': True, 'Linux_and_Windows_other_adaptation_bytes_unchanged': True,
                      'negative_control_exit': negative.returncode, 'checks': checks,
                      'suppressed_health_negative_control_exit': health_negative.returncode,
                      'scope': 'Startup policy and source preservation; full native gameplay/performance tested separately'}, indent=2))


if __name__ == '__main__':
    main()
