"""Maintainer-only original SPC tremolo oracle; ROM resources remain in RAM."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--rom', type=Path, required=True)
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
original = args.rom.read_bytes()
rom = original[512:] if len(original) == 1049088 else original
assert len(rom) == 1048576
assert hashlib.sha1(rom).hexdigest() == '6d4f10a8b10e10dbe624cb23cf03b88bb8252973'
ram = bytearray(65536)
position = 0x998000
def read(count):
    global position
    result = bytearray()
    for _ in range(count):
        offset = ((position & 0x7f0000) >> 1) | (position & 0x7fff)
        result.append(rom[offset])
        position += 1
        if position & 0xffff == 0:
            position += 0x8000
    return result
for _ in range(257):
    header = read(4)
    length = int.from_bytes(header[:2], 'little')
    target = int.from_bytes(header[2:], 'little')
    if not length:
        break
    assert target + length <= len(ram)
    ram[target:target + length] = read(length)
else:
    raise AssertionError('Unterminated sound bank')
with tempfile.TemporaryDirectory(prefix='gamepkg-tremolo-driver-') as temporary:
    binary = Path(temporary) / 'oracle'
    upstream = project / 'engines/zelda3/upstream'
    source = (upstream / 'src/spc_player.c').read_text()
    for name in ['spc-state-patches.json', 'spc-playback-patches.json']:
        for patch in json.loads((project / 'engines/zelda3/adapters' / name).read_text()):
            assert source.count(patch['before']) == patch['count']
            source = source.replace(patch['before'], patch['after'])
    player = Path(temporary) / 'player.c'
    player.write_text(source)
    dsp = (upstream / 'snes/dsp.c').read_text()
    for patch in json.loads((project / 'engines/zelda3/adapters/dsp-state-patches.json').read_text()):
        assert dsp.count(patch['before']) == patch['count']
        dsp = dsp.replace(patch['before'], patch['after'])
    dsp_source = Path(temporary) / 'dsp.c'
    dsp_source.write_text(dsp)
    subprocess.run(['cc', '-std=c11', '-O1', '-g', '-Wall', '-Wextra', '-Werror',
        '-fsanitize=address,undefined', '-ffunction-sections', '-Wl,--gc-sections',
        '-I' + str(upstream), '-iquote', str(upstream / 'src'),
        '-iquote', str(upstream / 'snes'),
        '-I' + str(project / 'engines/zelda3/adapters/include'),
        f'-DGAMEPKG_SPC_PLAYER_SOURCE="{player}"',
        str(project / 'tests/zelda_tremolo_driver.c'),
        str(dsp_source),
        str(project / 'engines/zelda3/upstream/snes/spc.c'), '-o', str(binary)], check=True)
    subprocess.run([str(binary)], input=ram, check=True, timeout=120)
assert args.rom.read_bytes() == original
print('Selected ROM unchanged; driver data remained in RAM.')
