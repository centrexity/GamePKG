"""Isolated shader playback, exact fallback sampling, recovery and cache validation."""
from gui_test_support import start_display
import os, subprocess, tempfile, time, argparse, shutil, hashlib, json
from pathlib import Path
from PIL import Image
parser=argparse.ArgumentParser();parser.add_argument('--wrapped',action='store_true');parser.add_argument('--component',action='store_true');parser.add_argument('--texture',action='store_true');parser.add_argument('--backend',choices=['glsl','spirv','wgsl']);parser.add_argument('--without-compiler',action='store_true');parser.add_argument('--native',action='store_true');args=parser.parse_args()
if args.native and args.component:parser.error('native execution uses core-v1')
if args.wrapped: args.texture=True
project=Path(__file__).resolve().parents[1]
os.chdir(project)
binaries=Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',project/'target/release'))
with tempfile.TemporaryDirectory(prefix='gamepkg-shader-ui-') as directory:
 root=Path(directory)
 env=dict(os.environ,DISPLAY=':97',LIBGL_ALWAYS_SOFTWARE='1',GAMEPKG_CONFIG=str(root/'settings.toml'),GAMEPKG_HOME=str(root/'library'),GAMEPKG_SLANG_ROOT=os.environ.get('GAMEPKG_SLANG_ROOT',''));env.pop('WAYLAND_DISPLAY',None)
 # No backend setting by default: first-launch defaults must support desktop playback.
 (root/'settings.toml').write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
 if args.backend: (root/'settings.toml').write_text((root/'settings.toml').read_text()+f'shader_backend = "{args.backend}"\nrenderer = "'+('opengl' if args.backend=='glsl' else 'vulkan')+'"\n')
 def run(*arguments):
  if arguments and arguments[0] in ('build','integrity-fix') and '--target' not in arguments: arguments=(*arguments,'--target','native' if args.native else 'wasm')
  result=subprocess.run([str(binaries/'gamepkg'),*map(str,arguments)],env=env,capture_output=True,text=True,timeout=140)
  assert result.returncode==0,result.stderr
 key=root/'key';key.write_text(os.urandom(32).hex())
 source=project/'examples/good/shader-switch'
 if args.component or args.texture:
  source=root/'source';shutil.copytree(project/'examples/good/shader-switch',source)
  if args.component:
   manifest=source/'manifest.toml';manifest.write_text('runtime_profile = "component-v1"\n'+manifest.read_text())
 if args.texture:
  (source/'source/src/lib.rs').write_text('pub fn game_tick(input:u32,state:u64)->u64 { if input & gamepkg::HELD_DOWN != 0 { if !(gamepkg::draw_shader(0)) { loop {} }; return state+1; } let pixels=if input & gamepkg::HELD_RIGHT != 0 { [0,255,0,255] } else { [0,0,255,255] }; if !(!gamepkg::draw_shader_image(99,1,1,&pixels)) { loop {} }; if !(!gamepkg::draw_shader_image(0,513,1,&pixels)) { loop {} }; if !(!gamepkg::draw_shader_image(0,1,1,&pixels[..3])) { loop {} }; if !(gamepkg::draw_shader_image(0,1,1,&pixels)) { loop {} }; state+1 }')
  (source/'shaders/main.slang').write_text((project/'examples/good/shader-texture/shaders/main.slang').read_text())
 if args.wrapped:
  shutil.copy2(project/'examples/good/media-game/assets/texture.png',source/'assets/texture.png')
  (source/'source/src/lib.rs').write_text('pub fn game_tick(input:u32,state:u64)->u64 { let Some(shader)=gamepkg::Shader::from_slang("shaders/main.slang","blueMain") else {loop {}}; let ok=if input & gamepkg::HELD_DOWN != 0 { shader.draw() } else if input & gamepkg::HELD_RIGHT != 0 { shader.draw_rgba(1,1,&[0,255,0,255]) } else { shader.draw_texture("texture.png") }; if !ok {loop {}}; state+1 }')
 if args.texture:
  shader=source/'shaders/main.slang'
  # Expose sampled RGB and alpha through opaque output: a white fallback must
  # fail even though it contains neither the previous blue nor green texture.
  shader.write_text(shader.read_text().replace('return sampleImage(position.xy / float2(640, 480));','float4 color = sampleImage(position.xy / float2(640, 480)); return float4(color.r + color.a * 0.25, color.g, color.b, 1.0);'))
 run('pack',source,'-o',root/'game.gamepkg','--key',key)
 run('install',root/'game.gamepkg','--trust-creator');run('build','com.gamepkg.shader-switch')
 display=start_display(env)
 try:
  player_env=dict(env)
  if args.without_compiler:player_env.pop('GAMEPKG_SLANG_ROOT',None)
  app=subprocess.Popen([str(binaries/'gamepkg-gui'),'--play','com.gamepkg.shader-switch'],env=player_env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  try:
   for _ in range(100):
    if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
    state=subprocess.run(['xdotool','search','--onlyvisible','--name',r'^(GamePKG|com\..*)$'],env=env,capture_output=True,text=True)
    if state.returncode==0:break
    time.sleep(.1)
   window=state.stdout.strip().splitlines()[0]
   subprocess.run(['xdotool','windowfocus',window],env=env,check=True)
   time.sleep(.5)
   before=root/'before.png'
   subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(before)],env=env,check=True)
   def dominant(path, channel):
    image=Image.open(path).convert('RGB')
    points=sum(1 for rgb in (image.getpixel((x,y)) for y in range(image.height) for x in range(image.width)) if rgb[channel]>150 and rgb[channel]>2*rgb[1 if channel==2 else 2])
    return points>20000
   deadline=time.monotonic()+15
   while not dominant(before,2):
    assert time.monotonic()<deadline, 'Package blue shader pixels did not appear'
    if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
    time.sleep(.1)
    subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(before)],env=env,check=True)
   subprocess.run(['xdotool','keydown','Right'],env=env,check=True);time.sleep(.5)
   after=root/'after.png'
   subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(after)],env=env,check=True)
   assert dominant(after,1), 'Package green shader pixels missing'
   subprocess.run(['xdotool','keyup','Right'],env=env,check=True)
   if args.texture:
    subprocess.run(['xdotool','keydown','Down'],env=env,check=True);time.sleep(.5)
    subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(after)],env=env,check=True)
    deadline=time.monotonic()+10
    while True:
     image=Image.open(after).convert('RGB');samples=[image.getpixel((image.width*x//4,image.height*y//4)) for x in [1,2,3] for y in [1,2,3]]
     if all(sample==(0,0,0) for sample in samples):break
     assert time.monotonic()<deadline, f'Plain shader must sample transparent black, got {samples}'
     if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
     time.sleep(.1);subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(after)],env=env,check=True)
    subprocess.run(['xdotool','keyup','Down'],env=env,check=True)
    deadline=time.monotonic()+10
    while True:
     subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(after)],env=env,check=True)
     if dominant(after,2):break
     assert time.monotonic()<deadline,'Texture did not recover after plain shader draw'
     if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
     time.sleep(.1)
   shutil.copy2(after,project/'target'/f'shader-player-{args.backend}-{args.component}-{args.texture}.png')
   print(f"Shader playback passed: {'native' if args.native else 'wasm'}, {'component-v1' if args.component else 'core-v1'}, {args.backend or 'default'}; texture={args.texture}, wrapped={args.wrapped}.")
   if args.texture:print('Nine transparent-black fallback samples and recovery after held input passed.')
  finally:
   app.terminate();app.wait(timeout=5)
 finally:
  display.terminate();display.wait(timeout=5)

 # Resource declarations and missing reflection must fail before graphics setup.
 metadata=next((root/'library/shader-builds').glob('*/0.reflection.json'))
 original=metadata.read_bytes()
 receipt=next((root/'library/builds').glob('*/shaders-*.json'))
 original_receipt=receipt.read_bytes()
 for changes in [{'format_version':2},{'package_digest':'0'*64},{'backend':'wrong'},{'cache_digest':'../escape'},{'unexpected':True}]:
  record=dict(json.loads(original_receipt),**changes);receipt.write_text(json.dumps(record))
  result=subprocess.run([str(binaries/'gamepkg'),'run','com.gamepkg.shader-switch','--frames','1'],env=env,capture_output=True,text=True,timeout=10)
  assert result.returncode!=0,result.stdout
 receipt.write_bytes(original_receipt)

 def update_receipt(path):
  record=json.loads(receipt.read_text());record['files'][path.name]=hashlib.sha256(path.read_bytes()).hexdigest();receipt.write_text(json.dumps(record))

 parsed=json.loads(original)
 mismatched=dict(parsed,entryPoints=[{'name':'not-the-signed-entry'}])
 missing_entries=dict(parsed);missing_entries.pop('entryPoints',None)
 wrong_stage=dict(parsed,entryPoints=[dict(parsed['entryPoints'][0],stage='compute')])
 missing_stage=dict(parsed,entryPoints=[{k:v for k,v in parsed['entryPoints'][0].items() if k!='stage'}])
 for reflection in [{'parameters':[{'name':'untrustedTexture'}]}, {}, mismatched, missing_entries, wrong_stage, missing_stage]:
  metadata.write_text(json.dumps(reflection))
  result=subprocess.run([str(binaries/'gamepkg'),'run','com.gamepkg.shader-switch','--frames','1'],env=env,capture_output=True,text=True)
  assert result.returncode!=0 and 'checksum mismatch' in result.stderr,result.stderr
  update_receipt(metadata)

  result=subprocess.run([str(binaries/'gamepkg'),'run','com.gamepkg.shader-switch','--frames','1'],env=env,capture_output=True,text=True)
  assert result.returncode!=0 and ('resource' in result.stderr or 'reflection' in result.stderr), result.stderr
 metadata.write_bytes(original);receipt.write_bytes(original_receipt)
 print('External resources, missing reflection, and entry-point and stage mismatches rejected before playback.')

 if args.backend in ('spirv','wgsl'):
  binary=metadata.with_name('0.shader');original_binary=binary.read_bytes()
  binary.write_bytes(b'not SPIR-V');update_receipt(binary)
  result=subprocess.run([str(binaries/'gamepkg'),'run','com.gamepkg.shader-switch','--frames','1'],env=env,capture_output=True,text=True)
  assert result.returncode!=0 and ('SPIR-V' in result.stderr or 'WGSL' in result.stderr), result.stderr
  binary.write_bytes(original_binary);receipt.write_bytes(original_receipt)
  print('Malformed cached '+args.backend+' rejected before playback.')
