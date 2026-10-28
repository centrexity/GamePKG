"""Actual Windows source/native shader pixels without WSL or owned media.

Uses pinned per-app Mesa only in CI; it does not qualify a physical Vulkan GPU.
Python and Pillow are test tools, never player requirements.
"""
import hashlib,json,os,platform,shutil,subprocess,tempfile
from pathlib import Path
from windows_gui_support import verify_shader_input
assert platform.system()=='Windows','This fixture requires actual Windows'
PROJECT=Path(__file__).resolve().parents[1]
CLI=PROJECT/'target/release/gamepkg.exe'
with tempfile.TemporaryDirectory(prefix='gamepkg native shader GUI ') as directory:
 root=Path(directory);home=root/'library';config=root/'settings.toml'
 env=dict(os.environ,GAMEPKG_HOME=str(home),GAMEPKG_CONFIG=str(config))
 for name in ['GAMEPKG_SLANG_ROOT','GAMEPKG_DXC_ROOT','GAMEPKG_WINDOWS_RUST_ROOT','GAMEPKG_RENDERER']:
  env.pop(name,None)
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
 for backend in ['glsl','spirv','wgsl']:
  # Compile each output natively; the software OpenGL renderer validates the
  # production SPIR-V/WGSL translation as well as direct GLSL pixel output.
  config.write_text(f'automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\ntick_deadline_ms=500\nrenderer="opengl"\nshader_backend="{backend}"\n')
  run('build',game_id)
  build=home/'builds'/digest
  assert not list(build.rglob('*.wasm')),'Default native Build produced a Wasm game'
  binaries=list(build.glob('native-v1-*/game'));assert len(binaries)==1,binaries
  assert binaries[0].read_bytes().startswith(b'MZ')
  run('integrity-check',game_id)
  for launch in ['first','restart']:
   verify_shader_input(PROJECT,env,game_id,f'native-{backend}-{launch}',texture=True)
   records=list((home/'progress').glob('*.json'));assert len(records)==1,records
   played=json.loads(records[0].read_text())['played_ms']
   assert played>previous,f'Normal close did not persist additional play time: {previous} -> {played}'
   previous=played
  run('integrity-check',game_id)
  print('Verified native Windows shader texture/input/clear/resize/restart/history:',backend,flush=True)
 print('Actual Windows native game GUI checks passed; no WSL/Python player dependency, media or audible output.')
