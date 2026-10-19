"""Real pinned Slang builds, reflection, backends and denied host includes."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
PROJECT = Path(__file__).resolve().parents[1]
CLI = Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',PROJECT/'target/release'))/'gamepkg'
assert os.environ.get('GAMEPKG_SLANG_ROOT'), 'set GAMEPKG_SLANG_ROOT to the pinned Slang installation'
with tempfile.TemporaryDirectory(prefix='gamepkg-shaders-') as directory:
    root = Path(directory)
    source = root / 'source'
    shutil.copytree(PROJECT / 'examples/good/collect-stars', source)
    key = root / 'key'
    key.write_text(os.urandom(32).hex())
    library = root / 'library'
    config = root / 'settings.toml'
    env = dict(os.environ, GAMEPKG_HOME=str(library), GAMEPKG_CONFIG=str(config))
    def run(*args, success=True):
        if args and args[0] in ('build','integrity-fix') and '--target' not in args: args=(*args,'--target','wasm')
        result = subprocess.run([str(CLI), *map(str,args)], env=env, capture_output=True, text=True, timeout=140)
        assert (result.returncode == 0) == success, result.stderr
        return result.stdout + result.stderr
    package = root / 'good.gamepkg'
    run('pack', source, '-o', package, '--key', key)
    run('install', package, '--trust-creator')
    for backend in ['spirv', 'wgsl', 'metal', 'dxil']:
        config.write_text(f'shader_backend = "{backend}"\n')
        previous = set((library / 'shader-builds').glob('*'))
        run('build', 'com.gamepkg.collect-stars')
        new = set((library / 'shader-builds').glob('*')) - previous
        assert len(new) == 1
        cache = new.pop()
        binary = (cache / '0.shader').read_bytes()
        assert binary
        if backend == 'spirv': assert binary[:4] == bytes([3,2,35,7])
        if backend == 'dxil': assert binary[:4] == b'DXBC'
        reflection = json.loads((cache / '0.reflection.json').read_text())
        assert reflection['entryPoints'][0]['name'] == 'fragmentMain', reflection
        run('build', 'com.gamepkg.collect-stars')
        assert len(list((library / 'shader-builds').glob('*'))) == len(previous) + 1
    config.write_text('shader_backend = "dxil"\n')
    caches = set((library/'shader-builds').glob('*'))
    pinned_dxc = env.pop('GAMEPKG_DXC_ROOT')
    assert 'DXIL requires pinned DXC' in run('build','com.gamepkg.collect-stars',success=False)
    modified = root/'modified-dxc/lib'
    modified.mkdir(parents=True)
    shutil.copyfile(Path(pinned_dxc)/'lib/libdxcompiler.so',modified/'libdxcompiler.so')
    (modified/'libdxil.so').write_bytes(b'not-the-approved-library')
    env['GAMEPKG_DXC_ROOT'] = str(modified.parent)
    assert 'DXC library checksum mismatch' in run('build','com.gamepkg.collect-stars',success=False)
    assert set((library/'shader-builds').glob('*')) == caches
    env['GAMEPKG_DXC_ROOT'] = pinned_dxc
    env.pop('GAMEPKG_DXC_ROOT')
    config.write_text(f'shader_backend = "dxil"\ndxc_directory = {json.dumps(pinned_dxc)}\n')
    run('build','com.gamepkg.collect-stars')
    assert set((library/'shader-builds').glob('*')) == caches
    env['GAMEPKG_DXC_ROOT'] = str(modified.parent)
    assert 'DXC library checksum mismatch' in run('build','com.gamepkg.collect-stars',success=False)
    env['GAMEPKG_DXC_ROOT'] = pinned_dxc
    config.write_text('shader_backend = "wgsl"\n')
    manifest = source/'manifest.toml'
    manifest.write_text(manifest.read_text().replace('version = "0.1.0"','version = "0.2.0"'))
    (source/'shaders/main.slang').write_text('uniform float4 tint; [shader("fragment")] float4 fragmentMain() : SV_Target { return tint; }\n')
    unsupported=root/'resources.gamepkg'
    run('pack',source,'-o',unsupported,'--key',key)
    run('install',unsupported)
    previous_shaders=set((library/'shader-builds').glob('*'))
    previous_wasm=set((library/'builds').glob('*/game.wasm'))
    for backend in ['glsl', 'spirv', 'wgsl']:
        config.write_text(f'shader_backend = "{backend}"\n')
        assert 'resource bindings' in run('build','com.gamepkg.collect-stars',success=False)
        assert set((library/'shader-builds').glob('*')) == previous_shaders
        assert set((library/'builds').glob('*/game.wasm')) == previous_wasm
    config.write_text('shader_backend = "spirv"\n')
    manifest = source / 'manifest.toml'
    manifest.write_text(manifest.read_text().replace('version = "0.2.0"', 'version = "0.3.0"'))
    (source / 'shaders/main.slang').write_text('#include "/etc/passwd"\n')
    bad = root / 'bad.gamepkg'
    caches = set((library / 'shader-builds').glob('*'))
    assert 'shader file inclusion' in run('pack', source, '-o', bad, '--key', key, success=False)
    assert not bad.exists(), 'Rejected shader source published a package'
    assert set((library / 'shader-builds').glob('*')) == caches
    assert len(list((library / 'builds').glob('*/game.wasm'))) == 1
    print('Shaders passed: SPIR-V, WGSL, Metal, DXIL, reflection, unsupported resource rejection before cache publication, rebuild caching and host include denial.')
