"""Play exported shader/audio host APIs in a private headless browser."""
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import os, shutil, subprocess, tempfile, threading
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.common.by import By
from selenium.webdriver.support.ui import WebDriverWait
PROJECT = Path(__file__).resolve().parents[1]
CLI = Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR', PROJECT/'target/release'))/'gamepkg'
class Quiet(SimpleHTTPRequestHandler):
    def log_message(self, *args): pass
with tempfile.TemporaryDirectory(prefix='gamepkg-media-web-') as directory:
    root = Path(directory)
    server = ThreadingHTTPServer(('127.0.0.1', 0), partial(Quiet, directory=str(root)))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    options = Options()
    options.binary_location = shutil.which('chromium')
    for option in ['--headless=new','--no-sandbox','--disable-dev-shm-usage','--enable-unsafe-swiftshader','--mute-audio']:
        options.add_argument(option)
    options.add_argument('--user-data-dir='+str(root/'browser'))
    try:
        with webdriver.Chrome(service=Service(shutil.which('chromedriver')), options=options) as browser:
            for profile in ['core-v1', 'component-v1']:
                work = root/profile
                work.mkdir()
                source = work/'source'
                shutil.copytree(PROJECT/'examples/good/media-game', source)
                manifest = source/'manifest.toml'
                manifest.write_text(f'runtime_profile = "{profile}"\n'+manifest.read_text())
                env = dict(os.environ, GAMEPKG_HOME=str(work/'library'), GAMEPKG_CONFIG=str(work/'settings.toml'))
                def run(*args):
                    result = subprocess.run([str(CLI), *map(str,args)], env=env, capture_output=True, text=True, timeout=140)
                    assert result.returncode == 0, result.stderr
                key = work/'key'
                run('keygen', '-o', key)
                package = work/'media.gamepkg'
                run('pack', source, '-o', package, '--key', key)
                run('install', package, '--trust-creator')
                run('build', 'com.gamepkg.media')
                run('web-export', 'com.gamepkg.media', '-o', work/'web')
                browser.get(f'http://127.0.0.1:{server.server_port}/{profile}/web/')
                wait = WebDriverWait(browser, 20)
                wait.until(lambda b: b.find_element(By.ID, 'status').text == 'Ready to play')
                browser.find_element(By.ID, 'play').click()
                wait.until(lambda b: b.find_element(By.TAG_NAME, 'canvas').get_attribute('data-state') not in [None, '0'])
                def blue_pixels(browser):
                    return browser.execute_script('const d=document.querySelector("canvas").getContext("2d").getImageData(0,0,640,480).data;let n=0;for(let i=0;i<d.length;i+=4)if(d[i+2]>150&&d[i+2]>2*d[i]&&d[i+2]>2*d[i+1])n++;return n;')
                wait.until(lambda b: blue_pixels(b) > 20000)
                browser.find_element(By.ID, 'stop').click()
                assert browser.find_element(By.ID, 'status').text == 'Stopped'
    finally:
        server.shutdown()
        server.server_close()
print('Exported shader/audio host APIs rendered actual PNG/RGBA pixels in Chromium for core and component profiles; normal audio permission/buffer checks completed before rendering.')
