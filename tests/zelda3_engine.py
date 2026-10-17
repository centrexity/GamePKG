"""Development proof: ROM read-only, resources only in memory, import-free Wasm."""
from pathlib import Path
import argparse, hashlib, importlib.util, json, os, subprocess, sys
PROJECT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('rom',type=Path);p.add_argument('--engine',type=Path,default=PROJECT/'target/zelda3-engine/engine.wasm');args=p.parse_args()
path=PROJECT/'engines/zelda3/adapters/rom_resources.py'
spec=importlib.util.spec_from_file_location('gamepkg_zelda_resources',path);decoder=importlib.util.module_from_spec(spec);spec.loader.exec_module(decoder)
raw=args.rom.read_bytes();before=hashlib.sha256(raw).digest()
# Deny actual disk writes/mkdir throughout the memory-only conversion.
def readonly(event,values):
    if event=='open':
        flags=values[2]
        if flags & (os.O_WRONLY|os.O_RDWR|os.O_CREAT|os.O_TRUNC|os.O_APPEND):raise AssertionError('ROM conversion attempted disk write')
    if event in ['os.mkdir','os.rename','os.remove']:raise AssertionError('ROM conversion attempted filesystem modification')
active=True
def audit(event,values):
    if active:readonly(event,values)
sys.addaudithook(audit)
try:resources=decoder.decode(raw)
finally:active=False
assert hashlib.sha256(args.rom.read_bytes()).digest()==before
code=r'''
const fs=require('fs');
const moduleBytes=fs.readFileSync(process.argv[1]);
const moduleObject=new WebAssembly.Module(moduleBytes);
if(WebAssembly.Module.imports(moduleObject).length)throw Error('Engine has ambient imports');
const e=new WebAssembly.Instance(moduleObject,{}).exports;
const resources=fs.readFileSync(0);
const ptr=e.allocate(resources.length);if(!ptr)throw Error('Allocation failed');
new Uint8Array(e.memory.buffer,ptr,resources.length).set(resources);
if(e.initialize(ptr,resources.length)!==0)throw Error('Initialization failed');
let audio=false, separated=false;
if(e.audio_channels()!==2)throw Error('Stereo channel metadata missing');
for(let frame=0;frame<1000;frame++) {
  if(e.step(0)!==0)throw Error('Step failed');
  const samples=new Int16Array(e.memory.buffer,e.audio(),1600);
  if(samples.some(value=>value!==0))audio=true;
  for(let i=0;i<samples.length;i+=2)if(samples[i]!==samples[i+1])separated=true;
}
if(!audio)throw Error('Engine never generated audio');
if(!separated)throw Error('Original stereo channels were collapsed');
const image=Buffer.from(new Uint8Array(e.memory.buffer,e.pixels(),256*224*4));
process.stdout.write(image);
'''
result=subprocess.run(['node','-e',code,str(args.engine)],input=resources,capture_output=True,timeout=60)
assert result.returncode==0,result.stderr.decode()
assert len(result.stdout)==256*224*4
from PIL import Image
image=Image.frombytes('RGBA',(256,224),result.stdout,'raw','BGRA')
assert len(image.getcolors(256*224)) > 16, 'Title screen lacks game detail'
proof=PROJECT/'target/zelda3-proof';proof.mkdir(parents=True,exist_ok=True)
image.convert('RGB').resize((512,448),Image.Resampling.NEAREST).save(proof/'engine-title.png')
print('Zelda3 engine passed: selected ROM unchanged, no resource files written, zero Wasm imports, 1000 frames, real audio and rendered image.')
