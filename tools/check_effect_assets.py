"""Maintainer-only signed original-rate effect loader check.
Original MP3s, generated WAVs, and synthetic signed packages stay temporary.
Players do not need Python, SDL tools, or an external decoder.
"""
from pathlib import Path
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import wave

parser = argparse.ArgumentParser()
parser.add_argument('original', type=Path)
parser.add_argument('--private-output', type=Path, help='Create a new private directory of exact original-rate migration inputs; never publishes')
parser.add_argument('--include-music', action='store_true', help='Also copy original playlist inputs into the private output')
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
reference = json.loads((project/'tests/fixtures/arkanoo-recorded-effects.json').read_text())
assert hashlib.sha256((args.original/'README.md').read_bytes()).hexdigest()==reference['upstream_credits_readme_sha256'],'Pinned upstream credit source changed'
oracle_sha=hashlib.sha256((project/'tools/audio-decoder/reference.c').read_bytes()).hexdigest()
assert not args.include_music or args.private_output is not None,'Music preparation requires an explicit private output'
assert args.private_output is None or not args.private_output.exists(),'Private output must be new'
with tempfile.TemporaryDirectory(prefix='gamepkg-original-rate-effects-') as temporary:
    root = Path(temporary)
    binary = root/'decode'
    flags = subprocess.check_output(['pkg-config', '--cflags', '--libs', 'SDL2_mixer', 'sdl2'], text=True).split()
    subprocess.run(['cc', str(project/'tools/audio-decoder/reference.c'), '-o', str(binary), *flags], check=True)
    prepared=[]
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
        prepared.append({'asset':effect['asset'],'source':effect['original'],'source_sha256':effect['original_sha256'],'rate':44100,'frames':len(pcm)//4,'pcm_sha256':hashlib.sha256(pcm).hexdigest(),'wav_sha256':hashlib.sha256((root/Path(effect['asset']).name).read_bytes()).hexdigest()})
    subprocess.run(['cargo', 'test', '--locked', '--release', '--test', 'audio_effect_assets',
                    'all_original_recordings_load_from_signed_assets_inside_one_effect_pool',
                    '--', '--ignored', '--nocapture'], cwd=project,
                   env=dict(os.environ, GAMEPKG_EFFECT_WAV_DIRECTORY=str(root)), check=True, timeout=300)
    if args.private_output is not None:
        destination=args.private_output.resolve()
        assert not destination.is_relative_to(project),'Migration recordings must remain outside the source workspace'
        destination.mkdir(parents=True,exist_ok=False);(destination/'audio').mkdir()
        for effect in prepared:shutil.copy2(root/Path(effect['asset']).name,destination/effect['asset'])
        songs=[]
        if args.include_music:
            (destination/'music').mkdir()
            for source in sorted((args.original/'assets/music').glob('*.mp3')):
                assert source.is_file() and not source.is_symlink() and not any(c in source.name for c in '\r\n'),'Invalid music source'
                path='music/'+source.name;shutil.copy2(source,destination/path)
                songs.append({'asset':path,'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'bytes':source.stat().st_size})
        (destination/'audio/music-list.txt').write_text(''.join(song['asset']+'\n' for song in songs),encoding='utf-8')
        (destination/'migration-inputs.json').write_text(json.dumps({'upstream_revision':reference['revision'],'decoder_source_sha256':oracle_sha,'effects':prepared,'music':songs,'private_verification_inputs':True},indent=2)+'\n')
        print('Private migration inputs prepared:',destination,'effects',len(prepared),'songs',len(songs))
print('All five pinned original recordings: signed 44100 Hz asset playback and shared-pool cleanup passed.')
