// Trusted browser persistence. No database handle is exposed to guest code.
export const DATABASE='gamepkg-progress-v1';
const MAX_SAVE=1048576,MAX_HISTORY=65536,MAX_UNLOCKS=1024,IO_DEADLINE=5000;
export class ProgressDataError extends Error {}
const invalid=message=>{throw new ProgressDataError(message);};
export function normalizeSave(value){
 if(!(value instanceof Uint8Array)&&!Array.isArray(value))invalid('Invalid browser save');
 if(value.length>MAX_SAVE)invalid('Browser save exceeds its limit');
 if(Array.isArray(value))for(let i=0;i<value.length;i++)if(!Number.isInteger(value[i])||value[i]<0||value[i]>255)invalid('Invalid browser save byte');
 return new Uint8Array(value);
}
export function normalizeHistory(value,defaults=true){
 if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).some(key=>!['played_ms','unlocked'].includes(key))||(!defaults&&(!Object.hasOwn(value,'played_ms')||!Object.hasOwn(value,'unlocked'))))invalid('Invalid browser play history');
 const played_ms=value.played_ms===undefined?0:value.played_ms,unlocked=value.unlocked===undefined?[]:value.unlocked;
 if(!Number.isSafeInteger(played_ms)||played_ms<0||!Array.isArray(unlocked)||unlocked.length>MAX_UNLOCKS||new Set(unlocked).size!==unlocked.length)invalid('Invalid browser play history');
 for(const id of unlocked)if(typeof id!=='string'||!/^[A-Za-z0-9_-]{1,64}$/.test(id))invalid('Invalid browser play history');
 const history={played_ms,unlocked:[...unlocked]};
 if(JSON.stringify(history).length>MAX_HISTORY)invalid('Browser play history exceeds its limit');
 return history;
}
function legacy(storage,key,fallback,maximum=MAX_SAVE*4+2){
 const text=(storage??globalThis.localStorage).getItem(key);if(text===null)return fallback;
 if(text.length>maximum)invalid('Legacy browser record exceeds its limit');
 try{return JSON.parse(text);}catch{invalid('Invalid legacy browser record');}
}
export class ProgressStore {
 constructor(saveKey,historyKey,factory=globalThis.indexedDB,storage=null){
  if(typeof saveKey!=='string'||typeof historyKey!=='string'||!saveKey.startsWith('gamepkg:save:')||!historyKey.startsWith('gamepkg:history:'))throw Error('Invalid browser progress scope');
  this.saveKey=saveKey;this.historyKey=historyKey;this.factory=factory;this.storage=storage;
  this.database=null;this.opening=null;this.active=new Set();this.loaded=false;this.serial=0;this.openEpoch=0;
 }
 open(){
  if(this.database)return Promise.resolve(this.database);if(this.opening)return this.opening;
  const epoch=this.openEpoch;
  this.opening=new Promise((resolve,reject)=>{
   let settled=false;const fail=error=>{if(settled)return;settled=true;clearTimeout(timer);reject(error);};
   const timer=setTimeout(()=>fail(Error('Browser progress database timed out')),IO_DEADLINE);
   let request;try{if(!this.factory)throw Error('IndexedDB is unavailable');request=this.factory.open(DATABASE,1);}catch(error){fail(error);return;}
   request.onupgradeneeded=()=>{if(epoch!==this.openEpoch){request.transaction.abort();return;}const db=request.result;for(const name of ['saves','history'])if(!db.objectStoreNames.contains(name))db.createObjectStore(name);};
   request.onerror=()=>fail(request.error||Error('Browser progress database unavailable'));
   request.onblocked=()=>fail(Error('Browser progress database is blocked'));
   request.onsuccess=()=>{
    const db=request.result;if(epoch!==this.openEpoch){fail(Error('Browser progress database closed'));db.close();return;}if(settled){db.close();return;}settled=true;clearTimeout(timer);this.database=db;
    const closed=()=>{if(this.database===db){this.database=null;this.opening=null;}};db.onclose=closed;db.onversionchange=()=>{db.close();closed();};resolve(db);
   };
  });const pending=this.opening;pending.catch(()=>{if(this.opening===pending)this.opening=null;});return pending;
 }
 transaction(db,mode,body){
  return new Promise((resolve,reject)=>{
   let tx;try{tx=db.transaction(['saves','history'],mode);}catch(error){reject(error);return;}
   this.active.add(tx);let result,failure;
   const timer=setTimeout(()=>{failure=Error('Browser progress transaction timed out');try{tx.abort();}catch{}},IO_DEADLINE);
   const done=()=>{clearTimeout(timer);this.active.delete(tx);};
   tx.oncomplete=()=>{done();resolve(result);};
   tx.onabort=()=>{done();reject(failure||tx.error||Error('Browser progress transaction aborted'));};
   const abort=error=>{failure=error;try{tx.abort();}catch{done();reject(error);}};
   try{body(tx,value=>{result=value;},abort);}catch(error){abort(error);}
  });
 }
 async load(){
  const serial=this.serial;this.loaded=false;
  const db=await this.open();if(serial!==this.serial)throw Error('Browser progress read cancelled');
  const record=await this.transaction(db,'readonly',(tx,result)=>{
   const save=tx.objectStore('saves').get(this.saveKey),history=tx.objectStore('history').get(this.historyKey);
   history.onsuccess=()=>result({save:save.result,history:history.result});
  });
  if(serial!==this.serial)throw Error('Browser progress read cancelled');
  this.fallbackSave=normalizeSave(record.save===undefined?legacy(this.storage,this.saveKey,[]):record.save);
  this.fallbackHistory=normalizeHistory(record.history===undefined?legacy(this.storage,this.historyKey,{},MAX_HISTORY):record.history,record.history===undefined);
  this.missingSave=record.save===undefined;this.loaded=true;return {save:this.fallbackSave.slice(),history:normalizeHistory(this.fallbackHistory)};
 }
 async commit(save,elapsed,unlocks){
  if(!this.loaded)throw Error('Browser progress was not loaded');
  if(!Number.isInteger(elapsed)||elapsed<0||elapsed>250||!Array.isArray(unlocks)||unlocks.length>MAX_UNLOCKS)invalid('Invalid browser progress update');
  const pending=normalizeHistory({played_ms:0,unlocked:unlocks});
  const serial=this.serial,bytes=save===null?null:normalizeSave(save),db=await this.open();
  if(serial!==this.serial)throw Error('Browser progress write cancelled');
  const committed=await this.transaction(db,'readwrite',(tx,result,abort)=>{
   const saves=tx.objectStore('saves'),history=tx.objectStore('history');
   const oldSave=this.missingSave?saves.get(this.saveKey):null,oldHistory=history.get(this.historyKey);
   oldHistory.onsuccess=()=>{try{
    // Read/modify/write under the same transaction, including concurrent tabs.
    const next=normalizeHistory(oldHistory.result===undefined?this.fallbackHistory:oldHistory.result,false);
    next.played_ms=Math.min(Number.MAX_SAFE_INTEGER,next.played_ms+elapsed);
    for(const id of pending.unlocked)if(!next.unlocked.includes(id))next.unlocked.push(id);
    const checked=normalizeHistory(next);
    if(bytes!==null)saves.put(bytes,this.saveKey);
    else if(oldSave&&oldSave.result===undefined)saves.put(this.fallbackSave,this.saveKey);
    history.put(checked,this.historyKey);result(checked);
   }catch(error){abort(error);}};
  });this.missingSave=false;return committed;
 }
 cancel(){this.serial++;for(const tx of this.active){try{tx.abort();}catch{}}}
 close(){this.openEpoch++;this.cancel();this.database?.close();this.database=null;this.opening=null;}
}
