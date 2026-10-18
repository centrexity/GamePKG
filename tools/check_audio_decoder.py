"""Maintainer-only native/Chromium decoder check; never a player dependency.
Usage: python3 tools/check_audio_decoder.py /path/to/original/assets/music
Music inputs and compiled helper remain local; this script does not publish them.
"""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import argparse,json,os,shutil,subprocess,tempfile,threading
parser=argparse.ArgumentParser();parser.add_argument('music',type=Path);parser.add_argument('--sdl-reference',action='store_true');parser.add_argument('--output-rate',type=int,choices=[44100,48000]);parser.add_argument('--original-mixer',action='store_true');parser.add_argument('--mono-input',type=Path);args=parser.parse_args()
if args.original_mixer and not args.output_rate:parser.error('--original-mixer requires --output-rate')
PROJECT=Path(__file__).resolve().parents[1]
def cargo(arguments):
 p=subprocess.run(['cargo',*arguments,'--message-format=json'],cwd=PROJECT,capture_output=True,text=True,timeout=300)
 assert p.returncode==0,p.stderr
 return [json.loads(line) for line in p.stdout.splitlines() if line.startswith('{')]
artifacts=cargo(['build','--locked','--manifest-path','tools/audio-decoder/Cargo.toml','--target','wasm32-unknown-unknown','--release'])
wasm=next(Path(name) for v in artifacts if v.get('reason')=='compiler-artifact' for name in v.get('filenames',[]) if name.endswith('.wasm'))
artifacts=cargo(['test','--release','--test','audio_decoder','--no-run'])
executable=next(v['executable'] for v in artifacts if v.get('reason')=='compiler-artifact' and v.get('target',{}).get('name')=='audio_decoder' and v.get('executable'))
env=dict(os.environ,GAMEPKG_AUDIO_DECODER_WASM=str(wasm),GAMEPKG_MUSIC_INPUT_DIR=str(args.music.resolve()))
if args.output_rate:env['GAMEPKG_AUDIO_OUTPUT_RATE']=str(args.output_rate)
if args.original_mixer:env['GAMEPKG_AUDIO_ORIGINAL_MIXER']='1'
if args.mono_input:env['GAMEPKG_AUDIO_MONO_INPUT']=str(args.mono_input.resolve())
reference_temp=None
if args.sdl_reference:
 reference_temp=tempfile.TemporaryDirectory(prefix='gamepkg-decoder-reference-');reference_root=Path(reference_temp.name);env['GAMEPKG_AUDIO_REFERENCE_ROOT']=str(reference_root)
 flags=subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_mixer','sdl2'],text=True).split();subprocess.run(['cc',str(PROJECT/'tools/audio-decoder/reference.c'),'-o',str(reference_root/'decode'),*flags],check=True)
 for path in sorted(args.music.glob('*.mp3')):
  info=json.loads(subprocess.check_output(['ffprobe','-v','quiet','-show_entries','stream=sample_rate','-of','json',str(path)]));rate=info['streams'][0]['sample_rate']
  command=[str(reference_root/'decode'),str(path),str(args.output_rate)] if args.original_mixer else [str(reference_root/'decode'),str(path),rate,*([str(args.output_rate)] if args.output_rate else [])]
  with (reference_root/path.with_suffix('.s16').name).open('wb') as out:subprocess.run(command,stdout=out,check=True)
p=subprocess.run([executable,'--ignored','--nocapture'],cwd=PROJECT,env=env,capture_output=True,text=True,timeout=120)
assert p.returncode==0,p.stdout+'\n'+p.stderr
records=[json.loads(line.split('MUSIC_JSON ',1)[1]) for line in p.stdout.splitlines() if 'MUSIC_JSON ' in line];assert records
print('Native sandbox: complete streaming, chunk invariance, rewind, malformed input and bounded memory passed for',len(records),'tracks.',flush=True)
if reference_temp:
 for record in records:print('SDL comparison:',record['name'],record.get('sdl_samples'),record['samples'],record.get('sdl_different'),record.get('sdl_maximum_delta'),flush=True)
 reference_temp.cleanup()
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-music-decoder-') as temporary:
 root=Path(temporary);shutil.copyfile(wasm,root/'decoder.wasm');(root/'index.html').write_text('<!doctype html><title>Private decoder verification</title>')
 for index,record in enumerate(records):
  source=args.music/record['name'];assert source.is_file() and not source.is_symlink();record['url']=str(index)+'.mp3';shutil.copyfile(source,root/record['url'])
 if args.mono_input:
  assert args.mono_input.is_file() and not args.mono_input.is_symlink();shutil.copyfile(args.mono_input,root/'mono.mp3')
 (root/'expected.json').write_text(json.dumps(records));server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(root)));threading.Thread(target=server.serve_forever,daemon=True).start()
 options=Options();options.binary_location=shutil.which('chromium')
 for flag in ['--headless=new','--no-sandbox','--disable-dev-shm-usage']:options.add_argument(flag)
 browser=webdriver.Chrome(service=Service(shutil.which('chromedriver')),options=options);browser.set_script_timeout(120)
 try:
  browser.get(f'http://127.0.0.1:{server.server_port}/')
  result=browser.execute_async_script('''const mono=arguments[0],done=arguments[arguments.length-1];(async()=>{
   const binary=await(await fetch('decoder.wasm')).arrayBuffer(),expected=await(await fetch('expected.json')).json(),results=[];
   for(const item of expected){
    const {instance}=await WebAssembly.instantiate(binary,{}),e=instance.exports,input=await(await fetch(item.url)).arrayBuffer(),pointer=e.decoder_reserve(input.byteLength);
    if(!pointer)throw Error('input allocation');new Uint8Array(e.memory.buffer,pointer,input.byteLength).set(new Uint8Array(input));
    if(e.decoder_open()!==0)throw Error('open '+item.name);
    if(item.output_rate&&(item.original_mixer?e.decoder_set_mixer_rate(item.output_rate):e.decoder_set_output_rate(item.output_rate))!==0)throw Error('conversion '+item.name);
    if(e.decoder_rate()!==item.rate||e.decoder_channels()!==item.channels)throw Error('format '+item.name);
    for(const bad of [0,96000])if(e.decoder_set_output_rate(bad)!==-1||e.decoder_set_mixer_rate(bad)!==-1)throw Error('bad conversion rate accepted');
    for(const bad of [0,1602,...(item.channels===2?[1]:[])])if(e.decoder_read(bad)!==-1)throw Error('bad read accepted');
    let samples=0,fnv=2166136261,witness=0,calls=0;const out=e.decoder_output();
    for(;;){const count=e.decoder_read(1600);if(count<0||count>1600||count%item.channels)throw Error('invalid output');if(!count)break;
     if(calls===0&&(e.decoder_set_output_rate(44100)!==-1||e.decoder_set_mixer_rate(44100)!==-1||e.decoder_rate()!==item.rate||e.decoder_channels()!==item.channels))throw Error('midstream rate change');
     if(e.memory.buffer.byteLength>16777216||e.decoder_work()>524288)throw Error('resource budget');
     const bytes=new Uint8Array(e.memory.buffer,out,count*2);for(const byte of bytes)fnv=Math.imul(fnv^byte,16777619)>>>0;
     const view=new DataView(e.memory.buffer,out,count*2),first=Math.max(0,item.witness_offset-samples),last=Math.min(count,item.witness_offset+item.witness.length-samples);
     for(let i=first;i<last;i++){if(view.getInt16(i*2,true)!==item.witness[samples+i-item.witness_offset])throw Error('audible PCM '+item.name);witness++;}
     samples+=count;if(++calls%256===0)await new Promise(resolve=>setTimeout(resolve,0));
    }
    if(samples!==item.samples||fnv!==item.fnv32||witness!==item.witness.length)throw Error('full stream fingerprint '+item.name);
    if(e.decoder_rewind()!==0||e.decoder_read(1600)<=0)throw Error('rewind');results.push({name:item.name,samples,fnv32:fnv});
   }
   if(mono){
    const {instance}=await WebAssembly.instantiate(binary,{}),e=instance.exports,input=await(await fetch('mono.mp3')).arrayBuffer(),pointer=e.decoder_reserve(input.byteLength);
    if(!pointer)throw Error('mono allocation');new Uint8Array(e.memory.buffer,pointer,input.byteLength).set(new Uint8Array(input));
    if(e.decoder_open()!==0||e.decoder_channels()!==1)throw Error('mono format');
    const rate=e.decoder_rate();for(const target of [44100,48000])if(e.decoder_set_mixer_rate(target)!==-1||e.decoder_channels()!==1||e.decoder_rate()!==rate)throw Error('mono mixer rejection');
    if(e.decoder_set_output_rate(48000)!==0||e.decoder_channels()!==2)throw Error('mono global conversion');
    let heard=false,ended=false;
    for(let call=0;call<1000;call++){
     const count=e.decoder_read(1600);if(count<0||count%2)throw Error('mono output');if(!count){ended=true;break;}
     const pcm=new DataView(e.memory.buffer,e.decoder_output(),count*2);
     for(let i=0;i<count;i+=2){const left=pcm.getInt16(i*2,true);heard||=left!==0;if(left!==pcm.getInt16((i+1)*2,true))throw Error('mono duplication');}
    }
    if(!heard||!ended)throw Error('mono audible ending');
   }return results;
  })().then(results=>done({results})).catch(error=>done({error:String(error)}));''',bool(args.mono_input))
  assert 'error' not in result,result
  assert len(result['results'])==len(records)
  for record in result['results']:print('Chromium matches native: '+record['name']+'; '+str(record['samples'])+' sample values.',flush=True)
 finally:browser.quit();server.shutdown();server.server_close()
