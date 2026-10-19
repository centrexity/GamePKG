"""Signed source guest generates its own frames under the Wasm sandbox."""
import os
from pathlib import Path
import subprocess
import tempfile
PROJECT = Path(__file__).resolve().parents[1]
CLI = PROJECT / 'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-drawing-') as directory:
    root = Path(directory)
    key = root / 'key'
    key.write_text(os.urandom(32).hex())
    env = dict(os.environ, GAMEPKG_HOME=str(root/'library'), GAMEPKG_CONFIG=str(root/'settings.toml'))
    def run(*args):
        if args and args[0] in ('build','integrity-fix') and '--target' not in args: args=(*args,'--target','wasm')
        result = subprocess.run([str(CLI),*map(str,args)],env=env,capture_output=True,text=True,timeout=140)
        assert result.returncode==0,result.stderr
        return result.stdout
    package=root/'game.gamepkg'
    run('pack',PROJECT/'examples/good/shape-playground','-o',package,'--key',key)
    run('install',package,'--trust-creator')
    run('build','com.gamepkg.shape-playground')
    assert 'Ran 120 frames' in run('run','com.gamepkg.shape-playground','--frames','120')
    print('Guest drawing passed: signed source, isolated Rust/Slang build, 120 bounded frames.')
