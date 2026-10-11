"""Creator directory builds publish signed source only after isolated compilation."""
from pathlib import Path
import os, shutil, subprocess, tempfile
project=Path(__file__).resolve().parents[1]
cli=project/'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-creator-build-') as directory:
    root=Path(directory);source=root/'source';shutil.copytree(project/'examples/minimal',source)
    env=dict(os.environ,GAMEPKG_HOME=str(root/'installed'),GAMEPKG_CONFIG=str(root/'settings.toml'))
    def run(*args,success=True):
        result=subprocess.run([str(cli),*map(str,args)],env=env,capture_output=True,text=True,timeout=150)
        assert (result.returncode==0)==success,result.stderr
        return result.stdout
    key=root/'creator.key';run('keygen','-o',key)
    alias=root/'source-alias';alias.symlink_to(source,target_is_directory=True)
    dangling=root/'existing-symlink.gamepkg';dangling.symlink_to(root/'missing-target')
    for output in [source/'inside.gamepkg',alias/'inside.gamepkg',root/'missing-parent/game.gamepkg',dangling]:
        run('build',source,'-o',output,'--key',key,success=False)
        assert not output.exists()
    assert dangling.is_symlink(), 'existing symlink was replaced'
    assert not (source/'inside.gamepkg').exists(), 'source directory was polluted'
    original=(source/'manifest.toml').read_text()
    for profile in ['core-v1','component-v1']:
        (source/'manifest.toml').write_text(f'runtime_profile = "{profile}"\n'+original)
        output=root/(profile+'.gamepkg')
        run('build',source,'-o',output,success=False)
        assert not output.exists()
        run('build',source,'-o',output,'--key',key)
        run('verify',output)
        assert not (root/'installed').exists(), 'creator build installed into the end-user library'
        before=output.read_bytes()
        run('build',source,'-o',output,'--key',key,success=False)
        assert output.read_bytes()==before
    (source/'source/src/lib.rs').write_text('pub fn wrong_entry() {}')
    failed=root/'failed.gamepkg'
    run('build',source,'-o',failed,'--key',key,success=False)
    assert not failed.exists(), 'failed compilation published a package'
    run('install',root/'core-v1.gamepkg','--trust-creator')
    run('build','com.gamepkg.minimal')
    assert 'Ran 3 frames' in run('run','com.gamepkg.minimal','--frames','3')
    print('Creator build passed: core/WIT compilation, signature verification, no implicit install, required key, no overwrite, failed-build cleanup, and installed-ID compatibility.')
