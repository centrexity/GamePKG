"""Actual Cube 2 frame/input protocol on an isolated, display-free SDL renderer.

Run after the maintainer bridge build, with GAMEPKG_CUBE2_DATA pointing at
an original local Sauerbraten data directory. This test never publishes media.
"""
from pathlib import Path
import hashlib
import os
import select
import struct
import subprocess
import tempfile
import time

PROJECT = Path(__file__).resolve().parents[1]
DATA = Path(os.environ.get('GAMEPKG_CUBE2_DATA', PROJECT / 'target/cube2-test-data')).resolve()
BINARY = PROJECT / 'target/cube2-bridge-build/sauer_client'
assert (DATA / 'data/stdlib.cfg').is_file(), 'Original Cube 2 data folder required'
assert BINARY.is_file(), 'Run tools/build_cube2_engine.py first'


def run(malformed=False):
    with tempfile.TemporaryDirectory(prefix='cube2-bridge-test-') as temporary:
        logpath = Path(temporary) / 'engine.log'
        with logpath.open('wb') as log:
            command = ['bwrap', '--unshare-all', '--die-with-parent', '--new-session',
                '--ro-bind', '/usr', '/usr', '--symlink', 'usr/lib', '/lib',
                '--symlink', 'usr/lib64', '/lib64', '--symlink', 'usr/bin', '/bin',
                '--proc', '/proc', '--dev', '/dev', '--tmpfs', '/tmp', '--dir', '/save',
                '--dir', '/game', '--ro-bind', str(DATA / 'data'), '/game/data',
                '--ro-bind', str(DATA / 'packages'), '/game/packages', '--ro-bind', str(BINARY), '/engine',
                '--chdir', '/game', '--clearenv', '--setenv', 'PATH', '/usr/bin',
                '--setenv', 'HOME', '/save', '--setenv', 'SDL_VIDEODRIVER', 'offscreen',
                '--setenv', 'SDL_AUDIODRIVER', 'dummy', '--setenv', 'LIBGL_ALWAYS_SOFTWARE', '1',
                '--setenv', 'GALLIUM_DRIVER', 'llvmpipe', '--setenv', 'LP_NUM_THREADS', '2',
                '/engine', '-w640', '-h480', '-t0', '-q/save',
                '-lcomplex', '-xshowgui ""; consize 0; miniconsize 0; maxfps 60']
            app = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=log)
            def exact(length):
                result = bytearray()
                deadline = time.monotonic() + 60
                while len(result) < length:
                    assert time.monotonic() < deadline, 'Cube 2 frame timed out'
                    if select.select([app.stdout], [], [], 1)[0]:
                        chunk = os.read(app.stdout.fileno(), length - len(result))
                        assert chunk, f'Cube 2 exited: {logpath.read_text(errors="replace")[-6000:]}'
                        result.extend(chunk)
                return bytes(result)
            def tick(buttons=0, dx=0, dy=0):
                app.stdin.write(b'GPK1' + struct.pack('<Iii', buttons, dx, dy))
                app.stdin.flush()
                magic = exact(4)
                if magic == b'GPL1':
                    magic = exact(4)
                    assert magic != b'GPL1', 'Duplicate loading notification'
                header = magic + exact(16)
                assert header[:4] in [b'GPF2',b'GPF3'], 'Diagnostics polluted the frame channel'
                channels=struct.unpack('<I',exact(4))[0] if header[:4]==b'GPF3' else 1
                assert channels in [1,2]
                width, height, length, samples = struct.unpack('<IIII', header[4:])
                assert samples <= 4800
                assert (width, height, length) == (640, 480, 640*480*4)
                frame = exact(length)
                audio = exact(samples*channels*2)
                return frame
            try:
                app.stdin.write(b'GPB1' + bytes(12)); app.stdin.flush()
                first = tick()
                assert len(set(first[n:n+4] for n in range(0, len(first), 4))) > 64
                if malformed:
                    app.stdin.write(b'BAD!' + bytes(12)); app.stdin.flush()
                    assert app.wait(timeout=10) == 2
                    return
                # Advance actual Cube 2 physics, not a mock renderer.
                for _ in range(10):
                    tick()
                before = tick()
                for _ in range(12):
                    moved = tick(1 | (1<<16), 12, 0)
                assert hashlib.sha256(before).digest() != hashlib.sha256(moved).digest(), 'Movement did not change game frame'
                # Graceful EOF must release the engine without another tick.
                app.stdin.close()
                assert app.wait(timeout=10) == 0
                return moved
            finally:
                if app.poll() is None:
                    app.kill(); app.wait(timeout=10)

frame = run()
run(malformed=True)
from PIL import Image
Image.frombytes('RGBA', (640,480), frame).transpose(Image.Transpose.FLIP_TOP_BOTTOM).save(PROJECT / 'target/cube2-bridge.png')
print('Cube 2 isolated bridge: real rendered frames, movement, bounded dimensions, malformed protocol rejection and EOF shutdown passed; no display or network access.')
