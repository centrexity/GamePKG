"""Asset-free maintainer check; Python is never a player build requirement."""
from pathlib import Path
import argparse, hashlib, os, shlex, subprocess, tempfile

HERE = Path(__file__).resolve().parent
REFERENCE_SHA256 = '9fcc7d7cff232f6486472a3473290bc8503ac5a7566d124f48eaf89d14dd0948'


def inputs():
    patch = (HERE.parent/'patches/0012-gamepkg-hud-upscale-simd.patch').read_text()
    sections = {}
    for section in patch.split('diff --git ')[1:]:
        header, _, body = section.partition('\n')
        before, after = header.split()
        assert before.startswith('a/') and after.startswith('b/')
        sections[after[2:]] = body.splitlines()
    original = '\n'.join(line[1:] for line in sections['port/linux/src/hud_hires.c']
                         if line.startswith('-') and not line.startswith('---')).strip()+'\n'
    assert hashlib.sha256(original.encode()).hexdigest() == REFERENCE_SHA256, 'Original reference changed'
    assert '+#include "gamepkg_hud_upscale.inc"' in sections['port/linux/src/hud_hires.c']
    adapted = '\n'.join(line[1:] for line in sections['port/linux/src/gamepkg_hud_upscale.inc']
                        if line.startswith('+') and not line.startswith('+++'))+'\n'
    return original, adapted


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cc', default=os.environ.get('CC', 'cc'))
    parser.add_argument('--cflag', action='append', default=[])
    parser.add_argument('--sanitize', action='store_true')
    parser.add_argument('--scalar', action='store_true')
    parser.add_argument('--quick', action='store_true')
    parser.add_argument('--runner', default='')
    parser.add_argument('--source', type=Path, help='Also verify the actual patched checkout')
    args = parser.parse_args()
    original, adapted = inputs()
    if args.source:
        assert (args.source/'port/linux/src/gamepkg_hud_upscale.inc').read_text() == adapted
        assert '#include "gamepkg_hud_upscale.inc"' in (args.source/'port/linux/src/hud_hires.c').read_text()
    reference = original.replace('upscale_weights', 'reference_weights').replace(
        'hud_hires_upscale_pixels', 'reference_pixels')
    with tempfile.TemporaryDirectory(prefix='gamepkg-hud-upscale-') as temporary:
        root = Path(temporary)
        (root/'gamepkg_hud_upscale.inc').write_text(adapted)
        (root/'fixture.c').write_text(
            '#include <stdint.h>\n#include <stdlib.h>\n#include <string.h>\n'+reference+
            '\n#include "gamepkg_hud_upscale.inc"\n'+(HERE/'hud_upscale.c').read_text())
        command = [*shlex.split(args.cc), '-std=c11', '-D_POSIX_C_SOURCE=200809L', '-O2', '-g', '-ffp-contract=off', '-frounding-math',
                   '-Wall', '-Wextra', '-Werror', *args.cflag]
        if args.scalar:
            command += ['-U__SSE2__']
        if args.sanitize:
            command += ['-fsanitize=address,undefined', '-fno-sanitize-recover=all']
        command += [str(root/'fixture.c'), '-lm', '-o', str(root/'fixture')]
        compile_result = subprocess.run(command, capture_output=True, text=True, timeout=120)
        assert compile_result.returncode == 0, compile_result.stderr[-12000:]
        env = dict(os.environ)
        if args.quick:
            env['GAMEPKG_HUD_QUICK'] = '1'
        result = subprocess.run([*shlex.split(args.runner), str(root/'fixture')], env=env,
                                capture_output=True, text=True, timeout=180)
        assert result.returncode == 0, result.stdout[-2000:]+'\n'+result.stderr[-12000:]
        print(result.stdout, end='')


if __name__ == '__main__':
    main()
