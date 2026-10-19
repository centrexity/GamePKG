"""Actual IndexedDB atomicity, migration, concurrency and corruption checks."""
from pathlib import Path
from functools import partial
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import hashlib,json,os,shutil,tempfile,threading
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service
from selenium.webdriver.support.ui import WebDriverWait
project=Path(__file__).resolve().parents[1]
class Quiet(SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-browser-store-') as temporary:
 root=Path(temporary);shutil.copyfile(project/'web-player/progress-store.js',root/'progress-store.js')
 (root/'index.html').write_text('<script type="module">import * as api from "./progress-store.js";window.progressAPI=api;</script>')
 server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(root)));threading.Thread(target=server.serve_forever,daemon=True).start()
 options=Options();options.binary_location=shutil.which('google-chrome') or shutil.which('google-chrome-stable') or shutil.which('chromium');assert options.binary_location
 for flag in ['--headless=new','--no-sandbox','--disable-dev-shm-usage']:options.add_argument(flag)
 driver=shutil.which('chromedriver') or str(Path(os.environ.get('CHROMEWEBDRIVER','/nonexistent'))/'chromedriver')
 browser=webdriver.Chrome(service=Service(driver),options=options)
 try:
  browser.get(f'http://127.0.0.1:{server.server_port}/');WebDriverWait(browser,20).until(lambda b:b.execute_script('return !!window.progressAPI'))
  browser.set_script_timeout(30)
  result=browser.execute_async_script('''const done=arguments[arguments.length-1];(async()=>{
   const {ProgressStore,ProgressDataError}=window.progressAPI,assert=(value,message)=>{if(!value)throw Error(message);};
   const saveKey='gamepkg:save:fixture:creator:1',historyKey='gamepkg:history:fixture:creator';
   const legacySave=JSON.stringify([0,255,3]),legacyHistory=JSON.stringify({played_ms:17,unlocked:['legacy']});
   localStorage.setItem(saveKey,legacySave);localStorage.setItem(historyKey,legacyHistory);
   const closing=new ProgressStore(saveKey,historyKey),opening=closing.load();closing.close();let closed=false;try{await opening;}catch{closed=true;}assert(closed&&closing.database===null,'late database open survived close');await closing.load();closing.close();
   const a=new ProgressStore(saveKey,historyKey);let loaded=await a.load();
   assert(String(loaded.save)==='0,255,3'&&loaded.history.played_ms===17,'legacy read changed values');
   const read=async(store,name,key)=>{const db=await store.open();return new Promise((resolve,reject)=>{const tx=db.transaction(name),r=tx.objectStore(name).get(key);tx.oncomplete=()=>resolve(r.result);tx.onabort=()=>reject(tx.error);});};
   const write=async(store,name,key,value)=>{const db=await store.open();return new Promise((resolve,reject)=>{const tx=db.transaction(name,'readwrite');tx.objectStore(name).put(value,key);tx.oncomplete=resolve;tx.onabort=()=>reject(tx.error);});};
   assert(await read(a,'saves',saveKey)===undefined&&await read(a,'history',historyKey)===undefined,'load wrote migration prematurely');
   await a.commit(new Uint8Array([7,8,9]),25,['new']);
   const baseline={save:Array.from(await read(a,'saves',saveKey)),history:await read(a,'history',historyKey)};
   assert(String(baseline.save)==='7,8,9'&&baseline.history.played_ms===42&&String(baseline.history.unlocked)==='legacy,new','migration did not commit both values');
   assert(localStorage.getItem(saveKey)===legacySave&&localStorage.getItem(historyKey)===legacyHistory,'migration deleted legacy recovery data');
   localStorage.setItem(saveKey,'malformed');localStorage.setItem(historyKey,'malformed');
   loaded=await a.load();assert(String(loaded.save)==='7,8,9'&&loaded.history.played_ms===42,'authoritative database reread legacy data');
   const descriptor=Object.getOwnPropertyDescriptor(window,'localStorage');Object.defineProperty(window,'localStorage',{configurable:true,get(){throw Error('legacy storage denied');}});
   try{const current=new ProgressStore(saveKey,historyKey);assert(String((await current.load()).save)==='7,8,9','existing database required legacy access');await current.commit(new Uint8Array([7,8,9]),0,[]);current.close();}
   finally{if(descriptor)Object.defineProperty(window,'localStorage',descriptor);else delete window.localStorage;}
   const old=a.database;old.close();old.dispatchEvent(new Event('close'));await a.load();const fresh=a.database;assert(fresh!==old,'closed database connection was reused');old.dispatchEvent(new Event('close'));assert(a.database===fresh,'late close discarded the new connection');

   const original=IDBObjectStore.prototype.put;let succeeded=false;
   IDBObjectStore.prototype.put=function(...args){const request=original.apply(this,args);if(this.name==='saves'&&args[1]===saveKey)request.addEventListener('success',()=>{succeeded=true;this.transaction.abort();});return request;};
   let rejected=false;try{await a.commit(new Uint8Array([99]),13,['aborted']);}catch{rejected=true;}finally{IDBObjectStore.prototype.put=original;}
   assert(rejected&&succeeded,'abort after a successful save request was not exercised');
   assert(String(await read(a,'saves',saveKey))==='7,8,9'&&JSON.stringify(await read(a,'history',historyKey))===JSON.stringify(baseline.history),'transaction abort partially committed');
   IDBObjectStore.prototype.put=function(...args){if(this.name==='history')throw new DOMException('fixture quota','QuotaExceededError');return original.apply(this,args);};
   rejected=false;try{await a.commit(new Uint8Array([88]),13,['quota']);}catch{rejected=true;}finally{IDBObjectStore.prototype.put=original;}
   assert(rejected&&String(await read(a,'saves',saveKey))==='7,8,9'&&JSON.stringify(await read(a,'history',historyKey))===JSON.stringify(baseline.history),'history failure retained the pending save');
   const cancelled=a.commit(new Uint8Array([77]),13,['cancelled']);a.cancel();rejected=false;try{await cancelled;}catch{rejected=true;}
   assert(rejected&&String(await read(a,'saves',saveKey))==='7,8,9','cancel before asynchronous start still wrote data');
   const maximum=new Uint8Array(1048576);for(let i=0;i<maximum.length;i++)maximum[i]=i%256;
   await a.load();await a.commit(maximum,1,['maximum']);loaded=await a.load();
   assert(loaded.save.length===maximum.length&&loaded.save.every((value,index)=>value===maximum[index]),'maximum save changed bytes');
   const digest=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',loaded.save)),b=>b.toString(16).padStart(2,'0')).join('');
   const b=new ProgressStore(saveKey,historyKey);await b.load();const before=loaded.history.played_ms;
   await Promise.all(Array.from({length:20},(_,i)=>(i%2?a:b).commit(null,1,['parallel_'+i])));
   loaded=await a.load();assert(loaded.history.played_ms===before+20&&Array.from({length:20},(_,i)=>'parallel_'+i).every(id=>loaded.history.unlocked.includes(id)),'concurrent writers lost history');
   const other=new ProgressStore('gamepkg:save:fixture:other:1','gamepkg:history:fixture:other');await other.load();await other.commit(new Uint8Array([55]),1,['other']);
   assert((await a.load()).save.length===1048576&&String((await other.load()).save)==='55','creator isolation failed');
   const broken=new ProgressStore('gamepkg:save:broken:creator:1','gamepkg:history:broken:creator');localStorage.setItem(broken.historyKey,'{invalid');
   rejected=false;try{await broken.load();}catch(error){rejected=error instanceof ProgressDataError;}
   assert(rejected&&localStorage.getItem(broken.historyKey)==='{invalid'&&await read(broken,'history',broken.historyKey)===undefined,'malformed legacy JSON was overwritten');
   const goodHistory=await read(a,'history',historyKey),badHistory={played_ms:0,unlocked:null};await write(a,'history',historyKey,badHistory);
   rejected=false;try{await a.load();}catch(error){rejected=error instanceof ProgressDataError;}
   assert(rejected&&JSON.stringify(await read(a,'history',historyKey))===JSON.stringify(badHistory)&&(await read(a,'saves',saveKey)).length===1048576,'corruption was silently replaced');
   await write(a,'history',historyKey,goodHistory);await a.load();await a.commit(null,1,['recovered']);
   for(const store of [a,b,other,broken])store.close();
   return {digest,cases:['early close','lazy legacy access','connection recovery','legacy migration','request-success abort','history quota rollback','early cancellation','maximum bytes','concurrent history','creator isolation','invalid JSON preservation','database corruption and recovery']};
  })().then(value=>done({value})).catch(error=>done({error:String(error),stack:error.stack}));''')
  assert 'error' not in result,result
  assert result['value']['digest']==hashlib.sha256(bytes(range(256))*4096).hexdigest(),result
  print('Actual IndexedDB:',', '.join(result['value']['cases']),'passed; complete 1 MiB save hash verified.')
 finally:browser.quit();server.shutdown();server.server_close()
