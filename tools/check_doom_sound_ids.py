"""Execute original sound routines with the production reviewed adaptations."""
from pathlib import Path
import argparse
import json
import os
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--upstream', action='store_true')
parser.add_argument('--32-bit', dest='bits32', action='store_true')
parser.add_argument('--cc', default='cc')
parser.add_argument('--runtime-libraries', type=Path)
parser.add_argument('--fixture', choices=['ids', 'spatial'], default='ids')
parser.add_argument('--normal-only', action='store_true')
parser.add_argument('--real-angles', action='store_true')
args = parser.parse_args()
if (args.real_angles or args.normal_only) and args.fixture != 'spatial':
    parser.error('--real-angles and --normal-only require --fixture spatial')
project = Path(__file__).resolve().parents[1]
source = (project / 'engines/doom/upstream/s_sound.c').read_text()
if not args.upstream:
    for patch in json.loads((project / 'engines/doom/adapters/sound-patches.json').read_text()):
        assert source.count(patch['before']) == patch['count'] == 1
        source = source.replace(patch['before'], patch['after'])
with tempfile.TemporaryDirectory(prefix='gamepkg-doom-sound-ids-') as temporary:
    root = Path(temporary)
    adapted = root / 's_sound.c'
    adapted.write_text(source)
    extra = []
    if args.fixture == 'spatial':
        fixed = (project / 'engines/doom/upstream/m_fixed.c').read_text()
        for patch in json.loads((project / 'engines/doom/adapters/fixed-patches.json').read_text()):
            assert fixed.count(patch['before']) == patch['count'] == 1
            fixed = fixed.replace(patch['before'], patch['after'])
        (root / 'fixed.c').write_text(fixed)
        extra = [str(root / 'fixed.c'), str(project / 'engines/doom/upstream/tables.c')]
        if args.real_angles:
            angles = (project / 'engines/doom/upstream/r_main.c').read_text()
            for patch in json.loads((project / 'engines/doom/adapters/angle-patches.json').read_text()):
                assert angles.count(patch['before']) == patch['count'] == 1
                angles = angles.replace(patch['before'], patch['after'])
            (root / 'angles.c').write_text(angles)
            extra += ['-DGAMEPKG_REAL_ANGLES', str(root / 'angles.c')]
    flags = (['-m32', '-DGAMEPKG_TEST_32', '-isystem', '/usr/include/x86_64-linux-gnu',
              '-fsanitize=undefined', '-fsanitize-undefined-trap-on-error'] if args.bits32
             else ['-fsanitize=address,undefined'])
    binary = root / 'test'
    subprocess.run([args.cc, '-O1', '-g', '-Wall', '-Wextra', '-Werror',
                    '-ffunction-sections', '-fdata-sections', '-Wl,--gc-sections',
                    *flags, '-iquote', str(project / 'engines/doom/upstream'),
                    f'-DGAMEPKG_SOUND_SOURCE="{adapted}"',
                    str(project / f'tests/doom_sound_{args.fixture}.c'), *extra,
                    '-o', str(binary)], check=True)
    if args.bits32:
        assert binary.read_bytes()[:5] == b'\x7fELF\x01'
    env = dict(os.environ, ASAN_OPTIONS='detect_leaks=1', UBSAN_OPTIONS='halt_on_error=1')
    if args.runtime_libraries:
        env['LD_LIBRARY_PATH'] = str(args.runtime_libraries.resolve())
    subprocess.run([str(binary), *(['--normal-only'] if args.normal_only else [])], env=env, check=True)
