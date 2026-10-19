"""Successful builds export playable ticks; invalid entry points preserve caches."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
PROJECT = Path(__file__).resolve().parents[1]
CLI=PROJECT/'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-entry-') as directory:
    root=Path(directory);source=root/'source'
    shutil.copytree(PROJECT/'examples/minimal',source)
    original=(source/'manifest.toml').read_text()
    original_source=(source/'source/src/lib.rs').read_text()
    key=root/'creator.key'
    env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'))
    def run(*args,success=True):
        if args and args[0] in ('build','integrity-fix') and '--target' not in args: args=(*args,'--target','wasm')
        result=subprocess.run([str(CLI),*map(str,args)],env=env,capture_output=True,text=True,timeout=140)
        assert (result.returncode==0)==success,result.stderr
        return result
    run('keygen','-o',key)
    for profile in ['core-v1','component-v1']:
        package_id=f'com.gamepkg.entry-{profile}'
        def publish(version,text,world='game'):
            (source/'manifest.toml').write_text(f'runtime_profile = "{profile}"\n'+original.replace('com.gamepkg.minimal',package_id).replace('version = "0.1.0"',f'version = "{version}"').replace('entry_world = "game"',f'entry_world = "{world}"'))
            (source/'source/src/lib.rs').write_text(text)
            package=root/f'{profile}-{version}.gamepkg'
            run('pack',source,'-o',package,'--key',key)
            run('install',package,'--trust-creator')
            return root/'library/builds'/hashlib.sha256(package.read_bytes()).hexdigest()/'game.wasm'
        old=publish('0.1.0',original_source)
        run('build',package_id);run('run',package_id,'--frames','2')
        before=old.read_bytes()
        for version,text in [('0.2.0','pub fn tick() -> u64 { 0 }'),('0.3.0','pub fn game_tick(_: u32, _: u64) -> u32 { 0 }'),('0.4.0','pub fn game_tick(_: u32) -> u64 { 0 }')]:
            output=publish(version,text)
            run('build',package_id,success=False)
            assert not output.exists(), 'invalid entry point published a build'
            assert old.read_bytes()==before, 'failed build changed the prior cache'
        module=source/'source/src/engine.rs'
        module.write_text('pub fn game_tick(_: u32, _: u64) -> u64 { 42u64 << 32 }\n')
        output=publish('0.5.0','mod engine; pub use engine::game_tick;')
        run('build',package_id)
        assert '42' in run('run',package_id,'--frames','2').stdout
        output=publish('0.6.0',original_source,'unsupported-world')
        run('build',package_id,success=False)
        assert not output.exists()
        module.unlink()
    print('Entry points passed: playable core/WIT ticks and reexports; missing/wrong ticks and unsupported worlds rejected without cache damage.')
