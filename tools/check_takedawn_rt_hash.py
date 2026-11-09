#!/usr/bin/env python3
"""Asset-free maintainer checks for exact render-target backing-memory hashes.

Players do not run this script. The pinned original hash and ownership functions
are the oracle; the adapter must leave all other source bytes unchanged.
"""
import argparse
import hashlib
from pathlib import Path
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--toolkit', required=True, type=Path)
    parser.add_argument('--cc', default='cc')
    parser.add_argument('--cflag', action='append', default=[])
    parser.add_argument('--ldflag', action='append', default=[])
    parser.add_argument('--benchmark', action='store_true')
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    bridge = project / 'engines/takedawn/media-bridge'
    toolkit = args.toolkit.resolve()
    kernel = toolkit / 'src/kernel'
    source = kernel / 'nv2a_backend_common.c'
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    original_digest = digest(source)
    assert original_digest == '599e4afa22886d1a088054251b5e795dff6cd94290cf3f4b0ac67f2c26b0ec29'
    original = source.read_text()
    old = '    return nv2a_tex_hash(mem + addr, (uint32_t)n, NV2A_TEX_HASH_SEED);'
    new = '    return gamepkg_rt_hash(mem + addr, (uint32_t)n, NV2A_TEX_HASH_SEED);'
    assert original.count(old) == 1
    with tempfile.TemporaryDirectory(prefix='gamepkg-rt-hash.') as temporary:
        work = Path(temporary)
        output = work / source.name

        def adapt(src, dst):
            return subprocess.run(
                ['cmake', '-DINPUT=' + str(src), '-DOUTPUT=' + str(dst),
                 '-P', str(bridge / 'adapt_rt_zero_hash.cmake')],
                capture_output=True, text=True)

        result = adapt(source, output)
        assert result.returncode == 0, result.stdout + result.stderr
        assert output.read_text() == '#include "rt_zero_hash.h"\n' + original.replace(old, new)
        rejected = work / 'rejected.c'
        unknown = work / 'unknown.c'
        unknown.write_text(original + '\n/* unknown preimage */\n')
        assert adapt(unknown, rejected).returncode != 0 and not rejected.exists()
        assert adapt(output, rejected).returncode != 0 and not rejected.exists()
        assert adapt(source, source).returncode != 0
        alias = work / 'alias.c'
        alias.symlink_to(source)
        assert adapt(source, alias).returncode != 0
        assert digest(source) == original_digest

        start = original.index('uint64_t nv2a_tex_hash(')
        end = original.index('\nstatic _Atomic', start)
        hash_body = original[start:end].strip()
        assert hashlib.sha256(hash_body.encode()).hexdigest() == (
            '2cf5c0484cbc2480b059ddc1ffd00ef657d398cbabd4dde487dd25f2d988e9be')
        oracle = work / 'original.c'
        oracle.write_text('#include <stdint.h>\n#include <string.h>\n' + hash_body + '\n')
        shutil.copyfile(bridge / 'rt_zero_hash.h', work / 'rt_zero_hash.h')
        fixture = work / 'hash.c'
        shutil.copyfile(project / 'tools/fixtures/takedawn_rt_hash.c', fixture)
        common = [args.cc, '-std=gnu11', '-O2', *args.cflag, '-I' + str(work)]

        def run(command, expected=0):
            result = subprocess.run(command, capture_output=True, text=True, timeout=120)
            assert result.returncode == expected, (command, result.stdout, result.stderr)
            return result.stdout

        for name, flags in [('automatic', []), ('portable', ['-DGAMEPKG_RT_HASH_PORTABLE'])]:
            executable = work / name
            run([*common, *flags, str(oracle), str(fixture), *args.ldflag,
                 '-o', str(executable)])
            text = run([str(executable)])
            assert 'PASS cases=' in text and 'protected_boundaries' in text
            print(name + ': ' + text.strip())
            if args.benchmark and name == 'automatic':
                print(run([str(executable), '--benchmark']), end='')

        # Reject an incorrect prime power and a shortcut that ignores nonzero
        # data. If the optimized path stops running, these controls also fail.
        for name, before, after in [
            ('power', 'gamepkg_rt_prime_power(i / 32);', 'gamepkg_rt_prime_power(i / 32 + 1);'),
            ('prefix', 'uint32_t i = gamepkg_rt_zero_prefix(p, n);', 'uint32_t i = n - n % 32;'),
        ]:
            folder = work / ('negative-' + name)
            folder.mkdir()
            header = (bridge / 'rt_zero_hash.h').read_text()
            assert header.count(before) == 1
            (folder / 'rt_zero_hash.h').write_text(header.replace(before, after))
            mutated = folder / 'hash.c'
            shutil.copyfile(fixture, mutated)
            executable = folder / 'check'
            run([*common, str(oracle), str(mutated), *args.ldflag, '-o', str(executable)])
            result = subprocess.run([str(executable)], capture_output=True, text=True, timeout=30)
            assert result.returncode == 1 and 'CHECK_FAILED' in result.stderr
        print('negative controls: wrong power and unchecked nonzero data rejected')

        # Extract the actual original ownership implementation, rename only its
        # symbols, and compare its state/counters with the adapted implementation.
        start = original.index('static _Atomic uint64_t s_rt_hashes;')
        end = original.index('\nsize_t nv2a_tex_texels(', start)
        reference = original[start:end]
        for before, after in [
            ('s_rt_hashes', 'reference_hash_count'),
            ('rt_own_hash(', 'reference_own_hash('),
            ('nv2a_rt_own_reset', 'reference_rt_own_reset'),
            ('nv2a_rt_own_stale', 'reference_rt_own_stale'),
            ('nv2a_rt_own_hashes', 'reference_rt_own_hashes'),
        ]:
            reference = reference.replace(before, after)
        (work / 'rt_hash_reference.inc').write_text(reference)
        state = work / 'ownership.c'
        shutil.copyfile(project / 'tools/fixtures/takedawn_rt_ownership.c', state)
        includes = ['-I' + str(kernel), '-I' + str(toolkit / 'src/d3d'),
                    '-I' + str(toolkit / 'src/platform')]
        environment = toolkit / 'src/platform/recomp_env.c'
        executable = work / 'ownership'
        run([*common, *includes, str(state), str(environment), '-lm', '-pthread',
             *args.ldflag, '-o', str(executable)])
        text = run([str(executable)])
        assert 'PASS 24576 original ownership' in text and '40000 threaded checks' in text
        print(text.strip())
        executable = work / 'upstream-rt-alias'
        run([*common, *includes, str(output), str(toolkit / 'tests/rt_alias/test_main.c'),
             str(environment), '-lm', '-pthread', *args.ldflag, '-o', str(executable)])
        assert 'rt_alias: ok' in run([str(executable)])
        print('upstream rt_alias: ok; original source unchanged')
        assert digest(source) == original_digest


if __name__ == '__main__':
    main()
