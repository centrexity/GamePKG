"""Verify original whole-engine state callbacks with synthetic state only."""
from pathlib import Path
import argparse
import os
import json
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--32-bit', dest='bits32', action='store_true')
parser.add_argument('--cc', default='cc')
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
upstream = project / 'engines/zelda3/upstream'

def function(source, signature):
    assert source.count(signature) == 1
    start = source.index(signature)
    brace = source.index('{', start)
    end, depth = brace + 1, 1
    while depth:
        depth += (source[end] == '{') - (source[end] == '}')
        end += 1
    return source[start:end]

body = ''
for component in ['dma', 'dsp', 'ppu']:
    source = (upstream / f'snes/{component}.c').read_text()
    for patch in json.loads((project / f'engines/zelda3/adapters/{component}-state-patches.json').read_text()):
        assert source.count(patch['before']) == patch['count'] == 1
        source = source.replace(patch['before'], patch['after'])
    body += f'#include "gamepkg_{component}_state.h"\n'
    body += function(source, f'void {component}_saveload(') + '\n'
rtl = (upstream / 'src/zelda_rtl.c').read_text()
for name in ['InternalSaveLoad', 'LoadSnesState', 'SaveSnesState']:
    body += function(rtl, f'static void {name}(') + '\n'
body += '#include "gamepkg_snapshot_bridge.h"\n'
with tempfile.TemporaryDirectory(prefix='gamepkg-zelda-snapshot-') as temporary:
    root = Path(temporary)
    helpers = root / 'helpers.h'
    helpers.write_text(body)
    flags = (['-m32', '-DGAMEPKG_TEST_32', '-isystem', '/usr/include/x86_64-linux-gnu',
              '-fsanitize=undefined', '-fsanitize-undefined-trap-on-error'] if args.bits32
             else ['-fsanitize=address,undefined'])
    binary = root / 'test'
    subprocess.run([args.cc, '-O1', '-g', '-Wall', '-Wextra', '-Werror', *flags,
        '-I' + str(upstream), '-I' + str(project / 'engines/zelda3/adapters/include'),
        f'-DGAMEPKG_SNAPSHOT_HELPERS="{helpers}"', str(project / 'tests/zelda_snapshot.c'),
        '-o', str(binary)], check=True)
    if args.bits32:
        assert binary.read_bytes()[:5] == b'\x7fELF\x01'
    subprocess.run([str(binary)],
        env=dict(os.environ, ASAN_OPTIONS='detect_leaks=1', UBSAN_OPTIONS='halt_on_error=1'), check=True)
    # The original SPC map omits last_written_edl. Exercise the production
    # restore adapter and original echo setup across every saved/inherited byte.
    spc = (upstream / 'src/spc_player.c').read_text()
    for patch in json.loads((project / 'engines/zelda3/adapters/spc-state-patches.json').read_text()):
        assert spc.count(patch['before']) == patch['count'] == 1
        spc = spc.replace(patch['before'], patch['after'])
    maps = spc[spc.index('typedef struct MemMap {'):spc.index('static void PlayNote(')]
    audio = '#include "gamepkg_spc_state.h"\n' + maps + function(spc, 'void SpcPlayer_CopyVariablesFromRam(') + '\n'
    audio += function(spc, 'static void SetupEchoParameter_EDL(') + '\n'
    adapter = (project / 'engines/zelda3/adapters/engine.c').read_text()
    audio += function(adapter, 'void ZeldaRestoreMusicAfterLoad_Locked(') + '\n'
    cache = 'if (!reset) player->last_written_edl = player->dsp->ram[EDL];'
    assert audio.count(cache) == 1
    for old in [False, True]:
        helpers.write_text(audio.replace(cache, '') if old else audio)
        subprocess.run([args.cc, '-O1', '-g', '-Wall', '-Wextra', '-Werror', *flags,
            *(['-DGAMEPKG_EXPECT_OLD'] if old else []), '-I' + str(upstream), '-I' + str(project / 'engines/zelda3/adapters/include'),
            f'-DGAMEPKG_SNAPSHOT_AUDIO_HELPERS="{helpers}"', str(project / 'tests/zelda_snapshot_audio.c'),
            '-o', str(binary)], check=True)
        if args.bits32:
            assert binary.read_bytes()[:5] == b'\x7fELF\x01'
        subprocess.run([str(binary)],
            env=dict(os.environ, ASAN_OPTIONS='detect_leaks=1', UBSAN_OPTIONS='halt_on_error=1'), check=True)
