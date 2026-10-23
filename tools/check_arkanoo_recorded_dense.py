"""Private full-audio dense-scene comparison against the active source port."""
from pathlib import Path
import argparse, hashlib, json, os, shutil, subprocess, tempfile, tomllib, zipfile

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('candidate',type=Path)
parser.add_argument('--full-save',action='store_true',help='Keep an exact 1 MiB, 5041-pattern archive resident during every scene')
args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
limits=tomllib.loads((args.candidate/'manifest.toml').read_text())['limits']
assert (limits['hard_memory_bytes'],limits['effects_memory_bytes'],limits['music_memory_bytes'])==(16777216,3145728,7340032), 'Dense qualification requires the existing 16/3/7 MiB shares'
env=dict(os.environ,
 GAMEPKG_BUNDLED_AUDIO_DECODER=str(project/'tools/audio-decoder/target/wasm32-unknown-unknown/release/gamepkg_host_audio_decoder.wasm'),
 GAMEPKG_BUNDLED_AUDIO_CONVERTER=str(project/'tools/audio-converter/target/wasm32-unknown-unknown/release/gamepkg_host_audio_converter.wasm'))
compiled=subprocess.run(['cargo','test','--release','--test','arkanoo_recorded_dense','--no-run','--message-format=json'],cwd=project,env=env,capture_output=True,text=True,timeout=180)
assert compiled.returncode==0,compiled.stderr
executable=next(value['executable'] for line in compiled.stdout.splitlines() if line.startswith('{') and (value:=json.loads(line)).get('target',{}).get('name')=='arkanoo_recorded_dense' and value.get('executable'))
from arkanoo_dense_fixture import probe

with tempfile.TemporaryDirectory(prefix='gamepkg-recorded-dense-') as temporary:
 root=Path(temporary);(root/'key').write_text('81'*32)
 (root/'settings.toml').write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
 environment=dict(env,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'),GAMEPKG_RECORDED_DENSE_ROOT=str(root/'library'))
 def run(*parts):
  result=subprocess.run([str(project/'target/release/gamepkg'),*map(str,parts)],cwd=project,env=environment,capture_output=True,text=True,timeout=180)
  assert result.returncode==0,result.stdout+'\n'+result.stderr
 failures=[]
 for profile in ['core-v1','component-v1']:
  library=root/('library-'+profile)
  environment['GAMEPKG_HOME']=str(library)
  environment['GAMEPKG_RECORDED_DENSE_ROOT']=str(library)
  for label,source in [('baseline',project/'ports/arkanoo'),('recorded',args.candidate)]:
   recipe=root/(profile+'-'+label);shutil.copytree(source,recipe)
   entry=recipe/'source/src/lib.rs';text=entry.read_text();signature='pub fn game_tick(input: u32, state: u64) -> u64 {'
   assert text.count(signature)==1
   injected=probe.replace('DENSE_EFFECT','gamepkg::effect_play("audio/bounce.wav");' if label=='recorded' else '')
   entry.write_text(text.replace(signature,signature+injected))
   manifest=recipe/'manifest.toml';manifest.write_text('runtime_profile="'+profile+'"\n'+manifest.read_text().replace('com.gamepkg.arkanoo','com.gamepkg.arkanoo-dense-'+label))
   archive=root/(profile+'-'+label+'.gamepkg')
   run('pack',recipe,'--key',root/'key','-o',archive);run('install',archive,'--trust-creator')
   if args.full_save:
    with zipfile.ZipFile(archive) as package:signed=tomllib.loads(package.read('manifest.toml').decode())
    count=(1048576-20)//208;extra=1048576-20-count*208
    blob=bytearray(b'ARK4')
    for word in [42,9,(64<<1)|(64<<10),count]:blob.extend(word.to_bytes(4,'little'))
    for index in range(count):
     name=(f'{index:04}'+('x'*extra if index+1==count else '')).encode()
     blob.extend(len(name).to_bytes(4,'little'));blob.extend(name);blob.extend(bytes([6])*200)
    assert len(blob)==1048576
    scope=f"GamePKG save v1\0{signed['id']}\0{signed['creator_key']}"
    if signed.get('save_schema',1)!=1:scope+=f"\0schema={signed['save_schema']}"
    saves=library/'saves';saves.mkdir(parents=True,exist_ok=True)
    (saves/(hashlib.sha256(scope.encode()).hexdigest()+'.blob')).write_bytes(blob)
   run('build','com.gamepkg.arkanoo-dense-'+label,'--target','wasm')
   if profile=='core-v1':run('build','com.gamepkg.arkanoo-dense-'+label,'--target','native')
  for native in ([False,True] if profile=='core-v1' else [False]):
   selected=dict(environment,GAMEPKG_RECORDED_DENSE_NATIVE=str(int(native)))
   if args.full_save:selected['GAMEPKG_RECORDED_DENSE_FULL_SAVE']='1'
   print('Dense recorded-audio comparison:',profile,'native:',native,flush=True)
   result=subprocess.run([executable,'--ignored','--nocapture'],cwd=project,env=selected,timeout=60)
   if result.returncode:failures.append((profile,native,result.returncode))
 assert not failures,f'Dense recorded-audio failures: {failures}'
print('Private complete-audio dense scenes match active-port commands and pixels; no artifacts published.')
