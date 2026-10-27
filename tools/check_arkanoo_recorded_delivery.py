"""Verify the canonical port delivers every qualified recording and image.

Maintainer check only; players use the bundled Rust audio companions.
The separately frozen 48 kHz SDL fixture still verifies legacy packages.
"""
from pathlib import Path
import argparse
import hashlib
import json
import tomllib
import wave
from PIL import Image

project = Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--candidate',type=Path);args=parser.parse_args()
port = args.candidate or project / 'ports/arkanoo'
proof = json.loads((port / 'assets/ASSET_PROVENANCE.json').read_text())
legacy = json.loads((project / 'tests/fixtures/arkanoo-recorded-effects.json').read_text())
assert proof['upstream_revision'] == legacy['revision']
limits = tomllib.loads((port / 'manifest.toml').read_text())['limits']
large=limits.get('render_profile')=='large-images-v1'
assert limits.get('render_profile') in [None,'large-images-v1']
hard=268435456 if large else 16777216
assert (limits['hard_memory_bytes'], limits['effects_memory_bytes'], limits['music_memory_bytes']) == (hard, 3145728, 7340032)
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
assert len(images) == (178 if large else 76)
assert {row['asset'] for row in images} == {p.name for p in (port / 'assets').glob('*.png')}
for row in images:
    path = port / 'assets' / row['asset']
    assert hashlib.sha256(path.read_bytes()).hexdigest() == row['encoded_sha256'], path
    with Image.open(path) as image:
        assert list(image.size) == row['size']
        assert hashlib.sha256(image.convert('RGBA').tobytes()).hexdigest() == row['rgba_sha256'], path
if large:
    originals=json.loads((project/'tests/fixtures/arkanoo-original-image-pixels.json').read_text())['images']
    rows=proof['original_backgrounds']['tiles'];assert len(rows)==144
    assert len(proof['original_backgrounds']['sources'])==6
    for number,reference in enumerate(originals,1):
        source=proof['original_backgrounds']['sources'][number-1]
        assert source['source_sha256']==reference['source_sha256'] and source['rgba_sha256']==reference['rgba_sha256']
        whole=Image.new('RGBA',(2816,1536))
        for index in range(24):
            row=next(row for row in rows if row['asset']==f'background{number}-{index}.png')
            x,y=index%6*510,index//6*510;width,height=min(510,2816-x),min(510,1536-y)
            left,top=max(0,x-1),max(0,y-1)
            assert row['core']==[x,y,width,height]
            with Image.open(port/'assets'/row['asset']) as tile:
                whole.paste(tile.convert('RGBA').crop((x-left,y-top,x-left+width,y-top+height)),(x,y))
        assert hashlib.sha256(whole.tobytes()).hexdigest()==reference['rgba_sha256'],number
print('Canonical delivery: 5 original-rate effects, all 9 original music tracks,',len(images),'lossless images;',hard//1048576,'MiB hard memory; independent legacy fixtures preserved.')
