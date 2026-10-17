"""Maintainer native/Wasm integration; all data lives in a private test root."""
from pathlib import Path
import os,shutil,subprocess,tempfile
project=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-2048-state-') as temporary:
 root=Path(temporary);config=root/'settings.toml';config.write_text('automatic_update_checks=false\n')
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),GAMEPKG_2048_TEST_ROOT=str(root/'library'))
 key=root/'key';key.write_text('35'*32)
 cli=project/'target/release/gamepkg'
 def run(*args):
  result=subprocess.run([str(cli),*map(str,args)],env=env,capture_output=True,text=True,timeout=120)
  assert result.returncode==0,result.stdout+result.stderr
 run('pack',project/'ports/2048','-o',root/'game.gamepkg','--key',key)
 run('install',root/'game.gamepkg','--trust-creator')
 for target in ['native','wasm']:run('build','com.gamepkg.2048','--target',target)
 denied=root/'denied';shutil.copytree(project/'ports/2048',denied)
 manifest=denied/'manifest.toml';manifest.write_text(manifest.read_text().replace('com.gamepkg.2048','com.gamepkg.2048-nosave').replace('save = true','save = false'))
 run('pack',denied,'-o',root/'denied.gamepkg','--key',key)
 run('install',root/'denied.gamepkg','--trust-creator')
 for target in ['native','wasm']:run('build','com.gamepkg.2048-nosave','--target',target)
 subprocess.run(['cargo','test','--locked','--offline','--all-features','--test','game_2048_play','--','--ignored','--test-threads=1'],cwd=project,env=env,check=True)
