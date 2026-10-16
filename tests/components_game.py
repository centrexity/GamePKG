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
    manifest.write_text(manifest.read_text() + '\n[[achievements]]\nid="component_started"\ntitle="Component started"\ndescription="Play a WIT game"\n')
    rom = root / 'selected.rom'; rom.write_bytes(b'ROM')
    (source / 'assets').mkdir(exist_ok=True)
    (source / 'assets/message.txt').write_bytes(b'ABC')
    (source / 'source/src/lib.rs').write_text('''
pub fn game_tick(_: u32, _: u64) -> u64 {
    if !(gamepkg::achievement_unlock(0)) { loop {} };
    if !(!gamepkg::achievement_unlock(9999)) { loop {} };
    let mut buffer = [0; 8];
    if !((gamepkg::asset_read("message.txt", 1, &mut buffer)) == (Some(2))) { loop {} };
    if !((&buffer[..2]) == (b"BC")) { loop {} };
    if !((gamepkg::asset_read("message.txt", 3, &mut buffer)) == (Some(0))) { loop {} };
    if !((gamepkg::asset_read("message.txt", 4, &mut buffer)) == (None)) { loop {} };
    for path in ["../manifest.toml", "/etc/passwd", "missing", "../assets/message.txt"] {
        if !((gamepkg::asset_read(path, 0, &mut buffer)) == (None)) { loop {} };
    }
    if !((gamepkg::rom_size()) == (Some(3))) { loop {} };
    if !((gamepkg::rom_read(1, &mut buffer)) == (Some(2))) { loop {} };
    if !((&buffer[..2]) == (b"OM")) { loop {} };
    if !(gamepkg::save_write(b"saved")) { loop {} };
    if !((gamepkg::save_size()) == (Some(5))) { loop {} };
    if !((gamepkg::save_read(0, &mut buffer)) == (Some(5))) { loop {} };
    if !((&buffer[..5]) == (b"saved")) { loop {} };
    if !(gamepkg::draw_clear(0x102030ff)) { loop {} };
    if !(gamepkg::draw_rect(10.0,20.0,30.0,40.0,0xabcdef12)) { loop {} };
    if !(gamepkg::draw_circle(100.0,100.0,20.0,0x123456ff)) { loop {} };
    if !(gamepkg::audio_submit(&[100,-100,0])) { loop {} };
    42u64 << 32
}
''')
    key = root / 'key'
    key.write_text(os.urandom(32).hex())
    env = dict(os.environ, GAMEPKG_HOME=str(root / 'library'), GAMEPKG_CONFIG=str(root / 'settings.toml'))
    def run(*args):
        if args and args[0] in ('build','integrity-fix') and '--target' not in args: args=(*args,'--target','wasm')
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
    if !(gamepkg::save_write(b"rollback")) { loop {} };
    if !(false) { loop {} };
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
    if !((gamepkg::save_size()) == (None)) { loop {} };
    if !(!gamepkg::save_write(b"denied")) { loop {} };
    if !(!gamepkg::audio_submit(&[100])) { loop {} };
    0
}
''')
    package.unlink()
    run('pack',source,'-o',package,'--key',key); run('install',package,'--trust-creator'); run('build','com.gamepkg.minimal')
    run('run','com.gamepkg.minimal','--frames','1','--rom',rom)
    assert save_path.read_bytes() == b'saved'
    import json
    histories=list((root/'library/progress').glob('*.json'))
    assert len(histories)==1 and 'component_started' in json.loads(histories[0].read_text())['unlocked']
    print('WIT save rollback and signed audio/save permission denial passed.')
