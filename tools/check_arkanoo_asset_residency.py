"""Maintainer-only, lossless private asset sizing; never rewrites the port.
Players need neither this tool nor its Pillow/optipng dependencies.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
from PIL import Image

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--private-output', type=Path, required=True)
parser.add_argument('--migration-inputs', type=Path, required=True)
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
output = args.private_output.resolve()
assert output != project and project not in output.parents, 'Output must remain outside the workspace'
assert not output.exists(), 'Output must be new'
source = project/'ports/arkanoo/assets'
output.mkdir(parents=True)
rows = []
original = optimized = precise = 0
for path in sorted(source.rglob('*')):
    if not path.is_file():
        continue
    assert not path.is_symlink(), 'Symlink asset refused'
    name = path.relative_to(source).as_posix()
    original += path.stat().st_size
    size = path.stat().st_size
    if path.suffix.lower() == '.png':
        destination = output/name
        destination.parent.mkdir(parents=True, exist_ok=True)
        with Image.open(path) as image:
            image.load()
            image.save(destination, format='PNG', optimize=True)
            subprocess.run(['optipng', '-quiet', '-o2', str(destination)], check=True, timeout=60)
            with Image.open(destination) as rebuilt:
                rebuilt.load()
                assert image.size == rebuilt.size
                assert image.convert('RGBA').tobytes() == rebuilt.convert('RGBA').tobytes(), name
                rgba = image.convert('RGBA').tobytes()
                if any(value != 255 for value in rgba[3::4]):
                    precise += len(rgba)
                rows.append({'asset': name, 'source_sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                             'rgba_sha256': hashlib.sha256(rgba).hexdigest(),
                             'optimized_sha256': hashlib.sha256(destination.read_bytes()).hexdigest(),
                             'original_bytes': size, 'optimized_bytes': destination.stat().st_size})
        size = destination.stat().st_size
    optimized += size
migration = json.loads((args.migration_inputs/'migration-inputs.json').read_text())
# Replace the old effect assets rather than double-counting both encodings.
for effect in migration['effects']:
    name = effect['asset']
    assert hashlib.sha256((args.migration_inputs/name).read_bytes()).hexdigest() == effect['wav_sha256'], name
    optimized += (args.migration_inputs/name).stat().st_size - (source/name).stat().st_size
music = 0
for song in migration['music']:
    track = args.migration_inputs/song['asset']
    assert hashlib.sha256(track.read_bytes()).hexdigest() == song['source_sha256'], song['asset']
    music += track.stat().st_size
playlist = (args.migration_inputs/'audio/music-list.txt').stat().st_size
report = {'private_verification_inputs': True, 'original_assets_bytes': original,
          'optimized_assets_with_replacement_effects_bytes': optimized,
          'music_bytes': music, 'playlist_bytes': playlist, 'precise_image_bytes': precise,
          'web_export_accounted_bytes': optimized+music+playlist+precise,
          'web_export_limit_bytes': 32*1024*1024, 'images': rows}
(output/'residency.json').write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps({key: value for key, value in report.items() if key != 'images'}, indent=2))
print('All', len(rows), 'PNG dimensions and decoded RGBA bytes match exactly; published assets unchanged.')
