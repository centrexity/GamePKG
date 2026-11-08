#!/usr/bin/env python3
"""Asset-free maintainer checks for exact Windows clock conversion."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--toolkit', type=Path, required=True)
    parser.add_argument('--cc', default='clang')
    parser.add_argument('--sanitize', action='store_true')
    parser.add_argument('--m32', action='store_true')
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    bridge = project / 'engines/takedawn/media-bridge'
    source = args.toolkit.resolve() / 'src/platform/host_time.c'
    before_hash = sha(source)
    work = Path(tempfile.mkdtemp(prefix='gamepkg-clock-conversion.'))
    output = work / 'host_time.c'

    def adapt(original, target):
        return subprocess.run(['cmake', '-DINPUT=' + str(original),
                               '-DOUTPUT=' + str(target), '-P',
                               str(bridge / 'adapt_windows_clock.cmake')],
                              capture_output=True, text=True)

    result = adapt(source, output)
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)
    # Match the exact full-file candidate qualified with physical Windows APIs.
    assert sha(output) == '0b2529643d9f3bee1ba7710ceb1d6335e740a475bae8714d62658e80e37bd6ac'
    assert sha(source) == before_hash
    # Unknown source and attempts to overwrite upstream must fail before writes.
    unknown = work / 'unknown.c'
    unknown.write_text(source.read_text() + '\n/* changed preimage */\n')
    rejected_output = work / 'rejected.c'
    assert adapt(unknown, rejected_output).returncode != 0
    assert not rejected_output.exists()
    assert adapt(source, source).returncode != 0 and sha(source) == before_hash
    alias = work / 'clock-alias.c'
    alias.symlink_to(source)
    assert adapt(source, alias).returncode != 0 and sha(source) == before_hash

    # Extract both complete clock functions; only reset their static frequency
    # cache in the fixture so every supplied frequency reaches the same API path.
    prefix = '''#include <stdint.h>
#define _WIN32 1
typedef union {int64_t QuadPart;} LARGE_INTEGER;
extern int64_t clock_fixture_counter, clock_fixture_frequency;
static int QueryPerformanceFrequency(LARGE_INTEGER *p) {p->QuadPart=clock_fixture_frequency;return 1;}
static int QueryPerformanceCounter(LARGE_INTEGER *p) {p->QuadPart=clock_fixture_counter;return 1;}
'''
    for name, path in [('original', source), ('candidate', output)]:
        text = path.read_text()
        start = text.index('uint64_t xbox_HostNowNs(void)')
        end = text.index('\nvoid xbox_HostSleepNs', start)
        function = text[start:end].replace('xbox_HostNowNs(void)', name + '_now(void)')
        assert function.count('    static LARGE_INTEGER freq;') == 1
        function = function.replace('    static LARGE_INTEGER freq;',
                                    '    static LARGE_INTEGER freq;\n    freq.QuadPart = 0;')
        (work / (name + '.c')).write_text(prefix + function)
    candidate = (work / 'candidate.c').read_text()
    assert candidate.count('(uint64_t)t.QuadPart * 100u;') == 1
    (work / 'negative.c').write_text(candidate.replace('(uint64_t)t.QuadPart * 100u;',
                                                      '(uint64_t)t.QuadPart * 99u;'))
    fixture = bridge / 'tests/check_clock_conversion.c'
    flags = ['-std=c11', '-O3']
    if args.m32:
        flags += ['-m32']
    if args.sanitize:
        flags += ['-g', '-fsanitize=address,undefined']
    checks = []
    for name, code, expected in [('correctness', 'candidate.c', 0),
                                 ('negative', 'negative.c', 1)]:
        binary = work / name
        subprocess.run([args.cc, *flags, str(work / 'original.c'), str(work / code),
                        str(fixture), '-o', str(binary)], check=True)
        run = subprocess.run([str(binary)], capture_output=True, text=True)
        checks.append({'name': name, 'exit': run.returncode,
                       'stdout': run.stdout.strip(), 'stderr': run.stderr.strip()})
        assert run.returncode == expected, checks[-1]
        if name == 'correctness':
            assert run.stdout.strip() == ('PASS exact_clock_cases=12583128 fast_path_cases=523674 '
                                          'frequencies=12 negative_and_wrap_boundaries=true')
        else:
            assert 'mismatch frequency=10000000 counter=1 original=100 candidate=99' in run.stderr
    assert sha(source) == before_hash
    report = {'result': 'pass', 'work': str(work), 'checks': checks,
              'upstream_source_unchanged': True, 'unknown_preimage_rejected': True,
              'in_place_and_symlink_overwrite_rejected': True,
              'patched_source_sha256': sha(output),
              'scope': 'Exact clock-function outputs and adaptation guards; not game performance.'}
    (work / 'qualification.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
