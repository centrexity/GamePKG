"""Private actual-browser dense scenes with maximum saves and original audio."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import argparse, os, shutil, subprocess, tempfile, threading
from PIL import Image
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.support.ui import WebDriverWait
from arkanoo_dense_fixture import probe

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('candidate', type=Path)
parser.add_argument('--optimized-images', type=Path, required=True)
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]

class Quiet(SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass

with tempfile.TemporaryDirectory(prefix='gamepkg-recorded-dense-web-') as temporary:
    root = Path(temporary)
    (root/'key').write_text('83'*32)
    (root/'settings.toml').write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
    env = dict(os.environ, GAMEPKG_HOME=str(root/'library'), GAMEPKG_CONFIG=str(root/'settings.toml'))
    def run(*parts):
        result = subprocess.run([str(project/'target/release/gamepkg'), *map(str, parts)], env=env, capture_output=True, text=True, timeout=180)
        assert result.returncode == 0, result.stdout+'\n'+result.stderr
    for profile in ['core-v1', 'component-v1']:
        references = None
        for label, source in [('baseline', project/'ports/arkanoo'), ('recorded', args.candidate)]:
            recipe = root/(profile+'-'+label)
            shutil.copytree(source, recipe)
            entry = recipe/'source/src/lib.rs'
            text = entry.read_text()
            signature = 'pub fn game_tick(input: u32, state: u64) -> u64 {'
            assert text.count(signature) == 1
            # Repeated browser ticks retain a fixed scene. Play once on entry,
            # preserving the real four-voice limit rather than filling it artificially.
            counter = '((game.editor.patterns.len() as u64) << 32) | game.particles.len() as u64 | ((game.level as u64) << 16)'
            fixture = probe.replace('((game.editor.patterns.len() as u64) << 32) | game.particles.len() as u64', counter)
            effect = 'if state != ('+counter+') { gamepkg::effect_play("audio/bounce.wav"); }' if label == 'recorded' else ''
            entry.write_text(text.replace(signature, signature+fixture.replace('DENSE_EFFECT', effect)))
            for path in (project/'ports/arkanoo/assets').glob('*.png'):
                replacement = args.optimized_images/path.name
                with Image.open(path) as original, Image.open(replacement) as optimized:
                    assert original.size == optimized.size and original.convert('RGBA').tobytes() == optimized.convert('RGBA').tobytes(), path.name
                shutil.copy2(replacement, recipe/'assets'/path.name)
            manifest = recipe/'manifest.toml'
            game_id = 'com.gamepkg.arkanoo-dense-web-'+profile+'-'+label
            manifest.write_text('runtime_profile="'+profile+'"\n'+manifest.read_text().replace('com.gamepkg.arkanoo', game_id))
            archive = root/(profile+'-'+label+'.gamepkg')
            run('pack', recipe, '-o', archive, '--key', root/'key')
            run('install', archive, '--trust-creator')
            run('build', game_id, '--target', 'wasm')
            web = root/(profile+'-'+label+'-web')
            run('web-export', game_id, '-o', web)
            server = ThreadingHTTPServer(('127.0.0.1', 0), partial(Quiet, directory=str(web)))
            threading.Thread(target=server.serve_forever, daemon=True).start()
            options = Options()
            options.binary_location = shutil.which('google-chrome') or shutil.which('chromium') or shutil.which('google-chrome-stable')
            for flag in ['--headless=new', '--no-sandbox', '--disable-dev-shm-usage', '--autoplay-policy=no-user-gesture-required']:
                options.add_argument(flag)
            options.set_capability('goog:loggingPrefs', {'browser': 'ALL'})
            driver = shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER', '/nonexistent'))/'chromedriver')
            browser = webdriver.Chrome(service=Service(driver), options=options)
            browser.execute_cdp_cmd('Page.addScriptToEvaluateOnNewDocument', {'source': '''
                window.scene=0;window.results={};window.pending={};const Base=Worker;
                window.Worker=class extends Base{
                  constructor(...args){super(...args);this.addEventListener('message',({data})=>{
                    if(data.type!=='frame')return;window.lastState=data.state;
                    const scene=Number((BigInt(data.state)>>16n)&255n);
                    if(!scene||window.results[scene]||window.pending[scene])return;
                    window.pending[scene]=true;
                    (async()=>{let bytes=0;const commands=[];
                      for(const command of data.commands){const row=[];for(const item of command){
                        if(ArrayBuffer.isView(item)){bytes+=item.byteLength;
                          const hash=await crypto.subtle.digest('SHA-256',new Uint8Array(item.buffer,item.byteOffset,item.byteLength));
                          row.push({bytes:item.byteLength,sha256:Array.from(new Uint8Array(hash),v=>v.toString(16).padStart(2,'0')).join('')});
                        }else row.push(item);
                      }commands.push(row);}
                      window.results[scene]={background:data.background,state:data.state,bytes,commands};
                    })().catch(error=>window.probeError=String(error));
                  });}
                  postMessage(data,...args){if(data.type==='tick'&&window.scene)data={...data,input:0x9f000000+window.scene,events:[]};return super.postMessage(data,...args);}
                };
            '''})
            try:
                browser.get(f'http://127.0.0.1:{server.server_port}/')
                wait = WebDriverWait(browser, 20)
                wait.until(lambda b: b.find_element('id', 'status').text == 'Ready to play')
                hooked = browser.execute_async_script('''const done=arguments[arguments.length-1];Promise.all([import('./progress-store.js'),arguments[0]?import('./recorded-session.js'):Promise.resolve({})]).then(([{ProgressStore},{RecordedSession}])=>{
                  const bytes=new Uint8Array(1048576),view=new DataView(bytes.buffer);bytes.set(new TextEncoder().encode('ARK4'));
                  for(const [offset,value] of [[4,42],[8,9],[12,(64<<1)|(64<<10)],[16,5041]])view.setUint32(offset,value,true);
                  let offset=20;for(let i=0;i<5041;i++){const name=String(i).padStart(4,'0')+(i===5040?'x'.repeat(28):'');view.setUint32(offset,name.length,true);offset+=4;bytes.set(new TextEncoder().encode(name),offset);offset+=name.length;bytes.fill(6,offset,offset+200);offset+=200;}
                  if(offset!==bytes.length)throw Error('fixture size');
                  const load=ProgressStore.prototype.load;ProgressStore.prototype.load=async function(...args){return {...await load.apply(this,args),save:bytes};};
                  window.roots=[];if(RecordedSession){const open=RecordedSession.open,snapshot=RecordedSession.prototype.snapshot;
                  RecordedSession.open=async(...args)=>{window.roots.push(args[4]);const session=await open(...args);window.guestBytes=session.guestBytes;return session;};
                  RecordedSession.prototype.snapshot=async function(){const value=await snapshot.call(this);window.lastAudio=value;if(value.activeEffects)window.sawEffect=true;return value;};}done(true);
                }).catch(error=>done(String(error)));''', label == 'recorded')
                assert hooked is True, hooked
                browser.find_element('id', 'play').click()
                if label == 'recorded':
                    wait.until(lambda b: b.execute_script('return window.lastAudio?.musicStatus==="playing"'))
                    assert browser.execute_script('return window.guestBytes') == 3891008
                    assert browser.execute_script('return window.roots.at(-1).used') == 16777216
                wait.until(lambda b: b.execute_script('return window.lastState!==undefined'))
                results = []
                for scene in range(1, 7):
                    browser.execute_script('window.scene=arguments[0]', scene)
                    wait.until(lambda b: b.execute_script('return Boolean(window.results[arguments[0]])', scene))
                    result = browser.execute_script('return window.results[arguments[0]]', scene)
                    assert browser.find_element('id', 'status').text == 'Playing'
                    assert int(result['state']) == (5041 << 32) | 3000 | (scene << 16), result['state']
                    assert len(result['commands']) == 234 and result['bytes'] == 7896124, (scene, result['bytes'], len(result['commands']))
                    if label == 'recorded':
                        assert browser.execute_script('return window.lastAudio.musicStatus') == 'playing'
                        assert browser.execute_script('return window.roots.at(-1).used') == 16777216
                        assert result == references[scene-1], f'{profile} scene {scene} changed commands or pixels'
                    results.append(result)
                    print(profile, label, 'maximum archive plus dense scene', scene, 'passed', flush=True)
                if label == 'recorded':
                    wait.until(lambda b: b.execute_script('return window.sawEffect===true'))
                browser.find_element('id', 'stop').click()
                wait.until(lambda b: b.execute_script('return window.roots.every(root=>root.used===0)'))
                if label == 'baseline':
                    references = results
            except Exception:
                print(profile, label, browser.find_element('id', 'status').text, browser.execute_script('return {state:window.lastState,audio:window.lastAudio,error:window.probeError}'), browser.get_log('browser'), flush=True)
                raise
            finally:
                browser.quit()
                server.shutdown()
                server.server_close()
print('Both browser profiles preserve all six dense scenes with maximum saves and recorded audio; no artifacts published.')
