"""Exercise every advertised good and bad example through the actual CLI."""
from pathlib import Path
import os, subprocess, tempfile, tomllib
project = Path(__file__).resolve().parents[1]
cli = Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',project/'target/release'))/'gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-examples-') as directory:
    root = Path(directory)
    env = dict(os.environ, GAMEPKG_HOME=str(root/'library'), GAMEPKG_CONFIG=str(root/'settings.toml'))
    (root/'settings.toml').write_text('automatic_update_checks = false\n')
    def run(*args, success=True, reason=None):
        result = subprocess.run([str(cli), *map(str,args)],env=env,capture_output=True,text=True,timeout=150)
        assert (result.returncode == 0) == success, f'{args}: {result.stderr}'
        if reason is not None: assert reason in result.stderr, result.stderr
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
    expected_rejections={'filesystem':'forbidden construct','network':'forbidden construct','include-bytes':'forbidden construct','build-script':'Rust source files','proc-macro':'library configuration','unknown-crate':'guest crates','non-slang':'only Slang'}
    bad_examples=sorted(path for path in (project/'examples/bad').iterdir() if path.is_dir())
    assert {path.name for path in bad_examples}==set(expected_rejections), 'Declare an expected rejection reason for every bad example'
    for source in bad_examples:
        output=root/(source.name+'-rejected.gamepkg')
        run('pack',source,'-o',output,'--key',key,success=False,reason=expected_rejections[source.name])
        assert not output.exists() and not output.with_suffix('.gamepkg.partial').exists(), 'Rejected input published package data'
        print(f'{source.name}: rejected for the expected reason without publishing',flush=True)
    print(f'All {len(examples)} good and {len(bad_examples)} bad examples passed.')
