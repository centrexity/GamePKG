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
    parser.add_argument('--kernel-bridge', type=Path, help='Pinned upstream kernel_bridge.c for native CMake adapter checks')
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
        if args.kernel_bridge:
            source = Path(work) / 'kernel_bridge.c'
            original = args.kernel_bridge.read_bytes()
            source.write_bytes(original)
            adapted = Path(work) / 'adapted.c'
            script = project / 'engines/takedawn/media-bridge/adapt_threads.cmake'
            def adapt(input_path, output_path):
                return subprocess.run(['cmake', '-DINPUT=' + str(input_path),
                                       '-DOUTPUT=' + str(output_path), '-P', str(script)],
                                      capture_output=True, text=True)
            run = adapt(source, adapted)
            if run.returncode:
                raise RuntimeError(run.stdout + run.stderr)
            text = adapted.read_text()
            assert text.count('gamepkg_windows_pin_executor();') == 1
            assert 'static HANDLE bridge_priority_thread' in text
            policy = (project / 'engines/takedawn/rust-port/native_windows_cpu.inc.h').read_text()
            assert policy in text and source.read_bytes() == original
            # Corruption must fail before writing output, including when the
            # requested output is the input itself.
            assert adapt(source, source).returncode != 0
            assert source.read_bytes() == original
            source.write_bytes(original + b'\n/* changed source */\n')
            rejected = Path(work) / 'rejected.c'
            bad = adapt(source, rejected)
            assert bad.returncode != 0 and 'pinned hash does not match' in bad.stderr
            assert not rejected.exists()
            results.append({'name': 'pinned-CMake-adapter-and-corruption-guards', 'exit': 0})
    print(json.dumps({'state': 'terminal', 'result': 'pass-portable-policy-and-patch-guards',
                      'scope': 'Not native game integration or an FPS qualification', 'checks': results}, indent=2))


if __name__ == '__main__':
    main()
