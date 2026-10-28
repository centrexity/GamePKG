"""Actual Windows source/native shader pixels without WSL or owned media.

Uses pinned per-app Mesa only in CI; it does not qualify a physical Vulkan GPU.
Python and Pillow are test tools, never player requirements.
"""
import argparse,hashlib,json,os,platform,shutil,subprocess,sys,tempfile
from pathlib import Path
from windows_gui_support import verify_shader_input
assert platform.system()=='Windows','This fixture requires actual Windows'
parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--renderer',choices=['opengl','vulkan'],default='opengl');args=parser.parse_args()
PROJECT=Path(__file__).resolve().parents[1]
CLI=PROJECT/'target/release/gamepkg.exe'
sys.path.insert(0,str(PROJECT/'tools'))
from package_release import executable_architecture
architecture=executable_architecture(CLI,True)
assert architecture==os.environ.get('GAMEPKG_TEST_EXPECTED_ARCH',architecture),'CLI architecture differs from requested test'
assert executable_architecture(PROJECT/'target/release/gamepkg-gui.exe',True)==architecture,'GUI architecture differs from CLI'
assert executable_architecture(PROJECT/'target/release/opengl32.dll',True)==architecture,'Graphics DLL architecture differs from CLI'
proof=PROJECT/'target/windows-gui-proof';proof.mkdir(parents=True,exist_ok=True)
(proof/(args.renderer+'-architecture.json')).write_text(json.dumps({'app':architecture,'renderer':args.renderer,'os_machine':platform.machine(),'python_bits':64 if sys.maxsize>2**32 else 32},indent=2)+'\n')
if args.renderer=='vulkan':
 assert executable_architecture(PROJECT/'target/release/vulkan-1.dll',True)==architecture
with tempfile.TemporaryDirectory(prefix='gamepkg native shader GUI ') as directory:
 root=Path(directory);home=root/'library';config=root/'settings.toml'
 env=dict(os.environ,GAMEPKG_HOME=str(home),GAMEPKG_CONFIG=str(config))
 for name in ['GAMEPKG_SLANG_ROOT','GAMEPKG_DXC_ROOT','GAMEPKG_WINDOWS_RUST_ROOT','GAMEPKG_RENDERER']:
  env.pop(name,None)
 if args.renderer=='vulkan':
  icd=PROJECT/'target/release'/('lvp_icd.'+('x86' if architecture=='i686' else 'x86_64')+'.json')
  assert icd.is_file()
  env.update(VK_DRIVER_FILES=str(icd),VK_ICD_FILENAMES=str(icd))
  (proof/'vulkan-driver.json').write_text(json.dumps({'icd':json.loads(icd.read_text()),'icd_sha256':hashlib.sha256(icd.read_bytes()).hexdigest(),'dll_sha256':hashlib.sha256((icd.parent/'vulkan_lvp.dll').read_bytes()).hexdigest()},indent=2)+'\n')
 def run(*arguments,good=True):
  result=subprocess.run([str(CLI),*map(str,arguments)],env=env,capture_output=True,text=True,timeout=600)
  assert (result.returncode==0)==good,result.stdout+result.stderr
  return result.stdout+result.stderr
 source=root/'source';shutil.copytree(PROJECT/'examples/good/shader-switch',source)
 (source/'source/src/lib.rs').write_text('''pub fn game_tick(input:u32,state:u64)->u64 {
 if input & gamepkg::HELD_DOWN != 0 {
  if !gamepkg::draw_shader(0) { loop {} }
 } else {
  let pixels=if input & gamepkg::HELD_RIGHT != 0 { [0,255,0,255] } else { [0,0,255,255] };
  if !gamepkg::draw_shader_image(0,1,1,&pixels) { loop {} }
 }
 state+1
}''')
 shader=(PROJECT/'examples/good/shader-texture/shaders/main.slang').read_text()
 sample='return sampleImage(position.xy / float2(640, 480));'
 assert shader.count(sample)==2,'Texture fixture entries changed'
 # Expose sampled RGB and alpha as opaque output, as the Linux fixture does.
 # Otherwise transparent black blends into the game background and conceals
 # both a nonzero RGB value at alpha zero and the fallback alpha itself.
 shader=shader.replace(sample,'float4 color = sampleImage(position.xy / float2(640, 480)); return float4(color.r + color.a * 0.25, color.g, color.b, 1.0);')
 (source/'shaders/main.slang').write_text(shader)
 key=root/'creator.key';run('keygen','-o',key)
 package=root/'game.gamepkg';run('pack',source,'-o',package,'--key',key)
 game_id='com.gamepkg.shader-switch';run('install',package,'--trust-creator')
 digest=hashlib.sha256(package.read_bytes()).hexdigest();previous=0
 for backend in (['glsl','spirv','wgsl'] if args.renderer=='opengl' else ['spirv','wgsl']):
  # Compile each output natively and validate the production translation and
  # selected renderer against the same independent whole-window pixel oracle.
  config.write_text(f'automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\ntick_deadline_ms=500\nrenderer="{args.renderer}"\nshader_backend="{backend}"\n')
  run('build',game_id)
  build=home/'builds'/digest
  assert not list(build.rglob('*.wasm')),'Default native Build produced a Wasm game'
  binaries=list(build.glob('native-v1-*/game'));assert len(binaries)==1,binaries
  assert executable_architecture(binaries[0],True)==architecture,'Native game architecture differs from app'
  run('integrity-check',game_id)
  for launch in ['first','restart']:
   verify_shader_input(PROJECT,env,game_id,f'native-{args.renderer}-{backend}-{launch}',texture=True,renderer=args.renderer)
   records=list((home/'progress').glob('*.json'));assert len(records)==1,records
   played=json.loads(records[0].read_text())['played_ms']
   assert played>previous,f'Normal close did not persist additional play time: {previous} -> {played}'
   previous=played
  run('integrity-check',game_id)
  print('Verified native Windows shader texture/input/clear/resize/restart/history:',backend,args.renderer,flush=True)
 print('Verified native app, GUI, game and OpenGL DLL architecture:',architecture,flush=True)
 print('Actual Windows native game GUI checks passed; no WSL/Python player dependency, media or audible output.')
