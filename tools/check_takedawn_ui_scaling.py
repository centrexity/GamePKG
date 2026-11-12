#!/usr/bin/env python3
"""Maintainer check: real pinned scaler at 3x; unchanged 2x/4x golden images."""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--toolkit', type=Path, required=True)
    p.add_argument('--cc', default='clang-18')
    p.add_argument('--m32', action='store_true')
    p.add_argument('--system-include', action='append', default=[])
    a = p.parse_args()
    bridge = Path(__file__).resolve().parents[1] / 'engines/takedawn/media-bridge'
    src = a.toolkit.resolve() / 'src'
    with tempfile.TemporaryDirectory(prefix='gamepkg-ui-scale-') as temporary:
        work = Path(temporary)
        staged = []
        for kind, name in [('scale', 'nv2a_texscale.c'), ('store', 'nv2a_texscale_store.c')]:
            original = src / 'kernel' / name
            target = work / name
            def stage(source, output):
                return subprocess.run(['cmake', '-DINPUT='+str(source), '-DOUTPUT='+str(output),
                    '-DKIND='+kind, '-P', str(bridge / 'adapt_prompt_quality.cmake')], capture_output=True)
            before = original.read_bytes()
            result = stage(original, target)
            assert result.returncode == 0, result.stderr
            assert stage(original, original).returncode != 0
            alias = work / ('alias-'+name); alias.symlink_to(original)
            assert stage(original, alias).returncode != 0
            corrupt = work / ('corrupt-'+name); corrupt.write_bytes(before+b'\n/* changed */')
            assert stage(corrupt, work / ('rejected-'+name)).returncode != 0
            assert original.read_bytes() == before
            staged.append(target)
        test = (a.toolkit / 'tests/nv2a_texscale/test_main.c').read_text()
        test = test.replace('key, src, W, H, 3,', 'key, src, W, H, 5,')
        test = test.replace('W, H, 3, NV2A_TEXSCALE_SUPERXBR, dst) == 0',
                            'W, H, 5, NV2A_TEXSCALE_SUPERXBR, dst) == 0')
        test = test.replace('nv2a_texscale_set_factor(3);\n        CHECK(nv2a_texscale_factor() == 0);',
            'nv2a_texscale_set_factor(3);\n        CHECK(nv2a_texscale_factor() == 3);\n'
            '        nv2a_texscale_set_factor(0);\n        CHECK(nv2a_texscale_factor() == 0);')
        test = test.replace('    store(argc > 1',
            '    scaler(NV2A_TEXSCALE_SUPERXBR, 3);\n'
            '    scaler(NV2A_TEXSCALE_BICUBIC, 3);\n    store(argc > 1')
        test += '\nint gamepkg_prompt_scale(const uint32_t *s,uint32_t w,uint32_t h,unsigned f,uint32_t *d)'
        test += '{ (void)s;(void)w;(void)h;(void)f;(void)d;return 0; }\n'
        fixture = work / 'test.c'; fixture.write_text(test)
        command = [a.cc, '-std=c11', '-D_POSIX_C_SOURCE=200809L', '-O2', '-ffp-contract=off',
            '-fsanitize=address,undefined', '-fno-omit-frame-pointer', '-Wall', '-Wextra',
            '-I'+str(src/'kernel'), '-I'+str(src/'platform'), '-I'+str(src),
            '-I'+str(bridge), '-I'+str(bridge/'input-prompts')]
        if a.m32: command.append('-m32')
        for include in a.system_include: command += ['-idirafter', include]
        binary = work / 'check'
        subprocess.run(command + [str(fixture), *map(str,staged), str(src/'platform/recomp_env.c'),
            '-lm', '-pthread', '-o', str(binary)], check=True)
        cache = work / 'cache'; cache.mkdir()
        subprocess.run([str(binary), str(cache)], check=True, env=dict(os.environ, ASAN_OPTIONS="detect_leaks=0"))
        print('PASS: 3x Super-xBR/bicubic alpha, borders, determinism; 2x/4x golden images, cache and source guards')

if __name__ == '__main__': main()
