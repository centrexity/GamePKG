"""Every advertised good package must install, compile, and tick from signed source."""
from pathlib import Path
import os, subprocess, tempfile, tomllib
project = Path(__file__).resolve().parents[1]
cli = project/'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-examples-') as directory:
    root = Path(directory)
    env = dict(os.environ, GAMEPKG_HOME=str(root/'library'), GAMEPKG_CONFIG=str(root/'settings.toml'))
    (root/'settings.toml').write_text('shader_backend = "glsl"\nautomatic_update_checks = false\n')
    def run(*args, success=True):
        result = subprocess.run([str(cli), *map(str,args)],env=env,capture_output=True,text=True,timeout=150)
        assert (result.returncode == 0) == success, f'{args}: {result.stderr}'
        return result.stdout
    key=root/'creator.key';rom=root/'user.rom';rom.write_bytes(bytes([2,3,4,5]))
    run('keygen','-o',key)
    examples=[project/'examples/minimal', *sorted((project/'examples/good').iterdir())]
    for source in examples:
        manifest=tomllib.loads((source/'manifest.toml').read_text())
        id=manifest['id'];package=root/(source.name+'.gamepkg')
        run('pack',source,'-o',package,'--key',key)
        run('verify',package)
        run('install',package,'--trust-creator')
        run('build',id)
        args=['run',id,'--frames','3']
        if manifest.get('requires_rom',False):
            run(*args,success=False)
            args += ['--rom',str(rom)]
        output=run(*args)
        assert 'Ran 3 frames' in output, output
        expected={'counter':'score: 3','asset-catalog':'score: 1','save-counter':'score: 3','byteorder-game':'score: 42','modular-game':'score: 42','rom-required':'score: 4'}
        if source.name in expected:assert expected[source.name] in output, output
        if manifest.get('permissions',{}).get('audio',False):assert 'Audio samples:' in output, output
        print(f'{source.name}: signed, verified, built, and played',flush=True)
    print(f'All {len(examples)} good examples passed.')
