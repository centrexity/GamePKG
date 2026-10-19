"""Exercise a dependency-free guest source through offline Cargo builds."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
PROJECT = Path(__file__).resolve().parents[1]
CLI = Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR', PROJECT/'target/release'))/'gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-approved-') as directory:
    root=Path(directory);source=root/'source'
    shutil.copytree(PROJECT/'examples/good/modular-game',source)
    original=(source/'manifest.toml').read_text()
    key=root/'creator.key'
    env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'))
    def run(*args):
        if args and args[0] in ('build','integrity-fix') and '--target' not in args: args=(*args,'--target','wasm')
        result=subprocess.run([str(CLI),*map(str,args)],env=env,capture_output=True,text=True,timeout=140)
        assert result.returncode==0,result.stderr
        return result.stdout
    run('keygen','-o',key)
    for profile in ['core-v1','component-v1']:
        (source/'manifest.toml').write_text(f'runtime_profile = "{profile}"\n'+original.replace('com.gamepkg.modular',f'com.gamepkg.modular-{profile}'))
        package=root/f'{profile}.gamepkg'
        run('pack',source,'-o',package,'--key',key)
        run('install',package,'--trust-creator')
        run('build',f'com.gamepkg.modular-{profile}')
        assert '42' in run('run',f'com.gamepkg.modular-{profile}','--frames','2')
        run('build',f'com.gamepkg.modular-{profile}')
    print('Multi-file source with explicit-endian host-independent source passed: offline locked Cargo build, core/WIT gameplay, and rebuilds.')
