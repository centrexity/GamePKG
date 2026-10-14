"""Exercise safe guest asset wrappers through signing, isolated build and execution."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
PROJECT = Path(__file__).resolve().parents[1]
CLI = PROJECT / 'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-assets-') as directory:
    root = Path(directory)
    source = root / 'source'
    shutil.copytree(PROJECT / 'examples/minimal', source)
    (source / 'assets').mkdir(exist_ok=True)
    (source / 'assets/message.txt').write_bytes(b'ABC')
    (source / 'source/src/lib.rs').write_text('''
pub fn game_tick(_: u32, _: u64) -> u64 {
    let mut buffer = [0; 8];
    if !((gamepkg::asset_read("message.txt", 1, &mut buffer)) == (Some(2))) { loop {} };
    if !((&buffer[..2]) == (b"BC")) { loop {} };
    if !((gamepkg::asset_read("message.txt", 3, &mut buffer)) == (Some(0))) { loop {} };
    if !((gamepkg::asset_read("message.txt", 4, &mut buffer)) == (None)) { loop {} };
    for path in ["../manifest.toml", "/etc/passwd", "missing", "../assets/message.txt"] {
        if !((gamepkg::asset_read(path, 0, &mut buffer)) == (None)) { loop {} };
    }
    42u64 << 32
}
''')
    key = root / 'key'
    key.write_text(os.urandom(32).hex())
    env = dict(os.environ, GAMEPKG_HOME=str(root / 'library'), GAMEPKG_CONFIG=str(root / 'settings.toml'))
    def run(*args):
        result = subprocess.run([str(CLI), *map(str,args)], env=env, capture_output=True, text=True, timeout=75)
        assert result.returncode == 0, result.stderr
        return result.stdout
    package = root / 'assets.gamepkg'
    run('pack', source, '-o', package, '--key', key)
    run('install', package, '--trust-creator')
    run('build', 'com.gamepkg.minimal')
    output = run('run', 'com.gamepkg.minimal', '--frames', '2')
    assert '42' in output, output
    print('Guest assets passed: chunk reads, EOF, missing paths and traversal denied.')
