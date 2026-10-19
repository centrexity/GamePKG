"""Maintainer-only signed full private candidate qualification, without publication."""
from pathlib import Path
import argparse,json,os,shutil,subprocess,tempfile
parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('candidate',type=Path);args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
env=dict(os.environ,GAMEPKG_BUNDLED_AUDIO_DECODER=str(project/'tools/audio-decoder/target/wasm32-unknown-unknown/release/gamepkg_host_audio_decoder.wasm'),GAMEPKG_BUNDLED_AUDIO_CONVERTER=str(project/'tools/audio-converter/target/wasm32-unknown-unknown/release/gamepkg_host_audio_converter.wasm'))
result=subprocess.run(['cargo','test','--release','--test','arkanoo_recorded_candidate','--no-run','--message-format=json'],cwd=project,env=env,capture_output=True,text=True,timeout=180)
assert result.returncode==0,result.stderr
executable=next(json.loads(line)['executable'] for line in result.stdout.splitlines() if line.startswith('{') and json.loads(line).get('target',{}).get('name')=='arkanoo_recorded_candidate' and json.loads(line).get('executable'))
with tempfile.TemporaryDirectory(prefix='gamepkg-full-recorded-port-') as temporary:
 root=Path(temporary);(root/'key').write_text('62'*32);(root/'settings.toml').write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
 environment=dict(env,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'),GAMEPKG_RECORDED_PORT_ROOT=str(root/'library'))
 def run(*args):
  result=subprocess.run([str(project/'target/release/gamepkg'),*map(str,args)],env=environment,capture_output=True,text=True,timeout=180);assert result.returncode==0,result.stdout+'\n'+result.stderr
 for profile in ['core-v1','component-v1']:
  library=root/('library-'+profile)
  environment['GAMEPKG_HOME']=str(library);environment['GAMEPKG_RECORDED_PORT_ROOT']=str(library)
  candidate=args.candidate
  if profile=='component-v1':
   candidate=root/'component-candidate';shutil.copytree(args.candidate,candidate)
   manifest=candidate/'manifest.toml';manifest.write_text('runtime_profile="component-v1"\n'+manifest.read_text())
  archive=root/(profile+'.gamepkg')
  run('pack',candidate,'-o',archive,'--key',root/'key');run('install',archive,'--trust-creator')
  run('build','com.gamepkg.arkanoo','--target','wasm')
  if profile=='core-v1':run('build','com.gamepkg.arkanoo','--target','native')
  for native in ([False,True] if profile=='core-v1' else [False]):
   for folder in ['saves','progress']:shutil.rmtree(library/folder,ignore_errors=True)
   selected=dict(environment)
   if native:selected['GAMEPKG_RECORDED_PORT_NATIVE']='1'
   else:selected.pop('GAMEPKG_RECORDED_PORT_NATIVE',None)
   print('Full private port profile:',profile,'native:',native,flush=True)
   subprocess.run([executable,'--ignored','--nocapture'],cwd=project,env=selected,check=True,timeout=60)
print('Private complete native/core/component port music/effect controls and saved restart passed; no artifacts published.')
