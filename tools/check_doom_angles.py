"""Qualify actual DOOM angle arithmetic without game media or window devices."""
from pathlib import Path
import argparse
import json
import os
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--upstream', action='store_true')
parser.add_argument('--normal-only', action='store_true')
parser.add_argument('--32-bit', dest='bits32', action='store_true')
parser.add_argument('--cc', default='cc')
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
source = (project / 'engines/doom/upstream/r_main.c').read_text()
if not args.upstream:
    for patch in json.loads((project / 'engines/doom/adapters/angle-patches.json').read_text()):
        assert source.count(patch['before']) == patch['count'] == 1
        source = source.replace(patch['before'], patch['after'])
with tempfile.TemporaryDirectory(prefix='gamepkg-doom-angles-') as temporary:
    root = Path(temporary)
    adapted = root / 'r_main.c'
    adapted.write_text(source)
    flags = (['-m32', '-DGAMEPKG_TEST_32', '-isystem', '/usr/include/x86_64-linux-gnu',
              '-fsanitize=undefined', '-fsanitize-undefined-trap-on-error'] if args.bits32
             else ['-fsanitize=address,undefined'])
    binary = root / 'test'
    subprocess.run([args.cc, '-O1', '-g', '-Wall', '-Wextra', '-Werror',
                    '-ffunction-sections', '-fdata-sections', '-Wl,--gc-sections',
                    *flags, '-iquote', str(project / 'engines/doom/upstream'),
                    f'-DGAMEPKG_ANGLE_SOURCE="{adapted}"',
                    str(project / 'tests/doom_angles.c'),
                    str(project / 'engines/doom/upstream/tables.c'),
                    '-o', str(binary)], check=True)
    if args.bits32:
        assert binary.read_bytes()[:5] == b'\x7fELF\x01'
    subprocess.run([str(binary), *(['--normal-only'] if args.normal_only else [])],
                   env=dict(os.environ, ASAN_OPTIONS='detect_leaks=1', UBSAN_OPTIONS='halt_on_error=1'),
                   check=True)
