"""Test the actual local host and GamePKG-owned browser library."""
import argparse,json,os,shutil,subprocess,tempfile,urllib.error,urllib.request
from pathlib import Path
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.support.ui import WebDriverWait
PROJECT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('--play',action='store_true',help='Compile and play a signed achievement fixture, then verify persistent browser history');args=p.parse_args()
with tempfile.TemporaryDirectory(prefix='gamepkg-web-library-') as directory:
    root=Path(directory);env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'))
    (root/'settings.toml').write_text('automatic_update_checks=false\n')
    cli=PROJECT/'target/release/gamepkg'
    def run(*args):
     if args and args[0] in ('build','integrity-fix') and '--target' not in args: args=(*args,'--target','wasm')
     return subprocess.run([str(cli),*map(str,args)],env=env,check=True,capture_output=True,text=True,timeout=60)
    source=root/'source';shutil.copytree(PROJECT/'examples/good/collect-stars',source)
    manifest=source/'manifest.toml';manifest.write_text(manifest.read_text().replace('Find and collect stars around the board. Move with the arrow keys or WASD, improve your score and earn local achievements.','Literal <img src=x onerror=window.__metadataExecuted=true> description.'))
    if args.play:
        (source/'source/src/lib.rs').write_text('pub fn game_tick(_:u32,state:u64)->u64 { if !(gamepkg::achievement_unlock(0)) { loop {} }; state + 1 }')
    key=root/'key';package=root/'game.gamepkg';run('keygen','-o',key);run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator')
    catalog=root/'games.gamepkglist';catalog.write_text('format_version=1\n[[games]]\nid="com.example.waiting"\ntitle="Waiting game"\ndescription="Waiting description."\ndownload_url="https://unreachable.invalid/game.gamepkg"\n');run('import-list',catalog)
    host=subprocess.Popen([str(cli),'web-library','--port','0'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    try:
        line=host.stdout.readline();assert line.startswith('Open http://127.0.0.1:'),line+host.stderr.read()
        url=line.strip().split(' ',1)[1];base,fragment=url.split('#');token,scope=fragment.split(':')
        def rejected(headers,method='GET',path='api/library',expected=403):
            try:urllib.request.urlopen(urllib.request.Request(base+path,headers=headers,method=method),timeout=5)
            except urllib.error.HTTPError as error:assert error.code==expected,error
            else:raise AssertionError('unauthorized request succeeded')
        rejected({});rejected({'Authorization':'Bearer wrong'});rejected({'Authorization':'Bearer '+token,'Origin':'https://other.invalid'});rejected({'Host':'other.invalid'});rejected({},method='POST',expected=405)
        request=urllib.request.Request(base+'api/library',headers={'Authorization':'Bearer '+token})
        snapshot=json.load(urllib.request.urlopen(request,timeout=5));assert json.loads((root/'library/database/library.json').read_text()) == snapshot;assert snapshot['games'][0]['manifest']['id']=='com.gamepkg.collect-stars';assert snapshot['catalog'][0]['id']=='com.example.waiting'
        rejected({},path='api/icon/com.gamepkg.collect-stars')
        rejected({'Authorization':'Bearer '+token},path='api/icon/missing-game',expected=404)
        rejected({'Authorization':'Bearer '+token},path='api/icon/..',expected=404)
        icon=json.load(urllib.request.urlopen(urllib.request.Request(base+'api/icon/com.gamepkg.collect-stars',headers={'Authorization':'Bearer '+token}),timeout=5))
        assert icon['width']==40 and len(icon['rgba_hex'])==12800 and icon['identity']==snapshot['games'][0]['icon_identity']
        options=Options();options.binary_location=shutil.which('chromium');options.add_argument('--headless=new');options.add_argument('--no-sandbox');options.add_argument('--disable-dev-shm-usage');options.add_argument('--user-data-dir='+str(root/'profile'))
        with webdriver.Chrome(service=Service(shutil.which('chromedriver')),options=options) as browser:
            browser.get(url)
            WebDriverWait(browser,20).until(lambda b:'saved in the GamePKG browser cache' in b.find_element('id','status').text)
            WebDriverWait(browser,20).until(lambda b:len(b.find_elements('css selector','#games canvas'))==1)
            rows=browser.find_element('id','games').text;assert 'com.gamepkg.collect-stars' in rows and 'Waiting game' in rows and 'Not downloaded' in rows,rows
            assert 'Collect Stars' in rows and 'Literal <img src=x onerror=window.__metadataExecuted=true> description.' in rows,rows
            assert browser.execute_script("return window.__metadataExecuted !== true && document.querySelectorAll('#games img').length===0")
            browser.find_element('css selector','#games details summary').click()
            achievements=browser.find_elements('css selector','#games td:nth-child(6) details summary');assert len(achievements)==1
            achievements[0].click();rows=browser.find_element('id','games').text
            assert 'Not built' in rows,rows
            assert 'Locked — First star' in rows and 'Star collector' in rows and 'bytes expected' in rows,rows
            assert 'Library storage used:' in browser.find_element('id','library-space').text
            if args.play:
                run('build','com.gamepkg.collect-stars');run('run','com.gamepkg.collect-stars','--frames','10000')
                played=json.load(urllib.request.urlopen(request,timeout=5))['games'][0]
                assert played['built'] and played['history']['played_ms']>0
                assert set(played['history']['unlocked'])=={'first_star'}
                browser.find_element('id','refresh').click()
                WebDriverWait(browser,20).until(lambda b:'Not built' not in b.find_element('id','games').text and 'Built' in b.find_element('id','games').text)
                browser.find_element('css selector','#games td:nth-child(6) details summary').click()
                rows=browser.find_element('id','games').text
                assert 'Unlocked — First star' in rows and 'Locked — Star collector' in rows,rows
            browser.get(base+'#wrong:'+scope);browser.refresh()
            WebDriverWait(browser,20).until(lambda b:'showing cached metadata' in b.find_element('id','status').text)
            WebDriverWait(browser,20).until(lambda b:len(b.find_elements('css selector','#games canvas'))==1)
            rows=browser.find_element('id','games').text;assert 'Cached installed metadata' in rows and 'Cached catalog metadata' in rows,rows
        if args.play:
            with webdriver.Chrome(service=Service(shutil.which('chromedriver')),options=options) as browser:
                browser.get(base+'#wrong:'+scope)
                WebDriverWait(browser,20).until(lambda b:'showing cached metadata' in b.find_element('id','status').text)
                browser.find_element('css selector','#games td:nth-child(6) details summary').click()
                assert 'Unlocked — First star' in browser.find_element('id','games').text
                cached=browser.execute_async_script('''const done=arguments[0];import('/storage.js').then(async ({GamePKGLibraryStorage})=>{const storage=new GamePKGLibraryStorage(location.hash.slice(1).split(':')[1]);await storage.open();const snapshot=await storage.loadLibrary();await storage.close();done(snapshot.games[0].history);}).catch(error=>done({error:String(error)}));''')
                assert cached==played['history'],cached
                WebDriverWait(browser,20).until(lambda b:len(b.find_elements('css selector','#games canvas'))==1)
            print('Actual gameplay history and achievements persisted through the GamePKG browser cache and a browser restart with host authorization unavailable.')
        # Cache restrictions must not hide current authenticated host data or icons.
        isolated=Options();isolated.binary_location=shutil.which('chromium')
        for argument in ['--headless=new','--no-sandbox','--disable-dev-shm-usage']:isolated.add_argument(argument)
        with webdriver.Chrome(service=Service(shutil.which('chromedriver')),options=isolated) as browser:
            browser.execute_cdp_cmd('Page.addScriptToEvaluateOnNewDocument',{'source':"Object.defineProperty(window,'indexedDB',{get(){throw new DOMException('Blocked by test','SecurityError')}});"})
            browser.get(url)
            WebDriverWait(browser,20).until(lambda b:'Library loaded from host. Browser cache unavailable' in b.find_element('id','status').text)
            WebDriverWait(browser,20).until(lambda b:len(b.find_elements('css selector','#games canvas'))==1)
            assert 'Collect Stars' in browser.find_element('id','games').text
        print('Running browser library passed: authenticated host metadata, GamePKG-owned browser storage, installed/catalog rows, live build status, verified and cached icons, achievement descriptions, expected storage, cached display and cross-origin rejection.')
    finally:host.terminate();host.wait(timeout=10)
