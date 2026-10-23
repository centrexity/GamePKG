"""Maintainer-only signed full private candidate qualification, without publication."""
from pathlib import Path
import argparse,json,os,shutil,subprocess,tempfile
parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('candidate',type=Path);parser.add_argument('--long-names',action='store_true',help='Also qualify the full-size identifier in the pattern browser and active editor');parser.add_argument('--typing',action='store_true',help='Also replace and restore the final character of the full-size identifier');parser.add_argument('--unicode-name',help='Repeat this UTF-8 sample in the maximum-size active identifier');parser.add_argument('--selector-run',action='store_true',help='Use one maximum-length run of identical variation selectors');parser.add_argument('--mixed-selector-run',action='store_true',help='Qualify one maximum-size alternating variation-selector cluster');parser.add_argument('--missing-mark-run',action='store_true',help='Qualify one maximum-size run of an unconsumed combining mark');parser.add_argument('--mixed-missing-mark-run',action='store_true',help='Qualify one maximum-size same-class alternating missing-mark cluster');parser.add_argument('--mixed-class-missing-mark-run',action='store_true',help='Qualify one maximum-size missing-mark cluster requiring canonical reordering');parser.add_argument('--late-composing-mark-run',action='store_true',help='Qualify a full-size S name with a late composing cedilla');parser.add_argument('--playlist-case',choices=['empty','invalid-utf8','invalid-paths'],help='Verify original effects remain audible with unusable packaged music');args=parser.parse_args()
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
 failures=[]
 source=args.candidate
 if args.playlist_case:
  source=root/'unusable-playlist-candidate';shutil.copytree(args.candidate,source)
  (source/'assets/audio/music-list.txt').write_bytes({'empty':b'', 'invalid-utf8':b'\xff', 'invalid-paths':b'../outside.mp3\nmusic/../outside.mp3\nmusic/file.wav\n'}[args.playlist_case])
 for profile in ['core-v1','component-v1']:
  library=root/('library-'+profile)
  environment['GAMEPKG_HOME']=str(library);environment['GAMEPKG_RECORDED_PORT_ROOT']=str(library)
  candidate=source
  if profile=='component-v1':
   candidate=root/'component-candidate';shutil.copytree(source,candidate)
   manifest=candidate/'manifest.toml';manifest.write_text('runtime_profile="component-v1"\n'+manifest.read_text())
  archive=root/(profile+'.gamepkg')
  run('pack',candidate,'-o',archive,'--key',root/'key');run('install',archive,'--trust-creator')
  run('build','com.gamepkg.arkanoo','--target','wasm')
  if profile=='core-v1':run('build','com.gamepkg.arkanoo','--target','native')
  for native in ([False,True] if profile=='core-v1' else [False]):
   for folder in ['saves','progress']:shutil.rmtree(library/folder,ignore_errors=True)
   selected=dict(environment)
   if args.long_names or args.typing:selected['GAMEPKG_RECORDED_PORT_LONG_NAMES']='1'
   if args.unicode_name:
    assert args.unicode_name
    selected['GAMEPKG_RECORDED_PORT_UNICODE']=args.unicode_name
    selected['GAMEPKG_RECORDED_PORT_LONG_NAMES']='1'
   if args.selector_run:
    selected['GAMEPKG_RECORDED_PORT_SELECTOR_RUN']='1'
    selected['GAMEPKG_RECORDED_PORT_LONG_NAMES']='1'
   if args.mixed_selector_run:
    selected['GAMEPKG_RECORDED_PORT_SELECTOR_RUN']='mixed'
    selected['GAMEPKG_RECORDED_PORT_LONG_NAMES']='1'
   if args.missing_mark_run:
    selected['GAMEPKG_RECORDED_PORT_SELECTOR_RUN']='missing'
    selected['GAMEPKG_RECORDED_PORT_LONG_NAMES']='1'
   if args.mixed_missing_mark_run:
    selected['GAMEPKG_RECORDED_PORT_SELECTOR_RUN']='mixed-missing'
    selected['GAMEPKG_RECORDED_PORT_LONG_NAMES']='1'
   if args.mixed_class_missing_mark_run:
    selected['GAMEPKG_RECORDED_PORT_SELECTOR_RUN']='mixed-class-missing'
    selected['GAMEPKG_RECORDED_PORT_LONG_NAMES']='1'
   if args.late_composing_mark_run:
    selected['GAMEPKG_RECORDED_PORT_SELECTOR_RUN']='late-composing'
    selected['GAMEPKG_RECORDED_PORT_LONG_NAMES']='1'
   if args.typing:selected['GAMEPKG_RECORDED_PORT_TYPING']='1'
   if native:selected['GAMEPKG_RECORDED_PORT_NATIVE']='1'
   else:selected.pop('GAMEPKG_RECORDED_PORT_NATIVE',None)
   print('Full private port profile:',profile,'native:',native,flush=True)
   result=subprocess.run([executable,'unusable_packaged_playlist_keeps_original_effects_on_the_host' if args.playlist_case else 'complete_port_music_controls_save_and_restart_use_real_host_output','--ignored','--nocapture'],cwd=project,env=selected,timeout=60)
   if result.returncode:failures.append((profile,native,result.returncode))
 assert not failures,f'Private port runtime failures: {failures}'
print('Private native/core/component qualification passed:', args.playlist_case or 'complete music/effect controls and saved restart', '; no artifacts published.')
