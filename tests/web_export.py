"""Play a real portable export in Chromium and check integrity and shutdown."""
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import argparse, os, shutil, subprocess, tempfile, threading, time
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.common.by import By
from selenium.webdriver.common.action_chains import ActionChains
from selenium.webdriver.common.keys import Keys
from selenium.webdriver.support.ui import WebDriverWait

PROJECT=Path(__file__).resolve().parents[1]
CLI=PROJECT/'target/release/gamepkg'
p=argparse.ArgumentParser();p.add_argument('--component',action='store_true');p.add_argument('--backend',choices=['glsl','spirv','wgsl'],default='glsl');args=p.parse_args()
profile_prefix='runtime_profile = \"component-v1\"\n' if args.component else ''
class Quiet(SimpleHTTPRequestHandler):
    def log_message(self,*args): pass
    def end_headers(self):
        self.send_header("Cache-Control","no-store")
        super().end_headers()
with tempfile.TemporaryDirectory(prefix='gamepkg-web-export-') as temporary:
    root=Path(temporary);config=root/'settings.toml';config.write_text('automatic_update_checks=false\nshader_backend = \"'+args.backend+'\"\n')
    env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config))
    key=root/'key';key.write_text('45'*32)
    def run(*args):
        result=subprocess.run([str(CLI),*map(str,args)],env=env,capture_output=True,text=True,timeout=120)
        assert result.returncode==0,result.stderr
        return result
    archive=root/'2048.gamepkg'
    source=root/'source';shutil.copytree(PROJECT/'examples/good/2048',source)
    manifest=source/'manifest.toml';manifest.write_text(profile_prefix+manifest.read_text())
    run('pack',source,'-o',archive,'--key',key)
    run('install',archive,'--trust-creator');run('build','com.gamepkg.2048')
    out=root/'web';run('web-export','com.gamepkg.2048','-o',out)
    original=(out/'game.wasm').read_bytes()
    rejected=subprocess.run([str(CLI),'web-export','com.gamepkg.2048','-o',str(out)],env=env,capture_output=True,text=True,timeout=20)
    assert rejected.returncode!=0 and (out/'game.wasm').read_bytes()==original
    server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(out)))
    thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
    options=Options();options.binary_location=shutil.which('chromium');options.add_argument('--headless=new');options.add_argument('--enable-unsafe-swiftshader');options.add_argument('--window-size=1024,1000');options.add_argument('--no-sandbox');options.add_argument('--disable-dev-shm-usage');options.add_argument('--user-data-dir='+str(root/'browser'))
    try:
        with webdriver.Chrome(service=Service(shutil.which('chromedriver')),options=options) as browser:
            url=f'http://127.0.0.1:{server.server_port}/'
            browser.get(url);wait=WebDriverWait(browser,20)
            wait.until(lambda b:b.find_element(By.ID,'status').text=='Ready to play')
            browser.find_element(By.ID,'play').click()
            wait.until(lambda b:b.find_element(By.ID,'game').get_attribute('data-state'))
            canvas=browser.find_element(By.ID,'game');initial=canvas.get_attribute('data-state');canvas.send_keys(Keys.ARROW_RIGHT)
            wait.until(lambda b:b.find_element(By.ID,'game').get_attribute('data-state')!=initial)
            colors=browser.execute_script('const d=document.querySelector("canvas").getContext("2d").getImageData(0,0,640,480).data;let tiles=0;for(let i=0;i<d.length;i+=4)if(d[i]>180&&d[i+1]>120&&d[i+2]>80)tiles++;return tiles;')
            assert colors>10000,colors
            proof=PROJECT/'target/web-export-proof';proof.mkdir(parents=True,exist_ok=True);browser.execute_script('window.scrollTo(0,0)');browser.save_screenshot(str(proof/('2048-component-playing.png' if args.component else '2048-playing.png')))
            browser.find_element(By.ID,'stop').click();state=canvas.get_attribute('data-state');time.sleep(.1);assert canvas.get_attribute('data-state')==state
            assert browser.find_element(By.ID,'status').text=='Stopped'
            (out/'game.wasm').write_bytes(original+b'broken')
            browser.get(url);wait.until(lambda b:'integrity check failed' in b.find_element(By.ID,'status').text)
            assert not browser.find_element(By.ID,'play').is_enabled()
            scoped=root/'scoped';shutil.copytree(PROJECT/'examples/good/collect-stars',scoped)
            scoped_manifest=scoped/'manifest.toml'
            scoped_manifest.write_text(profile_prefix+'requires_rom = true\n'+scoped_manifest.read_text().replace('com.gamepkg.collect-stars','com.gamepkg.web-scoped').replace('save = false','save = true'))
            (scoped/'assets/message.txt').write_text('ABCD')
            (scoped/'source/src/lib.rs').write_text('''pub fn game_tick(_:u32,_:u64)->u64 {
                let mut buffer = [0; 8];
                let asset = gamepkg::asset_read("message.txt",0,&mut buffer) == Some(4) && &buffer[..4] == b"ABCD";
                let denied = gamepkg::asset_read("../message.txt",0,&mut buffer).is_none() && gamepkg::asset_read("/etc/passwd",0,&mut buffer).is_none();
                let rom = gamepkg::rom_count() == 1 && gamepkg::rom_read(0,&mut buffer) == Some(3) && &buffer[..3] == b"ROM";
                let saved = gamepkg::save_write(b"saved") && gamepkg::save_read(0,&mut buffer) == Some(5) && &buffer[..5] == b"saved";
                if asset && denied && rom && saved && gamepkg::achievement_unlock(0) { 42 } else { 0 }
            }''')
            run('pack',scoped,'-o',root/'scoped.gamepkg','--key',key);run('install',root/'scoped.gamepkg','--trust-creator');run('build','com.gamepkg.web-scoped')
            scoped_export=root/'scoped-web';run('web-export','com.gamepkg.web-scoped','-o',scoped_export)
            for path in scoped_export.iterdir():shutil.copyfile(path,out/path.name)
            browser.get(url);wait.until(lambda b:b.find_element(By.ID,'status').text=='Ready to play')
            rom=root/'user.rom';rom.write_bytes(b'ROM');browser.find_element(By.CSS_SELECTOR,'#roms input').send_keys(str(rom));time.sleep(.1)
            browser.find_element(By.ID,'play').click()
            wait.until(lambda b:b.find_element(By.ID,'game').get_attribute('data-state')=='42')
            browser.find_element(By.ID,'stop').click()
            persisted=browser.execute_script('return Object.keys(localStorage).filter(key=>key.startsWith("gamepkg:save:com.gamepkg.web-scoped:")).map(key=>JSON.parse(localStorage.getItem(key)));')
            assert persisted==[[115,97,118,101,100]],persisted
            history=browser.execute_script('return Object.keys(localStorage).filter(key=>key.startsWith("gamepkg:history:com.gamepkg.web-scoped:")).map(key=>JSON.parse(localStorage.getItem(key)));')
            assert history[0]['unlocked']==['first_star'] and history[0]['played_ms']>0,history
            browser.get(url);wait.until(lambda b:b.find_element(By.ID,'status').text=='Ready to play')
            assert browser.execute_script('return Object.keys(localStorage).some(key=>key.startsWith("gamepkg:save:com.gamepkg.web-scoped:"));')
            for name in ['sprite-playground','png-sprite-playground','tone-pad','shader-switch','shader-texture']:
                media=root/name;shutil.copytree(PROJECT/'examples/good'/name,media)
                manifest=media/'manifest.toml';manifest.write_text(profile_prefix+manifest.read_text())
                media_id='com.gamepkg.'+name
                run('pack',media,'-o',root/(name+'.gamepkg'),'--key',key);run('install',root/(name+'.gamepkg'),'--trust-creator');run('build',media_id)
                media_export=root/(name+'-web');run('web-export',media_id,'-o',media_export)
                for path in media_export.iterdir():shutil.copyfile(path,out/path.name)
                browser.get(url);wait.until(lambda b:b.find_element(By.ID,'status').text=='Ready to play')
                browser.find_element(By.ID,'play').click();wait.until(lambda b:b.find_element(By.ID,'game').get_attribute('data-state'))
                canvas=browser.find_element(By.ID,'game')
                if name in ['shader-switch','shader-texture']:
                    def dominant():
                        return browser.execute_script('const d=document.querySelector("canvas").getContext("2d").getImageData(320,240,1,1).data;return Array.from(d);')
                    wait.until(lambda b:dominant()[2]>dominant()[1]+100)
                    ActionChains(browser).click(canvas).key_down(Keys.ARROW_RIGHT).pause(.2).perform()
                    wait.until(lambda b:dominant()[1]>dominant()[2]+100)
                    ActionChains(browser).key_up(Keys.ARROW_RIGHT).perform()
                    wait.until(lambda b:dominant()[2]>dominant()[1]+100)
                elif name=='tone-pad':
                    wait.until(lambda b:int(b.find_element(By.ID,'game').get_attribute('data-audio-samples') or '0')>1000)
                    mute=browser.find_element(By.ID,'mute');mute.click();assert mute.text=='Unmute' and mute.get_attribute('aria-pressed')=='true';mute.click();assert mute.text=='Mute'
                else:
                    initial=canvas.get_attribute('data-state');ActionChains(browser).click(canvas).key_down(Keys.ARROW_RIGHT).pause(.2).key_up(Keys.ARROW_RIGHT).perform();wait.until(lambda b:b.find_element(By.ID,'game').get_attribute('data-state')!=initial)
                    colored=browser.execute_script('const d=document.querySelector("canvas").getContext("2d").getImageData(0,0,640,480).data;let count=0;for(let i=0;i<d.length;i+=4)if(d[i]>80||d[i+1]>80||d[i+2]>80)count++;return count;')
                    assert colored>10000,(name,colored)
                browser.find_element(By.ID,'stop').click()
            hanging=root/'hanging';shutil.copytree(PROJECT/'examples/good/2048',hanging)
            manifest=hanging/'manifest.toml';manifest.write_text(profile_prefix+manifest.read_text().replace('com.gamepkg.2048','com.gamepkg.web-hang'))
            (hanging/'source/src/lib.rs').write_text('pub fn game_tick(_:u32,_:u64)->u64 { loop {} }')
            run('pack',hanging,'-o',root/'hang.gamepkg','--key',key);run('install',root/'hang.gamepkg','--trust-creator');run('build','com.gamepkg.web-hang')
            hang_export=root/'hang-web';run('web-export','com.gamepkg.web-hang','-o',hang_export)
            for path in hang_export.iterdir():shutil.copyfile(path,out/path.name)
            browser.get(url);wait.until(lambda b:b.find_element(By.ID,'status').text=='Ready to play')
            browser.find_element(By.ID,'play').click()
            wait.until(lambda b:'deadline exceeded' in b.find_element(By.ID,'status').text)
            assert browser.find_element(By.ID,'stop').is_enabled() is False

    finally:
        server.shutdown();server.server_close();thread.join(timeout=5)
    print('Web export passed: isolated source build, portable browser gameplay, rendered tiles, input, stop, overwrite rejection, scoped asset/ROM/save operations, achievements, sprites, audio/mute, shaders/textures, watchdog and damaged-artifact rejection.')
