"""Private real-ROM Wasm snapshot proof. No extracted resources or state files."""
from pathlib import Path
import argparse
import importlib.util
import os
import subprocess
import sys
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--rom', type=Path, required=True)
parser.add_argument('--sysroot', type=Path, required=True)
parser.add_argument('--slang-root', type=Path, required=True)
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('gamepkg_snapshot_rom', project / 'engines/zelda3/adapters/rom_resources.py')
decoder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(decoder)
raw = args.rom.read_bytes()
active = True
def audit(event, values):
    if not active:
        return
    if event == 'open' and values[2] & (os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND):
        raise AssertionError('Resource decoding attempted a file write')
    if event in ['os.mkdir', 'os.rename', 'os.remove']:
        raise AssertionError('Resource decoding attempted a filesystem mutation')
sys.addaudithook(audit)
try:
    resources = decoder.decode(raw)
finally:
    active = False
with tempfile.TemporaryDirectory(prefix='gamepkg-zelda-snapshot-runtime-') as temporary:
    root = Path(temporary)
    library = root / 'library'
    key = root / 'synthetic.key'
    key.write_text('73' * 32)
    settings = root / 'settings.toml'
    settings.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
    env = dict(os.environ, GAMEPKG_HOME=str(library), GAMEPKG_CONFIG=str(settings),
        GAMEPKG_ZELDA3_SYSROOT=str(args.sysroot.resolve()), GAMEPKG_SLANG_ROOT=str(args.slang_root.resolve()))
    cli = project / 'target/release/gamepkg'
    def run(*command):
        subprocess.run(list(map(str, command)), cwd=project, env=env, check=True, timeout=600)
    run(cli, 'pack', project / 'ports/zelda3', '-o', root / 'zelda.gamepkg', '--key', key)
    run(cli, 'install', root / 'zelda.gamepkg', '--trust-creator')
    run(cli, 'build', 'com.gamepkg.zelda3', '--target', 'wasm')
    engines = list(library.rglob('zelda3.wasm'))
    assert len(engines) == 1
    subprocess.run(['node', str(project / 'tests/zelda_snapshot_engine.js'), str(engines[0])],
        cwd=project, env=env, input=resources, check=True, timeout=120)
assert args.rom.read_bytes() == raw
print('Selected ROM unchanged; snapshot resources and state remained in RAM.')
