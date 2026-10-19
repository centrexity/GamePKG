"""Maintainer-only, lossless private asset sizing; never rewrites the port.
Players need neither this tool nor its Pillow/optipng dependencies.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import zlib
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
original = optimized = precise = legacy_precise = 0
def exact_png(encoded):
    if encoded[25] not in [2,3,6] or encoded[28] or (encoded[24] not in ([1,2,4,8] if encoded[25]==3 else [8])):
        return False
    at=8
    while at+12<=len(encoded):
        length=int.from_bytes(encoded[at:at+4],"big");kind=encoded[at+4:at+8];end=at+12+length
        if end>len(encoded) or kind==b"acTL" or kind==b"tRNS" and encoded[25]!=3:return False
        if kind==b"IEND":return end==len(encoded)
        at=end
    return False
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
                    legacy_precise += len(rgba)
                    if not exact_png(destination.read_bytes()):
                        precise += len(rgba)
                rows.append({'asset': name, 'source_sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                             'rgba_sha256': hashlib.sha256(rgba).hexdigest(),
                             'optimized_sha256': hashlib.sha256(destination.read_bytes()).hexdigest(),
                             'original_bytes': size, 'optimized_bytes': destination.stat().st_size})
        size = destination.stat().st_size
    optimized += size
migration = json.loads((args.migration_inputs/'migration-inputs.json').read_text())
# Replace the old effect assets rather than double-counting both encodings.
effect_raw=effect_packed=effect_max=0
for effect in migration['effects']:
    name = effect['asset']
    assert hashlib.sha256((args.migration_inputs/name).read_bytes()).hexdigest() == effect['wav_sha256'], name
    data=(args.migration_inputs/name).read_bytes();packed=zlib.compress(data,9)
    assert zlib.decompress(packed)==data
    effect_raw+=len(data);effect_packed+=len(packed);effect_max=max(effect_max,len(data))
    optimized += len(data) - (source/name).stat().st_size
music = 0
for song in migration['music']:
    track = args.migration_inputs/song['asset']
    assert hashlib.sha256(track.read_bytes()).hexdigest() == song['source_sha256'], song['asset']
    music += track.stat().st_size
playlist = (args.migration_inputs/'audio/music-list.txt').stat().st_size
report = {'private_verification_inputs': True, 'original_assets_bytes': original,
          'optimized_assets_with_replacement_effects_bytes': optimized,
          'music_bytes': music, 'playlist_bytes': playlist, 'precise_image_bytes': precise, 'legacy_precise_image_bytes': legacy_precise,
          'uncompressed_web_export_bytes': optimized+music+playlist+precise,
          'effect_wav_bytes': effect_raw, 'python_zlib_effect_estimate_bytes': effect_packed,
          'python_encoded_export_estimate_bytes': optimized+music+playlist+precise-effect_raw+effect_packed,
          'asset_scratch_bytes_per_owner': effect_max+263168,
          'estimate_note': 'Exact host encoder sizes and memory compatibility are verified by check_arkanoo_recorded_web.py',
          'web_export_limit_bytes': 32*1024*1024, 'images': rows}
(output/'residency.json').write_text(json.dumps(report, indent=2)+'\n')
print(json.dumps({key: value for key, value in report.items() if key != 'images'}, indent=2))
print('All', len(rows), 'PNG dimensions and decoded RGBA bytes match exactly; published assets unchanged.')
