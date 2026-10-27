"""Verify the canonical port delivers every qualified recording and image.

Maintainer check only; players use the bundled Rust audio companions.
The separately frozen 48 kHz SDL fixture still verifies legacy packages.
"""
from pathlib import Path
import hashlib
import json
import tomllib
import wave
from PIL import Image

project = Path(__file__).resolve().parents[1]
port = project / 'ports/arkanoo'
proof = json.loads((port / 'assets/ASSET_PROVENANCE.json').read_text())
legacy = json.loads((project / 'tests/fixtures/arkanoo-recorded-effects.json').read_text())
assert proof['upstream_revision'] == legacy['revision']
limits = tomllib.loads((port / 'manifest.toml').read_text())['limits']
assert (limits['hard_memory_bytes'], limits['effects_memory_bytes'], limits['music_memory_bytes']) == (16777216, 3145728, 7340032)
assert limits['recommended_memory_bytes'] == limits['hard_memory_bytes']
assert len(proof['effects']) == 5 and len(proof['music']) == 9
assert {row['source_sha256'] for row in proof['effects']} == {row['original_sha256'] for row in legacy['effects']}

for row in proof['effects']:
    path = port / 'assets' / row['asset']
    assert hashlib.sha256(path.read_bytes()).hexdigest() == row['wav_sha256'], path
    with wave.open(str(path)) as recording:
        assert (recording.getframerate(), recording.getnchannels(), recording.getsampwidth(), recording.getnframes()) == (44100, 2, 2, row['frames'])
        assert hashlib.sha256(recording.readframes(row['frames'])).hexdigest() == row['pcm_sha256']
for row in legacy['effects']:
    path = project / 'tests/fixtures/arkanoo-legacy-effects' / Path(row['asset']).name
    assert hashlib.sha256(path.read_bytes()).hexdigest() == row['wav_sha256'], path

playlist = ''.join(row['asset'] + '\n' for row in proof['music'])
assert (port / 'assets/audio/music-list.txt').read_text() == playlist
assert {row['asset'] for row in proof['music']} == {p.relative_to(port / 'assets').as_posix() for p in (port / 'assets/music').glob('*.mp3')}
for row in proof['music']:
    path = port / 'assets' / row['asset']
    assert path.stat().st_size == row['bytes']
    assert hashlib.sha256(path.read_bytes()).hexdigest() == row['source_sha256'], path
images = proof['image_optimization']['images']
assert len(images) == 76
assert {row['asset'] for row in images} == {p.name for p in (port / 'assets').glob('*.png')}
for row in images:
    path = port / 'assets' / row['asset']
    assert hashlib.sha256(path.read_bytes()).hexdigest() == row['encoded_sha256'], path
    with Image.open(path) as image:
        assert list(image.size) == row['size']
        assert hashlib.sha256(image.convert('RGBA').tobytes()).hexdigest() == row['rgba_sha256'], path
print('Canonical delivery: 5 original-rate effects, all 9 original music tracks, 76 lossless images, unchanged 16 MiB hard limit; legacy fixtures independently preserved.')
