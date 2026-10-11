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
    manifest = source / 'manifest.toml'
    manifest.write_text('runtime_profile = "component-v1"\nrequires_rom = true\n' + manifest.read_text().replace('audio = false','audio = true').replace('save = false','save = true'))
    rom = root / 'selected.rom'; rom.write_bytes(b'ROM')
    (source / 'assets').mkdir(exist_ok=True)
    (source / 'assets/message.txt').write_bytes(b'ABC')
    (source / 'source/src/lib.rs').write_text('''
pub fn game_tick(_: u32, _: u64) -> u64 {
    let mut buffer = [0; 8];
    assert_eq!(gamepkg::asset_read("message.txt", 1, &mut buffer), Some(2));
    assert_eq!(&buffer[..2], b"BC");
    assert_eq!(gamepkg::asset_read("message.txt", 3, &mut buffer), Some(0));
    assert_eq!(gamepkg::asset_read("message.txt", 4, &mut buffer), None);
    for path in ["../manifest.toml", "/etc/passwd", "missing", "../assets/message.txt"] {
        assert_eq!(gamepkg::asset_read(path, 0, &mut buffer), None);
    }
    assert_eq!(gamepkg::rom_size(), Some(3));
    assert_eq!(gamepkg::rom_read(1, &mut buffer), Some(2));
    assert_eq!(&buffer[..2], b"OM");
    assert!(gamepkg::save_write(b"saved"));
    assert_eq!(gamepkg::save_size(), Some(5));
    assert_eq!(gamepkg::save_read(0, &mut buffer), Some(5));
    assert_eq!(&buffer[..5], b"saved");
    assert!(gamepkg::draw_clear(0x102030ff));
    assert!(gamepkg::draw_rect(10.0,20.0,30.0,40.0,0xabcdef12));
    assert!(gamepkg::draw_circle(100.0,100.0,20.0,0x123456ff));
    assert!(gamepkg::audio_submit(&[100,-100,0]));
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
    output = run('run', 'com.gamepkg.minimal', '--frames', '2', '--rom', rom)
    assert 'draw commands: 2' in output and 'Audio samples: 3' in output, output
    assert next((root/'library/saves').glob('*.blob')).read_bytes() == b'saved'
    print('WIT source component passed: typed asset/ROM/save lists, EOF/path denial, drawing, audio and persisted saves.')
    # A trapped typed save write must leave the last committed blob intact.
    save_path = next((root/'library/saves').glob('*.blob'))
    (source/'source/src/lib.rs').write_text('''
pub fn game_tick(_: u32, _: u64) -> u64 {
    assert!(gamepkg::save_write(b"rollback"));
    assert!(false);
    0
}
''')
    manifest.write_text(manifest.read_text().replace('version = "0.1.0"','version = "0.2.0"'))
    package.unlink()
    run('pack',source,'-o',package,'--key',key); run('install',package,'--trust-creator'); run('build','com.gamepkg.minimal')
    result = subprocess.run([str(CLI),'run','com.gamepkg.minimal','--frames','1','--rom',str(rom)],env=env,capture_output=True,text=True)
    assert result.returncode != 0 and save_path.read_bytes() == b'saved', result.stderr
    # Permission changes take effect in the typed host, not only in guest wrappers.
    manifest.write_text(manifest.read_text().replace('version = "0.2.0"','version = "0.3.0"').replace('save = true','save = false').replace('audio = true','audio = false'))
    (source/'source/src/lib.rs').write_text('''
pub fn game_tick(_: u32, _: u64) -> u64 {
    assert_eq!(gamepkg::save_size(),None);
    assert!(!gamepkg::save_write(b"denied"));
    assert!(!gamepkg::audio_submit(&[100]));
    0
}
''')
    package.unlink()
    run('pack',source,'-o',package,'--key',key); run('install',package,'--trust-creator'); run('build','com.gamepkg.minimal')
    run('run','com.gamepkg.minimal','--frames','1','--rom',rom)
    assert save_path.read_bytes() == b'saved'
    print('WIT save rollback and signed audio/save permission denial passed.')
