import {RecordedMusicLoader} from './recorded-music-loader.js';
// Trusted host dispatcher. Package assets are supplied by the verified player.
export class RecordedFrameDispatcher {
 #output;#assets;#cache=new Map();#pending=null;#busy=false;#closed=false;#effects;#music;#loader;
 constructor(output,assets,{effects=false,music=false,musicLoader=null}={}){
  if(music&&!(musicLoader instanceof RecordedMusicLoader))throw Error('Music requires a supervised loader');
  if(typeof effects!=='boolean'||typeof music!=='boolean'||!(assets instanceof Map)||typeof output?.request!=='function'||typeof output?.loadWav!=='function')throw Error('Invalid recorded audio dispatcher');
  this.#output=output;this.#assets=assets;this.#effects=effects;this.#music=music;this.#loader=musicLoader;
 }
 async #discard(entries){for(const [path,id] of entries){if(this.#closed)return;const result=await this.#output.request('unload',{clip:id});if(!result.ok){await this.close();throw Error('Unable to discard staged audio asset');}entries.delete(path);}}
 async prepare(frame){
  if(this.#closed||this.#busy||this.#pending)return false;
  if(!Number.isInteger(frame?.epoch)||frame.epoch<0||frame.epoch>4294967295||!Number.isInteger(frame.serial)||frame.serial<1||frame.serial>4294967295||!Array.isArray(frame.recordedControls)||frame.recordedControls.length>32)return false;
  // Validate the complete wire batch before loading any asset or changing a cache.
  for(const item of frame.recordedControls){
   if(!Array.isArray(item)||item.length!==3)return false;
   const [op,path,value]=item;if(!Number.isInteger(op)||op<0||op>7||typeof path!=='string'||!Number.isInteger(value)||value<0||value>4294967295||!(op<4?this.#effects:this.#music))return false;
   if(op===0||op===4){if(value!==0||!path||!this.#assets.has(path))return false;if(op===4){const encoded=this.#assets.get(path);if(!(encoded instanceof Uint8Array)||encoded.length<1||encoded.length>8388608)return false;}}
   else if(path!==''||((op===1||op===5)&&value>128)||((op===2||op===6)&&value>1)||((op===3||op===7)&&value!==0))return false;
  }
  this.#busy=true;const entries=new Map(),commands=[];let musicAction=null;
  try{
   const snapshot=await this.#output.request('snapshot');if(this.#closed)return false;let generation=snapshot.generation;
   for(const [op,path,value] of frame.recordedControls){
    if(op===4||op===7){if(generation===Number.MAX_SAFE_INTEGER)throw Error('Music generation exhausted');generation++;musicAction={generation,encoded:op===4?this.#assets.get(path):null};commands.push([op,generation]);continue;}
    if(op!==0){commands.push([op,value]);continue;}
    let id=this.#cache.get(path)??entries.get(path);
    if(id===undefined){
     const used=new Set([...this.#cache.values(),...entries.values()]);for(id=0;id<32&&used.has(id);id++);
     if(id===32)throw Error('Recorded effect cache is full');
     const admitted=await this.#output.loadWav(id,this.#assets.get(path));if(this.#closed)return false;if(!admitted.ok)throw Error('Recorded effect asset was rejected');entries.set(path,id);
    }
    commands.push([0,id]);
   }
   const prepared=await this.#output.request('prepare',{epoch:frame.epoch,frame:frame.serial,commands,activeGeneration:frame.recordedActiveGeneration??null});
   if(this.#closed)return false;
   if(!prepared.ok){await this.#discard(entries);return false;}
   this.#pending={epoch:frame.epoch,frame:frame.serial,entries,musicAction};return true;
  }catch(error){if(!this.#closed)await this.#discard(entries);throw error;}finally{this.#busy=false;}
 }
 async finish(epoch,serial,success){
  const pending=this.#pending;
  if(this.#closed||this.#busy||!pending||pending.epoch!==epoch||pending.frame!==serial||typeof success!=='boolean')return false;
  this.#busy=true;this.#pending=null;
  try{
   const result=await this.#output.request('finish',{epoch,frame:serial,success});if(this.#closed)return false;
   if(!result.ok){await this.#discard(pending.entries);return false;}
   if(success){for(const [path,id] of pending.entries)this.#cache.set(path,id);const action=pending.musicAction;if(action&&! (action.encoded?this.#loader.play(action.generation,action.encoded):this.#loader.stop(action.generation)))throw Error('Committed music generation was rejected');}else await this.#discard(pending.entries);
   return true;
  }catch(error){await this.close();throw error;}finally{this.#busy=false;}
 }
 async close(){if(this.#closed)return;this.#closed=true;this.#pending=null;this.#cache.clear();await Promise.all([this.#loader?.close(),this.#output.close()]);}
}
