"""End-to-end scoped saves, permission denial, update continuity and trap rollback."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
PROJECT = Path(__file__).resolve().parents[1]
CLI = PROJECT / 'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-saves-') as directory:
    root = Path(directory)
    source = root / 'source'
    shutil.copytree(PROJECT / 'examples/good/save-counter', source)
    key = root / 'key'
    key.write_text(os.urandom(32).hex())
    library = root / 'library'
    env = dict(os.environ, GAMEPKG_HOME=str(library), GAMEPKG_CONFIG=str(root / 'settings.toml'))
    def run(*args, success=True):
        result = subprocess.run([str(CLI), *map(str,args)], env=env, capture_output=True, text=True, timeout=75)
        assert (result.returncode == 0) == success, result.stderr
        return result.stdout + result.stderr
    version = 0
    def publish():
        global version
        version += 1
        manifest = source / 'manifest.toml'
        text = manifest.read_text()
        import re
        text = re.sub(r'version = "0\.\d+\.0"', f'version = "0.{version}.0"', text)
        manifest.write_text(text)
        package = root / f'game-{version}.gamepkg'
        run('pack', source, '-o', package, '--key', key)
        run('install', package, '--trust-creator')
        run('build', 'com.gamepkg.save-counter')
    publish()
    assert 'score: 2' in run('run', 'com.gamepkg.save-counter', '--frames', '2')
    assert 'score: 3' in run('run', 'com.gamepkg.save-counter', '--frames', '1')
    blobs = list((library / 'saves').glob('*.blob'))
    assert len(blobs) == 1
    before = blobs[0].read_bytes()
    (source / 'source/src/lib.rs').write_text('''
pub fn game_tick(_: u32, _: u64) -> u64 {
    assert_eq!(gamepkg::save_size(), Some(4));
    assert!(gamepkg::save_write(b"broken"));
    panic!("abort this tick")
}
''')
    publish()
    run('run', 'com.gamepkg.save-counter', '--frames', '1', success=False)
    assert blobs[0].read_bytes() == before
    (source / 'manifest.toml').write_text((source / 'manifest.toml').read_text().replace('save = true', 'save = false'))
    (source / 'source/src/lib.rs').write_text('''
pub fn game_tick(_: u32, _: u64) -> u64 {
    assert_eq!(gamepkg::save_size(), None);
    assert_eq!(gamepkg::save_read(0, &mut [0; 8]), None);
    assert!(!gamepkg::save_write(b"forbidden"));
    7u64 << 32
}
''')
    publish()
    assert 'score: 7' in run('run', 'com.gamepkg.save-counter', '--frames', '1')
    assert blobs[0].read_bytes() == before
    print('Saves passed: persistence, signed update continuity, trap rollback, permission denial.')
