"""Use the actual Hindsight Database/IndexedDB package in real browser storage."""
import argparse,functools,hashlib,http.server,json,shutil,tempfile,threading
from pathlib import Path
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
PROJECT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser();parser.add_argument('--indexeddb-package',type=Path,required=True);args=parser.parse_args()
package=args.indexeddb_package/'code_langs/js.js'
source=package.read_bytes()
with tempfile.TemporaryDirectory(prefix='gamepkg-hindsight-') as directory:
    root=Path(directory)
    (root/'hindsight.js').write_bytes(source)
    shutil.copy2(PROJECT/'html/library/storage.js',root/'storage.js')
    (root/'index.html').write_text('''<!doctype html><script src="hindsight.js"></script><script type="module">
    import {HindsightLibraryStorage} from './storage.js';
    window.HindsightLibraryStorage=HindsightLibraryStorage;
    </script>''')
    class Handler(http.server.SimpleHTTPRequestHandler):
        def end_headers(self):
            self.send_header('Cross-Origin-Opener-Policy','same-origin');self.send_header('Cross-Origin-Embedder-Policy','require-corp');super().end_headers()
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
                WebDriverWait(browser,15).until(lambda b:b.execute_script('return typeof HindsightLibraryStorage !== "undefined"'))
                result=browser.execute_async_script('''const done=arguments[arguments.length-1],snapshot=arguments[0],restart=arguments[1];
                (async()=>{const store=new HindsightLibraryStorage('test-library');await store.open();
                if(!restart){if(await store.loadLibrary()!==null)throw new Error('Unexpected initial data');await store.saveLibrary(snapshot);}
                const saved=await store.loadLibrary();const other=new HindsightLibraryStorage('other-library');await other.open();
                if(await other.loadLibrary()!==null)throw new Error('Library scope leaked');await other.close();await store.close();return saved;})().then(done,e=>done({error:String(e)}));''',snapshot,restart)
                assert result==snapshot,result
        print('Actual Hindsight IndexedDB passed: library metadata, catalog, settings, play history, scope isolation and browser-restart persistence.')
        print('Hindsight package SHA-256: '+hashlib.sha256(source).hexdigest())
    finally:server.shutdown();server.server_close()
