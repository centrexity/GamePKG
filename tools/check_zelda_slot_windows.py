"""Run the production slot tests as a PE32 executable in an isolated Wine prefix.

Build the test executable with cargo test --locked --target i686-pc-windows-gnu
--no-default-features --lib zelda_snapshots::slots::tests --no-run, then pass
its path here. This qualifies Windows file API behavior under Wine, not a
physical Windows system or full game playback. Players need no test tools.
"""
from pathlib import Path
import argparse
import os
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--exe', type=Path, required=True)
args = parser.parse_args()
exe = args.exe.resolve(strict=True)
image = exe.read_bytes()
assert image[:2] == b'MZ'
pe = int.from_bytes(image[0x3c:0x40], 'little')
assert image[pe:pe+4] == b'PE\0\0' and image[pe+4:pe+6] == b'\x4c\x01', 'Expected a real i686 PE executable'
with tempfile.TemporaryDirectory(prefix='gamepkg-slot-wine-') as temporary:
    env = dict(os.environ, WINEPREFIX=str(Path(temporary) / 'prefix'), WINEARCH='win32',
        WINEDEBUG='-all', WINEDLLOVERRIDES='mscoree,mshtml=')
    try:
        result = subprocess.run(['xvfb-run', '-a', 'wine', str(exe), 'zelda_snapshots::slots::tests::', '--nocapture'],
            env=env, capture_output=True, text=True, timeout=180)
        assert result.returncode == 0, result.stdout[-4000:] + result.stderr[-4000:]
        assert 'test result: ok. 3 passed; 0 failed; 0 ignored;' in result.stdout, result.stdout[-4000:]
        print(result.stdout.strip())
        print('Production 32-bit Windows slot storage passed under Wine; isolated prefix removed.')
    finally:
        subprocess.run(['wineserver', '-k'], env=env, capture_output=True, timeout=15)
        subprocess.run(['wineserver', '-w'], env=env, capture_output=True, timeout=15)
