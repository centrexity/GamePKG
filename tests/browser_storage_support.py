"""Inspect private browser fixture records through the standard IndexedDB API."""
_SCRIPT='''const [kind,prefix,value,write]=arguments,done=arguments[arguments.length-1];
const request=indexedDB.open('gamepkg-progress-v1',1);
request.onupgradeneeded=()=>{for(const name of ['saves','history'])if(!request.result.objectStoreNames.contains(name))request.result.createObjectStore(name);};
request.onerror=()=>done({error:String(request.error)});
request.onsuccess=()=>{const db=request.result,tx=db.transaction(kind,write?'readwrite':'readonly'),store=tx.objectStore(kind),keys=store.getAllKeys();let values=[],failure;
keys.onsuccess=()=>{const matching=keys.result.filter(key=>typeof key==='string'&&key.startsWith(prefix));
 if(write){if(!matching.length){failure='Fixture record is missing';tx.abort();return;}store.put(kind==='saves'?new Uint8Array(value):value,matching[0]);}
 else for(const key of matching){const r=store.get(key);r.onsuccess=()=>values.push(kind==='saves'?Array.from(r.result):r.result);}
};
tx.oncomplete=()=>{db.close();done({values});};tx.onabort=()=>{db.close();done({error:failure||String(tx.error)});};};'''
def read_records(browser,kind,prefix):
 result=browser.execute_async_script(_SCRIPT,kind,prefix,None,False)
 assert 'error' not in result,result
 return result['values']
def read_first(browser,kind,prefix):
 values=read_records(browser,kind,prefix)
 return values[0] if values else None
def write_first(browser,kind,prefix,value):
 result=browser.execute_async_script(_SCRIPT,kind,prefix,value,True)
 assert 'error' not in result,result
