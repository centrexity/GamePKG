"""An authenticated Cube 2 selector cannot smuggle ignored source or dependencies."""
from pathlib import Path
import os, shutil, subprocess,tempfile
PROJECT=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='cube2-recipe-attacks-') as temporary:
 root=Path(temporary);key=root/'test.key';key.write_text('54'*32)
 config=root/'settings.toml';config.write_text('automatic_update_checks=false\n')
 env=dict(os.environ,GAMEPKG_CONFIG=str(config),GAMEPKG_HOME=str(root/'library'))
 cases={
  'extra-rust':lambda source:(source/'source/src/helper.rs').write_text('pub fn extra() -> u32 { 7 }'),
  'extra-native':lambda source:(source/'source/engine.cpp').write_text('int main(){return 0;}'),
  'altered-launcher':lambda source:(source/'source/src/lib.rs').write_text((source/'source/src/lib.rs').read_text()+'pub fn unused() {}\n'),
  'dependency':lambda source:(source/'source/Cargo.toml').write_text((source/'source/Cargo.toml').read_text()+'\n[dependencies]\nserde="1"\n'),
  'physical-path':lambda source:(source/'manifest.toml').write_text((source/'manifest.toml').read_text().replace('[game_data]','[game_data]\npath="/home"')),
 }
 for name,attack in cases.items():
  source=root/name;shutil.copytree(PROJECT/'ports/cube2',source);attack(source)
  result=subprocess.run([str(PROJECT/'target/release/gamepkg'),'pack',str(source),'-o',str(root/(name+'.gamepkg')),'--key',str(key)],env=env,capture_output=True,text=True,timeout=15)
  assert result.returncode!=0,(name,result.stdout)
  assert not (root/(name+'.gamepkg')).exists(),name
 print('Cube 2 selector rejects extra Rust/native source, altered launcher, dependencies and manifest-supplied physical paths.')
