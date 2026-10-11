"""Validate real AV1/Theora previews and reject wrong codecs and damaged video."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
PROJECT = Path(__file__).resolve().parents[1]
CLI = PROJECT / 'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-previews-') as directory:
    root = Path(directory)
    key = root / 'creator.key'
    subprocess.run([str(CLI), 'keygen', '-o', str(key)], check=True, capture_output=True)
    def video(path, codec):
        subprocess.run(['ffmpeg', '-v', 'error', '-f', 'lavfi', '-i', 'testsrc2=size=64x64:rate=5', '-t', '1', '-an', '-c:v', codec, '-threads', '1', '-y', str(path)], check=True, timeout=60)
    for name, codec in [('preview.av1.webm', 'libaom-av1'), ('preview.ogv', 'libtheora')]:
        source = root / codec
        shutil.copytree(PROJECT / 'examples/minimal', source)
        video(source / name, codec)
        package = root / (codec + '.gamepkg')
        result = subprocess.run([str(CLI), 'pack', str(source), '-o', str(package), '--key', str(key)], capture_output=True, text=True, timeout=120)
        assert result.returncode == 0, result.stderr
        result = subprocess.run([str(CLI), 'verify', str(package)], capture_output=True, text=True, timeout=120)
        assert result.returncode == 0, result.stderr
        result = subprocess.run([str(CLI), 'preview', str(package)], capture_output=True, text=True, timeout=120)
        assert result.returncode == 0, result.stderr
        assert 'Decoded 5 preview frames' in result.stdout, result.stdout
    for case in ['wrong-codec', 'damaged', 'wrong-container']:
        source = root / case
        shutil.copytree(PROJECT / 'examples/minimal', source)
        preview = source / 'preview.av1.webm'
        if case == 'wrong-codec':
            video(preview, 'libvpx-vp9')
        elif case == 'wrong-container':
            subprocess.run(['ffmpeg', '-v', 'error', '-f', 'lavfi', '-i', 'testsrc2=size=64x64:rate=5', '-t', '1', '-an', '-c:v', 'libaom-av1', '-threads', '1', '-f', 'matroska', str(preview)], check=True, timeout=60)
        else:
            preview.write_bytes(b'\x1a\x45\xdf\xa3broken')
        package = root / (case + '.gamepkg')
        result = subprocess.run([str(CLI), 'pack', str(source), '-o', str(package), '--key', str(key)], capture_output=True, text=True, timeout=120)
        assert result.returncode != 0, case
        assert not package.exists(), case
    print('Real AV1 and Theora previews passed; wrong codecs and damaged containers rejected without publication.')
