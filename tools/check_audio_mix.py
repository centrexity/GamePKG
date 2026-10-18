"""Maintainer SDL S16 mixing oracle; no audio device or game assets needed."""
from pathlib import Path
import argparse
import hashlib
import json
import shlex
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('--write-fixture', action='store_true')
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
source = project / 'tools/audio-decoder/mix_reference.c'
fixture = project / 'tests/fixtures/sdl-s16-mix-reference.json'
flags = shlex.split(subprocess.check_output(
    ['pkg-config', '--cflags', '--libs', 'sdl2'], text=True))
version = subprocess.check_output(['pkg-config', '--modversion', 'sdl2'], text=True).strip()
with tempfile.TemporaryDirectory(prefix='gamepkg-mix-reference-') as temporary:
    executable = Path(temporary) / 'reference'
    subprocess.run(['cc', '-O2', str(source), '-o', str(executable), *flags], check=True)
    with subprocess.Popen([str(executable)], stdout=subprocess.PIPE) as process:
        hashes = []
        for volume in range(129):
            digest = hashlib.sha256()
            remaining = 65536 * 7 * 2
            while remaining:
                part = process.stdout.read(min(65536, remaining))
                assert part, f'truncated reference at volume {volume}'
                digest.update(part)
                remaining -= len(part)
            hashes.append(digest.hexdigest())
        assert not process.stdout.read(1), 'extra reference output'
        assert process.wait(timeout=10) == 0
record = dict(sdl_version=version, source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
              destinations=[-32768, -30000, -1, 0, 1, 30000, 32767],
              source_minimum=-32768, source_maximum=32767, volumes=list(range(129)),
              sha256=hashes)
if args.write_fixture:
    fixture.write_text(json.dumps(record, indent=2) + '\n')
else:
    assert json.loads(fixture.read_text()) == record, 'SDL reference changed'
print('SDL reference verified: 59,179,008 signed samples, all 129 volume levels.')
