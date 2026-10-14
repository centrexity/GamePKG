"""Signed Arkanoo port: isolated builds, native keyboard play and browser export."""
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import os, subprocess, tempfile, threading, time
from PIL import Image
from gui_test_support import start_display
import argparse
parser=argparse.ArgumentParser()
parser.add_argument('--skip-browser',action='store_true',help='Run native checks on hosts without Chromium/Selenium')
args=parser.parse_args()
if not args.skip_browser:
    from selenium import webdriver
    from selenium.webdriver.chrome.options import Options
    from selenium.webdriver.chrome.service import Service
    from selenium.webdriver.common.by import By
    from selenium.webdriver.common.action_chains import ActionChains
    from selenium.webdriver.common.keys import Keys
    from selenium.webdriver.support.ui import WebDriverWait
PROJECT=Path(__file__).resolve().parents[1];CLI=PROJECT/'target/release/gamepkg'
PROOF=PROJECT/'target/arkanoo-proof';PROOF.mkdir(parents=True,exist_ok=True)
class Quiet(SimpleHTTPRequestHandler):
    def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-arkanoo-',dir=PROJECT/'target') as temp:
    root=Path(temp);config=root/'settings.toml';config.write_text('automatic_update_checks=false\n')
    env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None)
    key=root/'key';key.write_text('61'*32)
    def run(*args):
        r=subprocess.run([str(CLI),*map(str,args)],env=env,text=True,capture_output=True,timeout=120)
        assert r.returncode==0,r.stderr
        return r.stdout
    run('pack',PROJECT/'examples/good/arkanoo','-o',root/'arkanoo.gamepkg','--key',key)
    run('install',root/'arkanoo.gamepkg','--trust-creator');run('build','com.gamepkg.arkanoo')
    result=run('run','com.gamepkg.arkanoo','--frames',180)
    assert 'Ran 180 frames' in result and 'draw commands: 1024' not in result,result
    run('integrity-check','com.gamepkg.arkanoo')
    probe_env=dict(env,GAMEPKG_ARKANOO_TEST_ROOT=str(root/'library'))
    subprocess.run(['cargo','test','--test','game_arkanoo_runtime','--','--ignored'],env=probe_env,check=True,timeout=120)
    # Real native window: locate the cyan paddle, hold movement, start, pause.
    display=start_display(env,width=1400,height=900);app=None
    def capture(window,name):
        path=PROOF/name
        subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True,timeout=10)
        return Image.open(path).convert('RGB')
    def paddle(image):
        points=[(x,y) for y in range(image.height//2,image.height-20) for x in range(image.width)
                if image.getpixel((x,y))==(99,220,255)]
        assert len(points)>100,len(points)
        return sum(x for x,y in points)/len(points)
    try:
        app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui'),'--play','com.gamepkg.arkanoo'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        deadline=time.monotonic()+20
        while time.monotonic()<deadline:
            if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
            found=subprocess.run(['xdotool','search','--onlyvisible','--name','^GamePKG$'],env=env,capture_output=True,text=True,timeout=2)
            if found.returncode==0:break
            time.sleep(.1)
        assert found.returncode==0
        window=found.stdout.splitlines()[0];subprocess.run(['xdotool','windowfocus',window],env=env,check=True);time.sleep(.5)
        capture(window,'native-title.png')
        subprocess.run(['xdotool','key','--window',window,'Up'],env=env,check=True);time.sleep(.2)
        before=capture(window,'native-start.png')
        subprocess.run(['xdotool','keydown','--window',window,'Right'],env=env,check=True);time.sleep(.3)
        subprocess.run(['xdotool','keyup','--window',window,'Right'],env=env,check=True)
        after=capture(window,'native-right.png');assert paddle(after)>paddle(before)+25
        subprocess.run(['xdotool','key','--window',window,'Down'],env=env,check=True);time.sleep(.2)
        capture(window,'native-paused.png')
        print('Native Arkanoo: visible campaign, start, held-key paddle movement and pause passed.')
    finally:
        if app is not None:
            app.terminate()
            try:app.wait(timeout=10)
            except subprocess.TimeoutExpired:app.kill();app.wait(timeout=5)
        display.terminate();display.wait(timeout=10)
    if args.skip_browser:raise SystemExit(0)
    # Both actual runtime profiles, not a hand-written replacement for the game.
    import shutil
    for component in [False,True]:
        game_id='com.gamepkg.arkanoo'
        if component:
            source=root/'component';shutil.copytree(PROJECT/'examples/good/arkanoo',source)
            manifest=source/'manifest.toml';manifest.write_text('runtime_profile = "component-v1"\n'+manifest.read_text().replace('com.gamepkg.arkanoo','com.gamepkg.arkanoo-component'))
            game_id+='-component'
            run('pack',source,'-o',root/'component.gamepkg','--key',key);run('install',root/'component.gamepkg','--trust-creator');run('build',game_id)
        out=root/('component-web' if component else 'core-web');run('web-export',game_id,'--output',out)
        server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(out)));threading.Thread(target=server.serve_forever,daemon=True).start()
        options=Options();options.binary_location='/usr/bin/chromium'
        for arg in ['--headless=new','--no-sandbox','--disable-dev-shm-usage','--enable-unsafe-swiftshader','--window-size=1200,1000']:options.add_argument(arg)
        browser=webdriver.Chrome(service=Service('/usr/bin/chromedriver'),options=options)
        try:
            browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20)
            wait.until(lambda b:b.find_element(By.ID,'status').text=='Ready to play')
            browser.find_element(By.ID,'play').click();canvas=browser.find_element(By.ID,'game')
            wait.until(lambda b:canvas.get_attribute('data-state'))
            ActionChains(browser).click(canvas).send_keys(Keys.ARROW_UP).perform();time.sleep(.15)
            def pixels():return browser.execute_script('return Array.from(document.querySelector("#game").getContext("2d").getImageData(0,0,640,480).data)')
            def center(data):
                xs=[i%640 for i in range(420*640,440*640) if data[i*4:i*4+3]==[99,220,255]]
                assert len(xs)>100
                return sum(xs)/len(xs)
            left=center(pixels())
            ActionChains(browser).key_down(Keys.ARROW_RIGHT).pause(.3).key_up(Keys.ARROW_RIGHT).perform()
            assert center(pixels())>left+25
            ActionChains(browser).send_keys(Keys.ARROW_DOWN).perform();time.sleep(.15)
            paused=pixels();time.sleep(.25);assert pixels()==paused,'pause changed pixels'
            ActionChains(browser).send_keys(Keys.ARROW_DOWN).perform();time.sleep(2)
            assert pixels()!=paused
            canvas.screenshot(str(PROOF/('browser-component.png' if component else 'browser-core.png')))
            browser.find_element(By.ID,'stop').click()
            assert 'Stopped' in browser.find_element(By.ID,'status').text
            print(('Component' if component else 'Core')+' browser Arkanoo: held movement, pause/resume and export passed.')
        finally:browser.quit();server.shutdown();server.server_close()
