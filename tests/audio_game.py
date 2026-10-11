"""Signed audio source -> isolated compilers -> bounded Wasm PCM submission."""
import os
from pathlib import Path
import subprocess
import tempfile
PROJECT=Path(__file__).resolve().parents[1]
CLI=PROJECT/'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-audio-') as directory:
    root=Path(directory)
    key=root/'key';key.write_text(os.urandom(32).hex())
    env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'))
    def run(*args):
        result=subprocess.run([str(CLI),*map(str,args)],env=env,capture_output=True,text=True,timeout=140)
        assert result.returncode==0,result.stderr
        return result.stdout
    package=root/'game.gamepkg'
    run('pack',PROJECT/'examples/good/tone-pad','-o',package,'--key',key)
    run('install',package,'--trust-creator');run('build','com.gamepkg.tone-pad')
    output=run('run','com.gamepkg.tone-pad','--frames','120')
    assert 'Audio samples: 768; peak: 1000' in output,output
    print('Guest audio passed: signed source, isolated compilers, 120 ticks of bounded PCM.')
