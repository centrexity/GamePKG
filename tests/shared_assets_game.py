"""Exercise signed store packages and large shared chunk reads in both runtimes."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
PROJECT = Path(__file__).resolve().parents[1]
CLI = PROJECT / 'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-shared-') as directory:
    root = Path(directory)
    source = root / 'source'
    shutil.copytree(PROJECT / 'examples/minimal', source)
    data = source / 'assets/large.bin'
    data.write_bytes(b'0' * (16 * 1024 * 1024 - 3) + b'ABC')
    (source / 'source/src/lib.rs').write_text('''pub fn game_tick(_: u32, _: u64) -> u64 {
        let mut buffer = [0; 8];
        assert_eq!(gamepkg::asset_read("large.bin", 16777213, &mut buffer), Some(3));
        assert_eq!(&buffer[..3], b"ABC");
        assert_eq!(gamepkg::asset_read("large.bin", 16777216, &mut buffer), Some(0));
        assert_eq!(gamepkg::asset_read("large.bin", 16777217, &mut buffer), None);
        assert_eq!(gamepkg::asset_read("../large.bin", 0, &mut buffer), None);
        42u64 << 32
    }''')
    key = root / 'creator.key'
    original = (source / 'manifest.toml').read_text()
    env = dict(os.environ, GAMEPKG_HOME=str(root/'library'), GAMEPKG_CONFIG=str(root/'settings.toml'))
    def run(*args, success=True, environment=env):
        result = subprocess.run([str(CLI), *map(str,args)],env=environment,capture_output=True,text=True,timeout=120)
        assert (result.returncode == 0) == success, result.stderr
        return result.stdout
    run('keygen','-o',key)
    for profile in ['core-v1','component-v1']:
        (source/'manifest.toml').write_text(f'runtime_profile = "{profile}"\n'+original.replace('com.gamepkg.minimal',f'com.gamepkg.{profile}'))
        package = root / f'{profile}.gamepkg'
        run('pack',source,'-o',package,'--key',key,'--shared-assets')
        assert package.stat().st_size < 1024 * 1024, 'assets remained embedded'
        run('verify',package)
        missing = dict(env,GAMEPKG_HOME=str(root/'missing-library'))
        run('install',package,'--trust-creator',success=False,environment=missing)
        run('install',package,'--trust-creator')
        run('build',f'com.gamepkg.{profile}')
        output=run('run',f'com.gamepkg.{profile}','--frames','2')
        assert '42' in output, output
    assert len(list((root/'library/asset-pool').glob('*.blob'))) == 2
    print('Shared assets passed: signed thin packages, missing-blob refusal, deduplication, and large range reads in core and WIT games.')
