"""Signed core/component exports recover lost host graphics on a fresh Play."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import os, shutil, subprocess, tempfile, threading, wave
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.support.ui import WebDriverWait

project = Path(__file__).resolve().parents[1]
class Quiet(SimpleHTTPRequestHandler):
    def log_message(self, *args): pass

with tempfile.TemporaryDirectory(prefix='gamepkg-graphics-recovery-') as temporary:
    root = Path(temporary)
    (root/'key').write_text('68'*32)
    (root/'settings.toml').write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
    env = dict(os.environ, GAMEPKG_HOME=str(root/'library'), GAMEPKG_CONFIG=str(root/'settings.toml'))
    def run(*args):
        result = subprocess.run([str(project/'target/release/gamepkg'), *map(str,args)], env=env, capture_output=True, text=True, timeout=180)
        assert result.returncode == 0, result.stdout+'\n'+result.stderr
    with wave.open(str(root/'tone.wav'),'wb') as tone:
        tone.setnchannels(2);tone.setsampwidth(2);tone.setframerate(44100);tone.writeframes(b'\x39\x30\x60\xa4'*441000)
    subprocess.run(['ffmpeg','-v','error','-i',str(root/'tone.wav'),'-codec:a','libmp3lame','-write_xing','0',str(root/'tone.mp3')],check=True)
    for profile in ['core-v1', 'component-v1']:
        recipe = root/(profile+'-source')
        shutil.copytree(project/'examples/minimal', recipe)
        game_id = 'com.gamepkg.graphics-recovery-'+profile
        manifest = recipe/'manifest.toml'
        manifest.write_text('runtime_profile="'+profile+'"\n'+manifest.read_text().replace('com.gamepkg.minimal',game_id))
        manifest.write_text(manifest.read_text().replace('audio = false','audio = true').replace('[limits]','[limits]\neffects_memory_bytes=4194304\nmusic_memory_bytes=8388608'))
        shutil.copy2(root/'tone.mp3',recipe/'assets/tone.mp3')
        with manifest.open('a') as output:
            output.write('\n[[shaders]]\nsource="shaders/main.slang"\nentry="fragmentMain"\nstage="fragment"\n\n[[shaders]]\nsource="shaders/extra.slang"\nentry="extraMain"\nstage="fragment"\n')
        (recipe/'shaders/main.slang').write_text('[shader("fragment")]\nfloat4 fragmentMain() : SV_Target { return float4(0.0,1.0,0.0,1.0); }\n')
        (recipe/'shaders/extra.slang').write_text((project/'examples/good/shader-texture/shaders/main.slang').read_text().replace('blueMain','extraMain'))
        (recipe/'source/src/lib.rs').write_text('''pub fn game_tick(_:u32,state:u64)->u64 {
            if state==0 && !gamepkg::music_play("tone.mp3") {return 0;}
            gamepkg::draw_clear(0x000000ff);
            let Some(main_shader)=gamepkg::Shader::from_slang("shaders/main.slang","fragmentMain") else {return 0;};
            let Some(extra_shader)=gamepkg::Shader::from_slang("shaders/extra.slang","extraMain") else {return 0;};
            let left=gamepkg::draw_clip(0.0,0.0,320.0,480.0);
            let shaded=main_shader.draw();
            let right=gamepkg::draw_clip(320.0,0.0,320.0,480.0);
            let second=extra_shader.draw_rgba(1,1,&[0u8,0,255,255]);
            let unclipped=gamepkg::draw_clip_reset();
            let filtered=gamepkg::draw_image_linear(10.0,10.0,20.0,20.0,1,1,&[255u8,64,0,255]);
            if shaded && second && filtered && left && right && unclipped {state+1}else{0}
        }''')
        archive = root/(profile+'.gamepkg')
        run('pack',recipe,'-o',archive,'--key',root/'key')
        run('install',archive,'--trust-creator')
        run('build',game_id,'--target','wasm')
        run('web-export',game_id,'-o',root/profile)
    server = ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(root)))
    threading.Thread(target=server.serve_forever,daemon=True).start()
    options = Options()
    options.binary_location = shutil.which('google-chrome') or shutil.which('chromium') or shutil.which('google-chrome-stable')
    for flag in ['--headless=new','--no-sandbox','--disable-dev-shm-usage']: options.add_argument(flag)
    driver = shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver')
    browser = webdriver.Chrome(service=Service(driver),options=options)
    browser.execute_cdp_cmd('Page.addScriptToEvaluateOnNewDocument', {'source': '''
        window.graphics=[];window.gameWorkers=0;
        const initial=new URL(location.href).searchParams.get('initial');
        if(initial==='compile')window.failShaderCompile='shader';if(initial==='unavailable')window.rejectNextContext='shader';
        window.pendingYields=[];
        const draw=WebGL2RenderingContext.prototype.drawArrays;
        WebGL2RenderingContext.prototype.drawArrays=function(...args){const value=draw.apply(this,args);const attributes=this.getContextAttributes();
            if(window.forceYieldGraphicsKind===(attributes?.premultipliedAlpha?'filtered':'shader')){window.rendererDrawn=true;const until=performance.now()+9;while(performance.now()<until){}}
            return value;};
        const schedule=setTimeout;window.setTimeout=function(callback,delay,...args){
            if(delay===0&&window.holdRenderYields&&window.rendererDrawn){window.rendererDrawn=false;window.pendingYields.push(()=>callback(...args));return schedule(()=>{},0);}
            return schedule(callback,delay,...args);};
        const get=OffscreenCanvas.prototype.getContext;
        OffscreenCanvas.prototype.getContext=function(type,options){
            const kind=options?.premultipliedAlpha?'filtered':'shader';
            if(type==='webgl2'&&window.rejectNextContext===kind){window.rejectNextContext=null;return null;}
            const context=get.call(this,type,options);
            if(type==='webgl2'&&context){
                window.graphics.push({context,kind});
                if(window.failShaderCompile===kind){window.failShaderCompile=null;let checks=0;const inspect=context.getShaderParameter;
                    context.getShaderParameter=function(shader,parameter){if(parameter===context.COMPILE_STATUS&&++checks===(kind==='shader'?3:1))return false;return inspect.call(this,shader,parameter);};}
            }
            return context;
        };
        const Original=Worker;window.Worker=class extends Original{constructor(...args){super(...args);if(String(args[0]).split('/').at(-1)==='worker.js')window.gameWorkers++;}};
    '''})
    def image_hash():
        return browser.execute_async_script('''const done=arguments[arguments.length-1],canvas=document.querySelector('#game');
            const pixels=canvas.getContext('2d').getImageData(0,0,640,480).data;
            crypto.subtle.digest('SHA-256',pixels).then(bytes=>done(Array.from(new Uint8Array(bytes),v=>v.toString(16).padStart(2,'0')).join(''))).catch(error=>done(String(error)));
        ''')
    try:
        for profile in ['core-v1','component-v1']:
            for initial in ['unavailable','compile']:
                browser.get(f'http://127.0.0.1:{server.server_port}/{profile}/?initial={initial}')
                wait=WebDriverWait(browser,20)
                kind='initial';fault=initial
                expected_error='This game requires WebGL 2' if initial=='unavailable' else 'WebGL shader compilation failed:'
                wait.until(lambda b:expected_error in b.find_element('id','status').text)
                assert browser.execute_script('return window.gameWorkers')==0,'Failed initial shader setup started a guest'
                assert browser.execute_script('return window.graphics.every(item=>item.context.isContextLost())'),'Failed initial setup kept a graphics context alive'
                assert not browser.find_element('id','play').is_enabled()
                print(profile,initial,'initial setup rejects before guest allocation and releases partial graphics.',flush=True)
            for kind,fault in [('filtered','unavailable'),('filtered','compile'),('shader','unavailable'),('shader','compile'),('filtered','suspended'),('shader','suspended'),('both','normal')]:
                browser.get(f'http://127.0.0.1:{server.server_port}/{profile}/')
                wait = WebDriverWait(browser,20)
                wait.until(lambda b:b.find_element('id','status').text=='Ready to play')
                hooked=browser.execute_async_script('''const done=arguments[arguments.length-1];Promise.all([import('./recorded-session.js'),import('./progress-store.js')]).then(([{RecordedSession},{ProgressStore}])=>{window.progressLoads=0;const load=ProgressStore.prototype.load;ProgressStore.prototype.load=async function(...args){const result=await load.apply(this,args);window.progressLoads++;return result;};window.audioRoots=[];const open=RecordedSession.open,snapshot=RecordedSession.prototype.snapshot;RecordedSession.open=async function(...args){window.audioRoots.push(args[4]);return open.apply(this,args);};RecordedSession.prototype.snapshot=async function(){const value=await snapshot.call(this);window.lastAudio=value;return value;};done(true);}).catch(error=>done(String(error)));''')
                assert hooked is True,hooked
                browser.find_element('id','play').click()
                wait.until(lambda b:b.execute_script('return Number(document.querySelector("#game").dataset.state)>0&&window.lastAudio?.musicStatus==="playing"&&window.lastAudio.playedFrames>0'))
                wait.until(lambda b:b.execute_script('return window.graphics.length===2'))
                assert browser.execute_script('const c=document.querySelector("#game").getContext("2d");return [Array.from(c.getImageData(200,200,1,1).data),Array.from(c.getImageData(500,200,1,1).data),Array.from(c.getImageData(20,20,1,1).data)]')==[[0,255,0,255],[0,0,255,255],[255,64,0,255]], 'Baseline shader/filter output was not rendered'
                expected = image_hash()
                previous = browser.execute_script('return window.gameWorkers')
                if fault=='suspended':
                    browser.execute_script('window.forceYieldGraphicsKind=arguments[0];window.holdRenderYields=true;window.rendererDrawn=false;',kind)
                    wait.until(lambda b:b.execute_script('return window.pendingYields.length===1'))
                lost = browser.execute_script('''const items=window.graphics.filter(item=>arguments[0]==='both'||item.kind===arguments[0]);
                    for(const item of items){const extension=item.context.getExtension('WEBGL_lose_context');if(!extension)return false;extension.loseContext();}return items.length===(arguments[0]==='both'?2:1);''',kind)
                assert lost is True, 'Context loss extension unavailable'
                if fault=='normal':
                    wait.until(lambda b:'context unavailable' in b.find_element('id','status').text)
                    assert image_hash()==expected
                    wait.until(lambda b:b.execute_script('return window.audioRoots.every(root=>root.used===0)'))
                    browser.find_element('id','play').click()
                elif fault=='suspended':
                    browser.find_element('id','stop').click()
                    browser.execute_script('window.holdRenderYields=false;window.forceYieldGraphicsKind=null;')
                    wait.until(lambda b:b.execute_script('return window.audioRoots.every(root=>root.used===0)'))
                    loads=browser.execute_script('return window.progressLoads')
                    browser.find_element('id','play').click()
                    wait.until(lambda b:b.execute_script('return window.progressLoads>arguments[0]',loads))
                    browser.execute_async_script('const done=arguments[arguments.length-1];setTimeout(()=>done(true),50);')
                    assert browser.execute_script('return window.gameWorkers')==previous,'Replacement started before cancelled rendering completed'
                    assert browser.execute_script('return window.graphics.length')==2,'Graphics cache was replaced before rendering cleanup'
                    assert image_hash()==expected,'Suspended preparation changed committed pixels'
                    browser.execute_script('for(const resume of window.pendingYields.splice(0))resume();')
                else:
                    wait.until(lambda b:'context unavailable' in b.find_element('id','status').text)
                    assert image_hash()==expected, 'Failed preparation changed the last committed image'
                    wait.until(lambda b:b.execute_script('return window.audioRoots.every(root=>root.used===0)'))
                    browser.execute_script('window[arguments[0]]=arguments[1]', 'rejectNextContext' if fault=='unavailable' else 'failShaderCompile',kind)
                    browser.find_element('id','play').click()
                    expected_error=('Filtered game images require WebGL 2' if kind=='filtered' else 'This game requires WebGL 2') if fault=='unavailable' else ('Filtered image shader failed' if kind=='filtered' else 'WebGL shader compilation failed:')
                    wait.until(lambda b:expected_error in b.find_element('id','status').text)
                    assert image_hash()==expected, 'Failed context recreation changed committed pixels'
                    wait.until(lambda b:b.execute_script('return window.audioRoots.every(root=>root.used===0)'))
                    if kind=='shader':assert browser.execute_script('return window.gameWorkers')==previous, 'Failed shader recovery started a guest'
                    browser.find_element('id','play').click()
                wait.until(lambda b:b.execute_script('return window.gameWorkers>arguments[0]&&Number(document.querySelector("#game").dataset.state)>0&&window.lastAudio?.musicStatus==="playing"&&window.lastAudio.playedFrames>0',previous))
                assert image_hash()==expected, 'Recovery changed shader IDs or filtered pixels'
                expected_contexts=4 if fault=='compile' or kind=='both' else 3
                assert browser.execute_script('return window.graphics.length')==expected_contexts, 'Healthy graphics context was unnecessarily replaced'
                assert browser.execute_script('return window.graphics.filter(item=>!item.context.isContextLost()).length')==2
                previous=browser.execute_script('return window.gameWorkers')
                browser.find_element('id','stop').click()
                wait.until(lambda b:b.execute_script('return window.audioRoots.every(root=>root.used===0)'))
                browser.find_element('id','play').click()
                wait.until(lambda b:b.execute_script('return window.gameWorkers>arguments[0]&&Number(document.querySelector("#game").dataset.state)>0&&window.lastAudio?.musicStatus==="playing"&&window.lastAudio.playedFrames>0',previous))
                assert image_hash()==expected
                assert browser.execute_script('return window.graphics.length')==expected_contexts, 'Healthy relaunch allocated another graphics context'
                browser.find_element('id','stop').click()
                wait.until(lambda b:b.execute_script('return window.audioRoots.every(root=>root.used===0)'))
                print(profile,kind,fault,'graphics fault/retry preserves committed pixels; fresh Play recovers exact output/music, releases audio budgets and reuses healthy contexts.',flush=True)
    except Exception:
        print('Graphics recovery failure:',profile,kind,fault,browser.find_element('id','status').text, browser.get_log('browser'),flush=True)
        raise
    finally:
        browser.quit();server.shutdown();server.server_close()
