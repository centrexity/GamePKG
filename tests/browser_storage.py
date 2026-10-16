"""Verify GamePKG browser cache persistence without external packages."""
import argparse,functools,hashlib,http.server,json,shutil,tempfile,threading
from pathlib import Path
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
PROJECT=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-browser-cache-') as directory:
    root=Path(directory)
    shutil.copy2(PROJECT/'html/library/storage.js',root/'storage.js')
    (root/'index.html').write_text('''<!doctype html><script type="module">
    import {GamePKGLibraryStorage} from './storage.js';
    window.GamePKGLibraryStorage=GamePKGLibraryStorage;
    </script>''')
    class Handler(http.server.SimpleHTTPRequestHandler):
        def log_message(self,*args):pass
    server=http.server.ThreadingHTTPServer(('127.0.0.1',0),functools.partial(Handler,directory=str(root)))
    threading.Thread(target=server.serve_forever,daemon=True).start()
    options=Options();options.binary_location=shutil.which('chromium');options.add_argument('--headless=new');options.add_argument('--no-sandbox');options.add_argument('--disable-dev-shm-usage');options.add_argument('--user-data-dir='+str(root/'profile'))
    snapshot={'format_version':1,'games':[{'id':'com.example.game','version':'0.1.0','creator_key':'ab'*32,'played_ms':1250,'achievements':['started'],'used_bytes':12345}], 'catalog':[{'id':'com.example.waiting','state':'not-downloaded'}], 'settings':{'mute_audio':True},'rejected':[]}
    try:
        for restart in [False,True]:
            with webdriver.Chrome(service=Service(shutil.which('chromedriver')),options=options) as browser:
                browser.set_script_timeout(30);browser.get(f'http://127.0.0.1:{server.server_port}')
                from selenium.webdriver.support.ui import WebDriverWait
                WebDriverWait(browser,15).until(lambda b:b.execute_script('return typeof GamePKGLibraryStorage !== "undefined"'))
                result=browser.execute_async_script('''const done=arguments[arguments.length-1],snapshot=arguments[0],restart=arguments[1];
                (async()=>{const store=new GamePKGLibraryStorage('test-library');await store.open();
                if(crossOriginIsolated)throw new Error('Cache test unexpectedly requires isolation');
                if(!restart){
                    if(await store.loadLibrary()!==null)throw new Error('Unexpected initial data');
                    await store.saveLibrary(snapshot);
                    let rejected=false;try{await store.saveLibrary({...snapshot,format_version:99});}catch(_){rejected=true;}if(!rejected)throw new Error('Accepted invalid snapshot');
                    rejected=false;try{await store.saveLibrary({...snapshot,scope:'other-library'});}catch(_){rejected=true;}if(!rejected)throw new Error('Accepted wrong scope');
                    rejected=false;try{await store.operation('readwrite',records=>{records.put('broken','test-library');throw new Error('forced rollback');});}catch(_){rejected=true;}if(!rejected)throw new Error('Failed write committed');
                    const icon={identity:'ab'.repeat(32),width:40,height:40,rgba_hex:'00'.repeat(6400)};await store.saveIcon(icon);
                    if((await store.loadIcon(icon.identity)).rgba_hex!==icon.rgba_hex)throw new Error('Icon did not persist');
                }
                const saved=await store.loadLibrary();const other=new GamePKGLibraryStorage('other-library');await other.open();
                if(await other.loadIcon('ab'.repeat(32))!==null)throw new Error('Icon scope leaked');
                if(await other.loadLibrary()!==null)throw new Error('Library scope leaked');await other.close();await store.close();return saved;})().then(done,e=>done({error:String(e)}));''',snapshot,restart)
                assert result==snapshot,result
        print('GamePKG browser cache passed: library metadata, catalog, settings, play history, scope isolation and browser-restart persistence.')
    finally:server.shutdown();server.server_close()
