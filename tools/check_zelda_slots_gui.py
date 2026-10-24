"""Private X11/Vulkan player proof for original snapshot hotkeys and restart.

Requires an already-built temporary signed fixture; never point this at a user
library. No media or snapshot bytes are printed or exported. Players need no
Python, Xvfb or test tools.
"""
from pathlib import Path
import argparse
import hashlib
import json
import os
import signal
import subprocess
import sys
import tempfile
import time
import tomllib
import zipfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--root', type=Path, required=True)
parser.add_argument('--rom', type=Path, required=True)
parser.add_argument('--target', choices=['native', 'wasm'], required=True)
parser.add_argument('--remap-slots', action='store_true', help='map function keys to slots 11–20')
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
sys.path.insert(0,str(project/'tests'))
from gui_test_support import start_display
root = args.root.resolve(strict=True)
assert root.is_relative_to(Path(tempfile.gettempdir()).resolve()), 'Only private temporary libraries are allowed'
rom = args.rom.resolve(strict=True)
rom_digest = hashlib.sha256(rom.read_bytes()).digest()
started = time.time_ns()
with zipfile.ZipFile(root / 'com.gamepkg.zelda3.gamepkg') as package:
    manifest = tomllib.loads(package.read('manifest.toml').decode())
roms = manifest.get('roms', [])
identity = [manifest['id'], manifest['creator_key'], bool(roms) or manifest.get('requires_rom', False), roms]
identity = hashlib.sha256(json.dumps(identity, separators=(',', ':'), ensure_ascii=False).encode()).hexdigest()
(root / 'rom-selections.json').write_text(json.dumps({identity: [str(rom)]}))

def wait(predicate, message, timeout=30):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        result = predicate()
        if result:
            return result
        time.sleep(0.05)
    raise AssertionError(message)

def slot(number):
    if args.remap_slots:
        number += 10
    entries = list((root / 'states').glob(f'*-{number:02}.gpstate'))
    if not entries:
        return None
    assert len(entries) == 1
    if entries[0].stat().st_mtime_ns < started:
        return None  # Another target's previous proof cannot satisfy this run.
    data = entries[0].read_bytes()
    assert len(data) == 96 + 275497 and data[:8] == b'GPZHSLOT'
    assert data[64:96] == hashlib.sha256(data[:64] + data[96:]).digest()
    assert int.from_bytes(data[52:56], 'little') == number - 1
    return int.from_bytes(data[56:64], 'little'), hashlib.sha256(data).digest()

with tempfile.TemporaryDirectory(prefix='gamepkg-zelda-slots-gui-') as temporary:
    private = Path(temporary)
    runtime = private / 'runtime'; runtime.mkdir(mode=0o700)
    settings = private / 'settings.toml'
    settings.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n')
    if args.remap_slots:
        with settings.open('a') as output:
            for field in ['snapshot_load_slots', 'snapshot_save_slots']:
                output.write(field + '=' + str(list(range(10,20))) + '\n')
    authority=private/'Xauthority';authority.touch(mode=0o600)
    env = dict(os.environ, GAMEPKG_HOME=str(root), GAMEPKG_CONFIG=str(settings), XAUTHORITY=str(authority),
        XDG_RUNTIME_DIR=str(runtime), WINIT_UNIX_BACKEND='x11', LIBGL_ALWAYS_SOFTWARE='1',GAMEPKG_RENDERER='vulkan')
    env.pop('WAYLAND_DISPLAY', None)
    server=start_display(env,1024,768,local_only=True)
    gui = None
    def stop():
        global gui
        if gui is not None:
            if gui.poll() is None:
                os.killpg(gui.pid, signal.SIGTERM)
            try:
                gui.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(gui.pid, signal.SIGKILL); gui.wait(timeout=10)
            gui = None
    try:
        log = open(private / 'player.log', 'ab')
        def xdo(*command):
            return subprocess.run(['xdotool', *map(str, command)], env=env, capture_output=True, text=True, timeout=5)
        def start():
            global gui
            gui = subprocess.Popen([str(project / 'target/release/gamepkg-gui'), '--play', manifest['id'], '--target', args.target],
                env=env, stdout=log, stderr=log, start_new_session=True)
            def window():
                assert gui.poll() is None, 'Game window stopped during startup: ' + (private / 'player.log').read_text(errors='replace')[-2000:]
                result = xdo('search', '--onlyvisible', '--name', '^' + manifest['id'] + '$')
                return result.stdout.strip().splitlines()[0] if result.returncode == 0 else None
            window_id = wait(window, 'Game window did not open')
            xdo('windowfocus', '--sync', window_id).check_returncode()
            return window_id
        start()
        last_press = 0
        def saved_title():
            global last_press
            assert gui.poll() is None, 'Game stopped before saving'
            if time.monotonic() - last_press > 0.25:
                xdo('key', '--clearmodifiers', 'shift+F1').check_returncode(); last_press = time.monotonic()
            value = slot(1)
            return value if value and value[0] >= 180 else None
        first, _ = wait(saved_title, 'Shift+F1 did not save a running title snapshot', timeout=45)
        xdo('keydown', 'shift').check_returncode(); xdo('keydown', 'F2').check_returncode()
        wait(lambda: slot(2), 'Shift+F2 did not save slot 2')
        before = slot(2)
        time.sleep(0.9)  # Cross the X server autorepeat delay while the key is held.
        assert slot(2) == before, 'Holding a snapshot key repeatedly replaced the slot'
        xdo('keyup', 'F2').check_returncode(); xdo('keyup', 'shift').check_returncode()
        stop(); start()
        last_press = 0
        def fresh_ready():
            global last_press
            if time.monotonic() - last_press > 0.25:
                xdo('key', '--clearmodifiers', 'shift+F4').check_returncode(); last_press = time.monotonic()
            return slot(4)
        fresh, _ = wait(fresh_ready, 'Restarted player did not become ready')
        assert fresh + 60 < first, 'Fresh session was not distinguishable from the saved counter'
        xdo('key', '--clearmodifiers', 'F1').check_returncode()
        time.sleep(0.15)
        xdo('key', '--clearmodifiers', 'shift+F3').check_returncode()
        restored, _ = wait(lambda: slot(3), 'Loaded session could not save another slot')
        assert first <= restored <= first + 45, f'F1 failed to restore the saved counter: {first}, {fresh}, {restored}'
        print(args.target + ': private Vulkan window saves, rejects held-key repeats, restarts and loads persisted state through ' + ('remapped slots 11–20.' if args.remap_slots else 'original function keys.'))
    finally:
        stop(); server.terminate()
        try:
            server.wait(timeout=10)
        except subprocess.TimeoutExpired:
            server.kill(); server.wait(timeout=10)
assert hashlib.sha256(rom.read_bytes()).digest() == rom_digest
