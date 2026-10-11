"""Test the actual local host and Hindsight-backed browser library."""
import argparse,json,os,shutil,subprocess,tempfile,urllib.error,urllib.request
from pathlib import Path
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.support.ui import WebDriverWait
PROJECT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('--indexeddb-package',type=Path,required=True);args=p.parse_args()
with tempfile.TemporaryDirectory(prefix='gamepkg-web-library-') as directory:
    root=Path(directory);env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'))
    (root/'settings.toml').write_text('automatic_update_checks=false\n')
    cli=PROJECT/'target/release/gamepkg'
    def run(*args):return subprocess.run([str(cli),*map(str,args)],env=env,check=True,capture_output=True,text=True,timeout=60)
    key=root/'key';package=root/'game.gamepkg';run('keygen','-o',key);run('pack',PROJECT/'examples/minimal','-o',package,'--key',key);run('install',package,'--trust-creator')
    catalog=root/'games.gamepkglist';catalog.write_text('format_version=1\n[[games]]\nid="com.example.waiting"\ntitle="Waiting game"\ndownload_url="https://unreachable.invalid/game.gamepkg"\n');run('import-list',catalog)
    host=subprocess.Popen([str(cli),'web-library','--indexeddb-package',str(args.indexeddb_package),'--port','0'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    try:
        line=host.stdout.readline();assert line.startswith('Open http://127.0.0.1:'),line
        url=line.strip().split(' ',1)[1];base,fragment=url.split('#');token,scope=fragment.split(':')
        def rejected(headers,method='GET',path='api/library',expected=403):
            try:urllib.request.urlopen(urllib.request.Request(base+path,headers=headers,method=method),timeout=5)
            except urllib.error.HTTPError as error:assert error.code==expected,error
            else:raise AssertionError('unauthorized request succeeded')
        rejected({});rejected({'Authorization':'Bearer wrong'});rejected({'Authorization':'Bearer '+token,'Origin':'https://other.invalid'});rejected({'Host':'other.invalid'});rejected({},method='POST',expected=405)
        request=urllib.request.Request(base+'api/library',headers={'Authorization':'Bearer '+token})
        snapshot=json.load(urllib.request.urlopen(request,timeout=5));assert snapshot['games'][0]['manifest']['id']=='com.gamepkg.minimal';assert snapshot['catalog'][0]['id']=='com.example.waiting'
        options=Options();options.binary_location=shutil.which('chromium');options.add_argument('--headless=new');options.add_argument('--no-sandbox');options.add_argument('--disable-dev-shm-usage');options.add_argument('--user-data-dir='+str(root/'profile'))
        with webdriver.Chrome(service=Service(shutil.which('chromedriver')),options=options) as browser:
            browser.get(url)
            WebDriverWait(browser,20).until(lambda b:'saved using Hindsight' in b.find_element('id','status').text)
            rows=browser.find_element('id','games').text;assert 'com.gamepkg.minimal' in rows and 'Waiting game' in rows and 'Not downloaded' in rows,rows
            browser.get(base+'#wrong:'+scope);browser.refresh()
            WebDriverWait(browser,20).until(lambda b:'showing cached metadata' in b.find_element('id','status').text)
            rows=browser.find_element('id','games').text;assert 'Cached installed metadata' in rows and 'Cached catalog metadata' in rows,rows
        print('Running browser library passed: authenticated host metadata, actual Hindsight storage, installed/catalog rows, cached display and cross-origin rejection.')
    finally:host.terminate();host.wait(timeout=10)
