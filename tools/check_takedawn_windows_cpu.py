#!/usr/bin/env python3
"""Maintainer checks for hybrid Windows worker placement; no game/media input."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cc', default='clang-18', help='C compiler with 32-bit multilib support')
    args = parser.parse_args()
    project = Path(__file__).resolve().parent.parent
    fixture = project / 'tools/fixtures/takedawn_windows_cpu.c'
    results = []
    with tempfile.TemporaryDirectory(prefix='gamepkg-windows-cpu.') as work:
        for name, flags in [('64bit-sanitized', ['-fsanitize=address,undefined', '-fno-omit-frame-pointer']),
                            ('32bit', ['-m32'])]:
            output = Path(work) / name
            command = [args.cc, '-std=c11', '-O2', '-Wall', '-Wextra', '-Werror',
                       *flags, str(fixture), '-o', str(output)]
            subprocess.run(command, check=True, capture_output=True, text=True)
            # No allocations in the portable test; namespace/ptrace wrappers can
            # prevent LeakSanitizer from operating. ASan/UBSan remain enabled.
            env = dict(os.environ, ASAN_OPTIONS='detect_leaks=0')
            run = subprocess.run([str(output)], env=env, check=True, capture_output=True, text=True)
            results.append({'name': name, 'exit': run.returncode, 'stdout': run.stdout.strip(),
                            'randomized_cases': 10000})
        patch = subprocess.run(['python3', '-m', 'unittest', 'test_takedawn_windows_cpu'],
                               cwd=project / 'tools', check=True, capture_output=True, text=True)
        results.append({'name': 'source-patch-guards', 'exit': patch.returncode, 'output': patch.stderr.strip()})
    print(json.dumps({'state': 'terminal', 'result': 'pass-portable-policy-and-patch-guards',
                      'scope': 'Not native game integration or an FPS qualification', 'checks': results}, indent=2))


if __name__ == '__main__':
    main()
