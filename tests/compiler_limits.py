"""A real Rust compiler must refuse an oversized artifact and preserve prior cache output."""
import os, shutil, subprocess, tempfile
from pathlib import Path
project=Path(__file__).resolve().parents[1]
cli=project/'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-compiler-limits-') as directory:
    root=Path(directory);source=root/'source';shutil.copytree(project/'examples/minimal',source)
    key=root/'key';key.write_text(os.urandom(32).hex())
    env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'))
    def run(*args,success=True):
        if args and args[0] in ('build','integrity-fix') and '--target' not in args: args=(*args,'--target','wasm')
        result=subprocess.run([str(cli),*map(str,args)],env=env,capture_output=True,text=True,timeout=140)
        assert (result.returncode==0)==success,result.stderr
        return result
    package=root/'initial.gamepkg';run('pack',source,'-o',package,'--key',key)
    run('install',package,'--trust-creator');run('build','com.gamepkg.minimal')
    old=next((root/'library/builds').glob('*/game.wasm'));before=old.read_bytes()
    manifest=source/'manifest.toml';manifest.write_text(manifest.read_text().replace('version = "0.1.0"','version = "0.2.0"'))
    with (source/'source/src/lib.rs').open('wb') as stream:
        stream.write(b'pub fn game_tick(_: u32, state: u64) -> u64 { let data = b"')
        for _ in range(34):stream.write(b'x'*1048576)
        stream.write(b'"; let index = (state as usize) % data.len(); gamepkg::save_write(&data[index..index+1]); state }')
    package=root/'oversized.gamepkg';run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator')
    result=run('build','com.gamepkg.minimal',success=False)
    assert 'isolated Wasm build failed' in result.stderr,result.stderr
    assert old.read_bytes()==before
    assert list((root/'library/builds').glob('*/game.wasm'))==[old]
    assert len(result.stderr)<1050000,'Diagnostics exceeded the capture limit'
    print('Oversized Rust artifact refused; no partial build published and prior cache preserved.')
