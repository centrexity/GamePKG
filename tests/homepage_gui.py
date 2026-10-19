"""Actual original menu clicks, host browser dispatch and browser target isolation.
All packages/libraries/keys are synthetic and temporary. The desktop browser is
replaced with a recording executable; Chromium maps the test hostname to loopback.
"""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json, os, shutil, subprocess, sys, tempfile, threading, time
from gui_test_support import start_display

PROJECT = Path(__file__).resolve().parents[1]
CLI = PROJECT/'target/release/gamepkg'
GUI = PROJECT/'target/release/gamepkg-gui'
URL = 'https://homepage.invalid/project'
class Quiet(SimpleHTTPRequestHandler):
    def log_message(self, *args): pass
with tempfile.TemporaryDirectory(prefix='gamepkg-homepage-') as temporary:
    root = Path(temporary)
    config = root/'settings.toml'
    config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n')
    browser_stub = root/'record-browser'
    browser_stub.write_text('''#!/usr/bin/env python3
import json,os,sys
with open(os.environ['GAMEPKG_BROWSER_RECORD'],'a') as out:out.write(json.dumps(sys.argv[1:])+'\\n')
''')
    browser_stub.chmod(0o755)
    receipt = root/'opened.jsonl'
    env = dict(os.environ, GAMEPKG_HOME=str(root/'library'), GAMEPKG_CONFIG=str(config),
               GAMEPKG_BROWSER_RECORD=str(receipt), BROWSER=str(browser_stub),
               LIBGL_ALWAYS_SOFTWARE='1', DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus')
    env.pop('WAYLAND_DISPLAY', None)
    key = root/'key'; key.write_text('35'*32)
    def run(*args):
        result = subprocess.run(list(map(str,args)), env=env, capture_output=True, text=True, timeout=180)
        assert result.returncode == 0, (args, result.stderr)
        return result.stdout
    def wait(predicate, label):
        deadline = time.monotonic()+20
        while not predicate():
            assert time.monotonic() < deadline, label
            time.sleep(.05)
    for profile,suffix in [('core-v1','core'),('component-v1','component')]:
        source = root/suffix; shutil.copytree(PROJECT/'ports/arkanoo', source)
        manifest = source/'manifest.toml'
        manifest.write_text('runtime_profile="'+profile+'"\n'+manifest.read_text().replace(
            'com.gamepkg.arkanoo','com.gamepkg.homepage-'+suffix).replace(
            'https://github.com/c0m4r/arkanoo', URL))
        entry = source/'source/src/lib.rs'
        text = entry.read_text().replace('pub fn game_tick(input: u32, state: u64) -> u64 {',
                                        'fn production_tick(input: u32, state: u64) -> u64 {')
        # Keep the main menu visible. The production click handler, renderer and
        # window-control request still execute unchanged on every tick.
        entry.write_text(text+'''\npub fn game_tick(input:u32,state:u64)->u64 {
let result=production_tick(input,state);
if let Ok(mut slot)=GAME.lock(){if let Some(game)=slot.as_mut(){game.mode=0;}}
result
}\n''')
        package = root/(suffix+'.gamepkg')
        run(CLI,'pack',source,'-o',package,'--key',key)
        run(CLI,'install',package,'--trust-creator')
        run(CLI,'build','com.gamepkg.homepage-'+suffix,'--target','wasm')
    display = None if '--browser-only' in sys.argv else start_display(env,1600,1000)
    app = None
    try:
        for suffix,target in ([] if '--browser-only' in sys.argv else [('core','wasm'),('component','wasm'),('core','native')]):
            game_id = 'com.gamepkg.homepage-'+suffix
            run(CLI,'build',game_id,'--target',target)
            receipt.unlink(missing_ok=True)
            app = subprocess.Popen([GUI,'--play',game_id,'--target',target,'--renderer','opengl'],
                                   env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            windows=[]
            def find():
                result=subprocess.run(['xdotool','search','--onlyvisible','--name','^'+game_id.replace('.','\\.')+'$'], env=env,capture_output=True,text=True)
                if result.returncode==0: windows[:]=result.stdout.splitlines(); return True
                assert app.poll() is None, app.stderr.read().decode()
                return False
            wait(find,'game window')
            window=windows[0]
            run('xdotool','windowsize',window,'640','480','windowfocus',window)
            time.sleep(.5)
            assert not receipt.exists(), 'Game opened a browser without an action'
            for count in [1,2]:
                run('xdotool','mousemove','--window',window,'600','405','click','1')
                wait(lambda:receipt.exists() and len(receipt.read_text().splitlines())==count,'committed homepage request')
                time.sleep(.2)
                assert [json.loads(line) for line in receipt.read_text().splitlines()] == [[URL]]*count
            app.terminate(); app.communicate(timeout=10); app=None
            print(suffix,target,'actual menu click dispatches only the declared URL, once per click.',flush=True)
    finally:
        if app is not None: app.terminate(); app.communicate(timeout=10)
        if display is not None: display.terminate(); display.wait(timeout=10)
    from selenium import webdriver
    from selenium.webdriver.chrome.options import Options
    from selenium.webdriver.chrome.service import Service
    from selenium.webdriver.common.action_chains import ActionChains
    from selenium.webdriver.support.ui import WebDriverWait
    for suffix in ['core','component']:
        output=root/(suffix+'-web')
        run(CLI,'web-export','com.gamepkg.homepage-'+suffix,'--output',output)
        server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(output)))
        threading.Thread(target=server.serve_forever,daemon=True).start()
        options=Options();options.binary_location=shutil.which('google-chrome') or shutil.which('google-chrome-stable') or shutil.which('chromium')
        assert options.binary_location
        for option in ['--headless=new','--window-size=1100,1000','--no-sandbox','--disable-dev-shm-usage','--enable-unsafe-swiftshader','--host-resolver-rules=MAP homepage.invalid 127.0.0.1']:
            options.add_argument(option)
        driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver')
        browser=webdriver.Chrome(service=Service(driver),options=options)
        try:
            browser.get(f'http://127.0.0.1:{server.server_port}/')
            ready=WebDriverWait(browser,20)
            ready.until(lambda b:b.find_element('id','status').text=='Ready to play')
            browser.find_element('id','play').click()
            ready.until(lambda b:b.find_element('id','game').get_attribute('data-state') is not None)
            original=browser.current_window_handle
            for click_index in range(5):
                if click_index == 2:
                    browser.find_element('id','restart').click()
                    ready.until(lambda b:b.find_element('id','game').get_attribute('data-state') is not None)
                if click_index == 3:
                    browser.execute_script("window.originalHomepageSetItem=Storage.prototype.setItem;Storage.prototype.setItem=function(){throw Error('fixture storage failure')}")
                time.sleep(.2)
                assert len(browser.window_handles)==1
                canvas=browser.find_element('id','game')
                rect=canvas.rect
                ActionChains(browser).move_to_element_with_offset(canvas,int(rect['width']*(600/640-.5)),int(rect['height']*(405/480-.5))).click().perform()
                if click_index == 3:
                    time.sleep(.4)
                    assert len(browser.window_handles)==1, 'Failed history persistence opened a homepage'
                    browser.execute_script('Storage.prototype.setItem=window.originalHomepageSetItem')
                    continue
                try:
                    ready.until(lambda b:len(b.window_handles)==2)
                except Exception as error:
                    state=browser.execute_script('return {requestedWindow,lastHomepageRequest,active:navigator.userActivation.isActive,focused:document.hasFocus(),canvasFocus:document.activeElement===canvas,rect:canvas.getBoundingClientRect().toJSON()}')
                    raise AssertionError((suffix,state,browser.find_element('id','status').text)) from error
                opened=next(handle for handle in browser.window_handles if handle!=original)
                browser.switch_to.window(opened)
                ready.until(lambda b:b.current_url==URL)
                assert browser.execute_script('return window.opener===null')
                browser.close();browser.switch_to.window(original)
                time.sleep(.2)
                assert len(browser.window_handles)==1, 'One request opened multiple tabs'
            print(suffix,'browser original clicks/restart/storage failure/recovery preserve declared URL and opener isolation.',flush=True)
        finally:
            browser.quit();server.shutdown();server.server_close()
