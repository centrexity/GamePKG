"""Verify portable original state component callbacks without game data."""
from pathlib import Path
import argparse
import json
import os
import signal
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--upstream', action='store_true')
parser.add_argument('--32-bit', dest='bits32', action='store_true')
parser.add_argument('--cc', default='cc')
parser.add_argument('--component', choices=['dma', 'dsp', 'ppu'], default='dma')
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
source = (project / f'engines/zelda3/upstream/snes/{args.component}.c').read_text()
if not args.upstream:
    for patch in json.loads((project / f'engines/zelda3/adapters/{args.component}-state-patches.json').read_text()):
        assert source.count(patch['before']) == patch['count'] == 1
        source = source.replace(patch['before'], patch['after'])
if args.component == 'ppu':
    # Keep strict warnings on the actual callback and new codec. The unrelated
    # original renderer has its existing warning policy in whole-engine builds.
    start = source.index('void ppu_saveload(')
    brace = source.index('{', start)
    end, depth = brace + 1, 1
    while depth:
        depth += (source[end] == '{') - (source[end] == '}')
        end += 1
    source = '#include "ppu.h"\n#include "gamepkg_ppu_state.h"\n' + source[start:end] + '\n'
with tempfile.TemporaryDirectory(prefix='gamepkg-zelda-dma-state-') as temporary:
    root = Path(temporary)
    adapted = root / f'{args.component}.c'
    adapted.write_text(source)
    flags = (['-m32', '-DGAMEPKG_TEST_32', '-isystem', '/usr/include/x86_64-linux-gnu',
              '-fsanitize=undefined', '-fsanitize-undefined-trap-on-error'] if args.bits32
             else ['-fsanitize=address,undefined'])
    binary = root / 'test'
    subprocess.run([args.cc, '-O1', '-g', '-Wall', '-Wextra', '-Werror',
                    '-ffunction-sections', '-fdata-sections', '-Wl,--gc-sections',
                    *flags, '-iquote', str(project / 'engines/zelda3/upstream/snes'),
                    '-I' + str(project / 'engines/zelda3/upstream'),
                    '-I' + str(project / 'engines/zelda3/adapters/include'),
                    f'-DGAMEPKG_{args.component.upper()}_SOURCE="{adapted}"',
                    str(project / f'tests/zelda_{args.component}_state.c'), '-o', str(binary)], check=True)
    if args.bits32:
        assert binary.read_bytes()[:5] == b'\x7fELF\x01'
    subprocess.run([str(binary)],
                   env=dict(os.environ, ASAN_OPTIONS='detect_leaks=1', UBSAN_OPTIONS='halt_on_error=1'),
                   check=True)
    if args.component == 'ppu':
        result = subprocess.run([str(binary), '--invalid-callback'], capture_output=True,
            env=dict(os.environ, ASAN_OPTIONS='detect_leaks=1', UBSAN_OPTIONS='halt_on_error=1'))
        if result.returncode != -signal.SIGABRT or result.stderr:
            raise RuntimeError('Invalid PPU callback did not reject flags cleanly: '
                + str(result.returncode) + ' ' + result.stderr.decode(errors='replace'))
        print('Invalid PPU callback rejected before returning to game execution.')
