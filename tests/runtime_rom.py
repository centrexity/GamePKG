"""Signed ROM game compiles and reads only the player's selected file."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
PROJECT = Path(__file__).resolve().parents[1]
CLI = PROJECT / 'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-rom-') as directory:
    root = Path(directory)
    source = root / 'source'
    shutil.copytree(PROJECT / 'examples/good/rom-required', source)
    (source / 'source/src/lib.rs').write_text('''
pub fn game_tick(_: u32, _: u64) -> u64 {
    if !((gamepkg::rom_size()) == (Some(3))) { loop {} };
    let mut buffer = [0; 8];
    if !((gamepkg::rom_read(1, &mut buffer)) == (Some(2))) { loop {} };
    if !((&buffer[..2]) == (b"BC")) { loop {} };
    if !((gamepkg::rom_read(3, &mut buffer)) == (Some(0))) { loop {} };
    if !((gamepkg::rom_read(4, &mut buffer)) == (None)) { loop {} };
    if !((gamepkg::rom_read(u32::MAX, &mut buffer)) == (None)) { loop {} };
    42u64 << 32
}
''')
    key = root / 'key'
    key.write_text(os.urandom(32).hex())
    env = dict(os.environ, GAMEPKG_HOME=str(root / 'library'), GAMEPKG_CONFIG=str(root / 'settings.toml'))
    def run(*args, success=True):
        result = subprocess.run([str(CLI), *map(str,args)], env=env, capture_output=True, text=True, timeout=75)
        assert (result.returncode == 0) == success, result.stderr
        return result.stdout + result.stderr
    package = root / 'rom.gamepkg'
    run('pack', source, '-o', package, '--key', key)
    run('install', package, '--trust-creator')
    run('build', 'com.gamepkg.rom-demo')
    assert 'selected by you' in run('run', 'com.gamepkg.rom-demo', '--frames', '2', success=False)
    rom = root / 'player.rom'
    rom.write_bytes(b'ABC')
    before = rom.read_bytes()
    assert '42' in run('run', 'com.gamepkg.rom-demo', '--rom', rom, '--frames', '2')
    assert rom.read_bytes() == before
    rom.write_bytes(b'')
    assert '1 byte to 64 MiB' in run('run', 'com.gamepkg.rom-demo', '--rom', rom, '--frames', '1', success=False)
    with rom.open('wb') as file: file.truncate(64 * 1024 * 1024 + 1)
    assert '1 byte to 64 MiB' in run('run', 'com.gamepkg.rom-demo', '--rom', rom, '--frames', '1', success=False)
    assert 'unable to open' in run('run', 'com.gamepkg.rom-demo', '--rom', root / 'missing', '--frames', '1', success=False)
    print('ROM delivery passed: explicit selection, bounded reads, EOF and invalid offsets, empty/oversized/missing rejection.')
