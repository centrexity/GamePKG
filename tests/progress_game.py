"""Verify signed, locally compiled achievement guests in both runtime profiles."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
PROJECT=Path(__file__).resolve().parents[1]
CLI=PROJECT/'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-progress-') as directory:
    root=Path(directory)
    for profile in ['core-v1','component-v1']:
        home=root/profile
        source=home/'source'
        shutil.copytree(PROJECT/'examples/minimal',source)
        manifest=source/'manifest.toml'
        manifest.write_text(f'runtime_profile="{profile}"\n'+manifest.read_text()+'\n[[achievements]]\nid="started"\ntitle="Started"\ndescription="Play this game"\n')
        (source/'source/src/lib.rs').write_text('''pub fn game_tick(_:u32,state:u64)->u64 {
            assert!(gamepkg::achievement_unlock(0));
            assert!(!gamepkg::achievement_unlock(9999));
            state + 1
        }''')
        env=dict(os.environ,GAMEPKG_HOME=str(home/'library'),GAMEPKG_CONFIG=str(home/'settings.toml'))
        def run(*args):
            result=subprocess.run([str(CLI),*map(str,args)],env=env,capture_output=True,text=True,timeout=90)
            assert result.returncode==0,result.stderr
            return result.stdout
        key=home/'creator.key';run('keygen','-o',key)
        package=home/'game.gamepkg'
        run('pack',source,'-o',package,'--key',key)
        run('install',package,'--trust-creator');run('build','com.gamepkg.minimal')
        run('run','com.gamepkg.minimal','--frames','3')
        record=next((home/'library/progress').glob('*.json'))
        first=json.loads(record.read_text())
        assert list(first['unlocked'])==['started']
        run('run','com.gamepkg.minimal','--frames','2')
        second=json.loads(record.read_text())
        assert second['unlocked']==first['unlocked'] and second['played_ms']>=first['played_ms']
        text=run('list')
        assert '[Unlocked] Started' in text and 'achievements: 1/1' in text and 'package:' in text and 'build:' in text,text
        print(profile+': signed source compiled, achievement unlocked once, session history persisted, storage shown.')
