// Trusted host dispatcher. Package assets are supplied by the verified player.
export class RecordedFrameDispatcher {
 #output;#assets;#cache=new Map();#pending=null;#busy=false;#closed=false;#effects;#music;
 constructor(output,assets,{effects=false,music=false}={}){
  if(typeof effects!=='boolean'||typeof music!=='boolean'||!(assets instanceof Map)||typeof output?.request!=='function'||typeof output?.loadWav!=='function')throw Error('Invalid recorded audio dispatcher');
  this.#output=output;this.#assets=assets;this.#effects=effects;this.#music=music;
 }
 async #discard(entries){for(const [path,id] of entries){if(this.#closed)return;const result=await this.#output.request('unload',{clip:id});if(!result.ok){await this.close();throw Error('Unable to discard staged audio asset');}entries.delete(path);}}
 async prepare(frame){
  if(this.#closed||this.#busy||this.#pending)return false;
  if(!Number.isInteger(frame?.epoch)||frame.epoch<0||frame.epoch>4294967295||!Number.isInteger(frame.serial)||frame.serial<1||frame.serial>4294967295||!Array.isArray(frame.recordedControls)||frame.recordedControls.length>32)return false;
  // Validate the complete wire batch before loading any asset or changing a cache.
  for(const item of frame.recordedControls){
   if(!Array.isArray(item)||item.length!==3)return false;
   const [op,path,value]=item;if(!Number.isInteger(op)||op<0||op>7||typeof path!=='string'||!Number.isInteger(value)||value<0||value>4294967295||!(op<4?this.#effects:this.#music))return false;
   if(op===0){if(value!==0||!path||!this.#assets.has(path))return false;}
   // Music track admission is wired separately; never silently drop Play/Stop.
   else if(op===4||op===7)return false;
   else if(path!==''||((op===1||op===5)&&value>128)||((op===2||op===6)&&value>1)||(op===3&&value!==0))return false;
  }
  this.#busy=true;const entries=new Map(),commands=[];
  try{
   for(const [op,path,value] of frame.recordedControls){
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
   this.#pending={epoch:frame.epoch,frame:frame.serial,entries};return true;
  }catch(error){if(!this.#closed)await this.#discard(entries);throw error;}finally{this.#busy=false;}
 }
 async finish(epoch,serial,success){
  const pending=this.#pending;
  if(this.#closed||this.#busy||!pending||pending.epoch!==epoch||pending.frame!==serial||typeof success!=='boolean')return false;
  this.#busy=true;this.#pending=null;
  try{
   const result=await this.#output.request('finish',{epoch,frame:serial,success});if(this.#closed)return false;
   if(!result.ok){await this.#discard(pending.entries);return false;}
   if(success)for(const [path,id] of pending.entries)this.#cache.set(path,id);else await this.#discard(pending.entries);
   return true;
  }catch(error){await this.close();throw error;}finally{this.#busy=false;}
 }
 async close(){if(this.#closed)return;this.#closed=true;this.#pending=null;this.#cache.clear();await this.#output.close();}
}
