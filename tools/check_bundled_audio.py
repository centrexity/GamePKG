"""Maintainer check: both relocated entry points contain the same decoder."""
from pathlib import Path
import argparse
import json
import os
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('binary_directory', type=Path)
args = parser.parse_args()
suffix = '.exe' if os.name == 'nt' else ''
with tempfile.TemporaryDirectory(prefix='gamepkg-relocated-decoder-') as temporary:
    root = Path(temporary)
    environment = dict(os.environ,
                       GAMEPKG_AUDIO_DECODER_WASM=str(root/'missing.wasm'),
                       GAMEPKG_BUNDLED_AUDIO_DECODER=str(root/'missing.wasm'))
    environment.pop('DISPLAY', None)
    environment.pop('WAYLAND_DISPLAY', None)
    records = []
    for name, option in [('gamepkg', 'check-bundled-audio'), ('gamepkg-gui', '--check-bundled-audio')]:
        binary = root/(name+suffix)
        shutil.copy2(args.binary_directory/(name+suffix), binary)
        result = subprocess.run([str(binary), option], cwd=root, env=environment,
                                capture_output=True, text=True, timeout=30)
        assert result.returncode == 0, result.stderr
        record = json.loads(result.stdout)
        assert 0 < record['maximum_memory_bytes'] <= 6*1048576
        assert len(record['sha256']) == 64
        records.append(record)
    assert records[0] == records[1], 'CLI and GUI contain different decoders'
print('Relocated CLI and GUI validate the same embedded decoder without runtime files or a display:', records[0]['sha256'])
