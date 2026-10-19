"""Maintainer-only signed original-rate effect loader check.
Original MP3s, generated WAVs, and synthetic signed packages stay temporary.
Players do not need Python, SDL tools, or an external decoder.
"""
from pathlib import Path
import argparse
import hashlib
import json
import os
import subprocess
import tempfile
import wave

parser = argparse.ArgumentParser()
parser.add_argument('original', type=Path)
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
reference = json.loads((project/'tests/fixtures/arkanoo-recorded-effects.json').read_text())
with tempfile.TemporaryDirectory(prefix='gamepkg-original-rate-effects-') as temporary:
    root = Path(temporary)
    binary = root/'decode'
    flags = subprocess.check_output(['pkg-config', '--cflags', '--libs', 'SDL2_mixer', 'sdl2'], text=True).split()
    subprocess.run(['cc', str(project/'tools/audio-decoder/reference.c'), '-o', str(binary), *flags], check=True)
    for effect in reference['effects']:
        source = args.original/'assets'/effect['original']
        assert hashlib.sha256(source.read_bytes()).hexdigest() == effect['original_sha256'], source.name
        pcm = subprocess.check_output([str(binary), str(source), '44100'])
        assert 0 < len(pcm) <= 2*1048576 and len(pcm) % 4 == 0
        with wave.open(str(root/Path(effect['asset']).name), 'wb') as output:
            output.setnchannels(2)
            output.setsampwidth(2)
            output.setframerate(44100)
            output.writeframes(pcm)
    subprocess.run(['cargo', 'test', '--locked', '--release', '--test', 'audio_effect_assets',
                    'all_original_recordings_load_from_signed_assets_inside_one_effect_pool',
                    '--', '--ignored', '--nocapture'], cwd=project,
                   env=dict(os.environ, GAMEPKG_EFFECT_WAV_DIRECTORY=str(root)), check=True, timeout=300)
print('All five pinned original recordings: signed 44100 Hz asset playback and shared-pool cleanup passed.')
