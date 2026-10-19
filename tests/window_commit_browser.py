"""Signed core/component browser controls wait for actual save/history writes.
Synthetic games, local server, private browser profiles; no player data or ROMs.
"""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import os, shutil, subprocess, tempfile, threading, time
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.common.keys import Keys
from selenium.webdriver.support.ui import WebDriverWait
PROJECT=Path(__file__).resolve().parents[1]
CLI=PROJECT/'target/release/gamepkg'
class Quiet(SimpleHTTPRequestHandler):
    def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-window-commit-') as temporary:
    root=Path(temporary);key=root/'key';key.write_text('34'*32)
    config=root/'settings.toml';config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
    env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config))
    def run(*args):
        result=subprocess.run([str(CLI),*map(str,args)],env=env,capture_output=True,text=True,timeout=180)
        assert result.returncode==0,result.stderr
    for profile,suffix in [('core-v1','core'),('component-v1','component')]:
        source=root/suffix;shutil.copytree(PROJECT/'examples/minimal',source)
        manifest=source/'manifest.toml'
        manifest.write_text('runtime_profile="'+profile+'"\n'+manifest.read_text().replace('com.gamepkg.minimal','com.gamepkg.window-commit-'+suffix).replace('save = false','save = true').replace('audio = false','audio = true')+'\n[[permissions.files]]\nid="patterns"\ntitle="Patterns"\nextensions=["txt"]\nmax_bytes=1024\nread=true\nwrite=true\n')
        (source/'source/src/lib.rs').write_text('''use std::sync::Mutex;
        static FILE:Mutex<Option<u32>>=Mutex::new(None);
        pub fn game_tick(input:u32,state:u64)->u64 {
            gamepkg::save_write(&[input as u8]);
            let mut requested=false;
            if let Ok(mut file)=FILE.lock(){
                if let Some(handle)=*file {
                    if let Some(status)=gamepkg::file_status(handle){
                        if status.status>=2 {gamepkg::file_close(handle);*file=None;}
                    }else{*file=None;}
                }
                if input&16!=0 {*file=gamepkg::file_import(0);requested=file.is_some();}
                if input&32!=0 {*file=gamepkg::file_export(0,"state.txt",&[1,2,3]);requested=file.is_some();}
            }
            if input&1!=0 {gamepkg::window_control(0,0);gamepkg::window_control(1,1);}
            if input&2!=0 {gamepkg::window_control(0,1);gamepkg::window_control(1,0);}
            if input&4!=0 {gamepkg::window_control(2,0);}
            if input&8!=0 {gamepkg::window_control(3,0);}
            let mut samples=[12345i16;1600];let mut index=1;
            while index<1600 {samples[index]=-23456;index+=2;}
            gamepkg::audio_submit_stereo(&samples);
            gamepkg::draw_clear(if state&1==0 {0x193249ff}else{0x824029ff});
            (state&2147483647).wrapping_add(1)|if requested {2147483648}else{0}
        }\n''')
        package=root/(suffix+'.gamepkg');game_id='com.gamepkg.window-commit-'+suffix
        run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator');run('build',game_id,'--target','wasm')
        output=root/(suffix+'-web');run('web-export',game_id,'--output',output)
        server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(output)))
        threading.Thread(target=server.serve_forever,daemon=True).start()
        options=Options();options.binary_location=shutil.which('google-chrome') or shutil.which('google-chrome-stable') or shutil.which('chromium');assert options.binary_location
        for flag in ['--headless=new','--window-size=1100,1000','--no-sandbox','--disable-dev-shm-usage','--enable-unsafe-swiftshader','--autoplay-policy=no-user-gesture-required']:options.add_argument(flag)
        downloads=root/(suffix+'-downloads');downloads.mkdir()
        options.add_experimental_option('prefs',{'download.default_directory':str(downloads),'download.prompt_for_download':False})
        driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver')
        browser=webdriver.Chrome(service=Service(driver),options=options)
        browser.execute_cdp_cmd('Page.setDownloadBehavior',{'behavior':'allow','downloadPath':str(downloads)})
        browser.execute_cdp_cmd('Page.addScriptToEvaluateOnNewDocument',{'source':'''window.windowFrames=[];window.windowCommits=[];window.workerSerial=0;
            window.outputSnapshots=[];window.audioBufferCount=0;window.audioSources=[];
            const originalBuffer=AudioContext.prototype.createBuffer;
            AudioContext.prototype.createBuffer=function(...args){const result=originalBuffer.apply(this,args);window.audioBufferCount++;window.lastAudioBuffer=result;return result;};
            const originalSource=AudioContext.prototype.createBufferSource;
            AudioContext.prototype.createBufferSource=function(...args){const result=originalSource.apply(this,args),record={stopped:false,ended:false};window.audioSources.push(record);if(window.audioSources.length>128)window.audioSources.shift();const stop=result.stop.bind(result);result.stop=(...args)=>{record.stopped=true;return stop(...args);};result.addEventListener('ended',()=>record.ended=true);return result;};
            const originalWorker=window.Worker;window.Worker=class extends originalWorker{constructor(...args){super(...args);const instance=++window.workerSerial;
            this.addEventListener('message',event=>{const m=event.data;if(m.type==='frame'){
                window.windowFrames.push({instance,serial:m.serial,epoch:m.epoch,window:m.window,state:m.state});if(window.windowFrames.length>64)window.windowFrames.shift();
                const canvas=document.querySelector('#game'),pixel=()=>Array.from(canvas.getContext('2d').getImageData(320,240,1,1).data),before=pixel(),beforeState=canvas.dataset.state,beforeAudio=window.audioBufferCount;
                setTimeout(()=>{const commit=window.windowCommits.find(c=>c.instance===instance&&c.epoch===m.epoch&&c.serial===m.serial);window.outputSnapshots.push({instance,epoch:m.epoch,serial:m.serial,state:m.state,background:m.background,before,beforeState,beforeAudio,after:pixel(),afterState:canvas.dataset.state,afterAudio:window.audioBufferCount,success:commit?.success});if(window.outputSnapshots.length>64)window.outputSnapshots.shift();},0);
            }});
            const send=this.postMessage.bind(this);this.postMessage=(m,...rest)=>{if(m.type==='window-commit'){window.windowCommits.push({...m,instance});if(window.windowCommits.length>64)window.windowCommits.shift();}send(m,...rest);};}};'''})
        try:
            browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20)
            wait.until(lambda b:b.find_element('id','status').text=='Ready to play');browser.find_element('id','play').click()
            wait.until(lambda b:b.execute_script('return window.windowCommits.some(value=>value.success)'))
            wait.until(lambda b:b.execute_script('return window.audioBufferCount>0'))
            assert browser.execute_script('const b=window.lastAudioBuffer;return b.numberOfChannels===2&&b.length===800&&b.sampleRate===48000&&b.getChannelData(0)[0]===12345/32768&&b.getChannelData(1)[0]===-23456/32768'), 'Successful fixture did not publish exact stereo PCM'
            def press(key,predicate,success):
                identity=browser.execute_script('const value=window.windowFrames.at(-1);return {serial:value.serial,epoch:value.epoch,instance:value.instance}')
                before=identity['serial'];epoch=identity['epoch'];instance=identity['instance']
                browser.find_element('id','game').send_keys(key)
                expression=f'return window.windowFrames.find(value=>value.instance==={instance}&&value.epoch==={epoch}&&value.serial>{before}&&({predicate}))?.serial'
                serial=wait.until(lambda b:b.execute_script(expression))
                wait.until(lambda b:b.execute_script('return window.windowCommits.some(value=>value.serial===arguments[0]&&value.success===arguments[1]&&value.epoch===arguments[2]&&value.instance===arguments[3])',serial,success,epoch,instance))
                return serial
            # First prove these controls have real effects with working storage.
            press(Keys.ARROW_LEFT,'!value.window.visible&&value.window.confined',True)
            wait.until(lambda b:b.execute_script('return document.pointerLockElement===document.querySelector("#game")'))
            assert browser.find_element('id','game').value_of_css_property('cursor')=='none'
            press(Keys.ARROW_RIGHT,'value.window.visible&&!value.window.confined',True)
            wait.until(lambda b:b.execute_script('return document.pointerLockElement===null'))
            for failure in ['save','history']:
                browser.execute_script('''window.failedWindowWrites=0;window.historyWriteAttempts=0;window.originalWindowSetItem=Storage.prototype.setItem;
                    const selected=arguments[0];Storage.prototype.setItem=function(key,value){if(String(key).includes(':history:'))window.historyWriteAttempts++;if(String(key).includes(':'+selected+':')){window.failedWindowWrites++;throw Error('fixture write failure');}return window.originalWindowSetItem.call(this,key,value);};''',failure)
                wait.until(lambda b:b.execute_script('return window.failedWindowWrites>0'))
                press(Keys.ARROW_LEFT,'!value.window.visible&&value.window.confined',False)
                press(Keys.ARROW_UP,'value.window.fullscreen_toggle',False)
                press(Keys.ARROW_DOWN,'value.window.closed',False)
                press('z','Number(value.state)>=2147483648',False)
                time.sleep(.2)
                assert not browser.find_element('id','user-files').is_displayed(), 'Failed persistence published a picker'
                press('x','Number(value.state)>=2147483648',False)
                time.sleep(.2)
                assert not browser.find_element('id','user-files').is_displayed(), 'Failed persistence published an export'
                assert browser.execute_script('return document.pointerLockElement===null&&document.fullscreenElement===null')
                assert browser.find_element('id','game').value_of_css_property('cursor')!='none'
                assert browser.find_element('id','status').text!='Game closed'
                snapshots=browser.execute_script('return window.outputSnapshots.filter(frame=>frame.success===false)')
                assert snapshots, 'No failed-persistence frame was observed'
                if failure=='save':
                    assert browser.execute_script('return window.historyWriteAttempts===0'), 'A rejected save still wrote play history'
                for frame in snapshots:
                    assert frame['afterState']==frame['beforeState'], ('Failed persistence published game state',frame)
                    assert frame['after']==frame['before'], ('Failed persistence published canvas pixels',frame)
                    assert frame['afterAudio']==frame['beforeAudio'], ('Failed persistence scheduled PCM',frame)
                browser.execute_script('Storage.prototype.setItem=window.originalWindowSetItem')
                baseline=int(browser.find_element('id','game').get_attribute('data-state'))
                wait.until(lambda b:int(b.find_element('id','game').get_attribute('data-state'))>=baseline+3)
                assert browser.find_element('id','status').text=='Playing', 'Storage warning survived successful recovery'
                assert browser.execute_script('const state=window.windowFrames.at(-1).window;return state.visible&&!state.confined&&!state.fullscreen_toggle&&!state.closed')
                assert browser.execute_script('return document.pointerLockElement===null&&document.fullscreenElement===null')
                press('z','Number(value.state)>=2147483648',True)
                wait.until(lambda b:b.find_element('id','user-files').is_displayed())
                browser.find_element('css selector','#user-files button').click()
                wait.until(lambda b:not b.find_element('id','user-files').is_displayed())
                time.sleep(.1)
                outgoing=downloads/'state.txt';outgoing.unlink(missing_ok=True)
                browser.execute_script('Object.defineProperty(window,"showSaveFilePicker",{value:undefined,configurable:true})')
                press('x','Number(value.state)>=2147483648',True)
                wait.until(lambda b:b.find_element('id','user-files').is_displayed())
                browser.find_element('css selector','#user-files button').click()
                wait.until(lambda b:outgoing.exists())
                assert outgoing.read_bytes()==bytes([1,2,3])
                wait.until(lambda b:not b.find_element('id','user-files').is_displayed())
                time.sleep(.1)
                print(suffix,failure,'failure rolls back cursor/fullscreen/close/import/export/pixels/state/PCM; fresh recovery transfer works.',flush=True)
            press(Keys.ARROW_UP,'value.window.fullscreen_toggle',True)
            wait.until(lambda b:b.execute_script('return document.fullscreenElement===document.querySelector("#game")'))
            press(Keys.ARROW_UP,'!value.window.fullscreen_toggle',True)
            wait.until(lambda b:b.execute_script('return document.fullscreenElement===null'))
            wait.until(lambda b:b.execute_script('return window.audioSources.some(source=>!source.stopped&&!source.ended)'))
            assert browser.execute_script('const pending=window.audioSources.filter(source=>!source.stopped&&!source.ended);document.querySelector("#restart").click();return pending.length>0&&pending.every(source=>source.stopped)'), 'Restart retained queued PCM from the previous game generation'
            wait.until(lambda b:b.execute_script('return window.windowFrames.at(-1)?.epoch===1&&window.windowCommits.at(-1)?.epoch===1'))
            # Valid JSON with an invalid schema must terminate explicitly,
            # acknowledge denial and preserve the damaged history record.
            browser.execute_script("window.badHistoryKey=Object.keys(localStorage).find(key=>key.startsWith('gamepkg:history:'));window.badHistoryValue=JSON.stringify({played_ms:0,unlocked:null});localStorage.setItem(window.badHistoryKey,window.badHistoryValue)")
            wait.until(lambda b:'browser play history is invalid' in b.find_element('id','status').text)
            assert browser.execute_script('return localStorage.getItem(window.badHistoryKey)===window.badHistoryValue&&!window.windowCommits.at(-1).success')
            browser.execute_script('localStorage.setItem(window.badHistoryKey,JSON.stringify({played_ms:0,unlocked:[]}))')
            browser.find_element('id','play').click()
            wait.until(lambda b:b.execute_script('const value=window.windowFrames.at(-1);return value.instance===window.workerSerial&&window.windowCommits.some(commit=>commit.instance===value.instance&&commit.success)'))
            # Final successful close proves explicit recovery did not leave a pending transaction.

            press(Keys.ARROW_DOWN,'value.window.closed',True)
            wait.until(lambda b:b.find_element('id','status').text=='Game closed')
            print(suffix,'actual confinement, fullscreen, restart and close work after persistence recovery.',flush=True)
        except Exception:
            print(suffix,'failure state:',browser.execute_script('return {frames:window.windowFrames.slice(-5),commits:window.windowCommits.slice(-5),status:document.querySelector("#status").textContent}'),flush=True)
            raise
        finally:
            browser.quit();server.shutdown();server.server_close()
