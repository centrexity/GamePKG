// Trusted point-to-point transport: one credited decoder block in flight.
// The receiver must be polled after each output quantum to replenish capacity.
import {BLOCK_FRAMES} from './recorded-queue.js';
const generation=value=>Number.isSafeInteger(value)&&value>0;
const pcm=value=>value instanceof Int16Array&&value.buffer instanceof ArrayBuffer&&value.byteOffset===0&&value.buffer.byteLength===value.byteLength&&value.length<=BLOCK_FRAMES*2&&value.length%2===0;
export class RecordedMusicReceiver {
 #port;#stream;#credit=0;#pending=0;#closed=false;#ended=false;
 constructor(port,stream){this.#port=port;this.#stream=stream;port.onmessage=({data})=>this.#receive(data);port.onmessageerror=()=>this.#fail();port.start();}
 #fail(){if(this.#closed)return;this.#ended=true;this.#pending=0;this.#stream.failMusic(this.#stream.generation);this.#port.postMessage({kind:'failed',generation:this.#stream.generation});}
 #receive(data){
  if(this.#closed||!data)return;
  if(data.kind==='closed'){this.close();return;}
  if(!generation(data.generation))return;
  if(data.kind==='begin'||data.kind==='stop'){
   const accepted=data.kind==='begin'?this.#stream.begin(data.generation):this.#stream.stopMusic(data.generation);
   if(!accepted)return;
   this.#pending=0;this.#ended=data.kind==='stop';this.poll();return;
  }
  // A response from a retired decoder cannot affect the current track.
  if(data.generation!==this.#stream.generation)return;
  if(this.#ended)return;
  if(data.kind==='failed'){this.#fail();return;}
  if(data.kind!=='pcm'||data.credit!==this.#pending||!this.#pending||!pcm(data.values)){this.#fail();return;}
  this.#pending=0;
  if(!data.values.length){this.#ended=true;this.#stream.finishMusic(data.generation);return;}
  if(!this.#stream.push(data.generation,data.values)){this.#fail();return;}
  this.poll();
 }
 poll(){
  if(this.#closed||this.#ended||this.#pending||this.#stream.availableFrames<BLOCK_FRAMES)return;
  if(!generation(this.#stream.generation))return;
  if(this.#credit===Number.MAX_SAFE_INTEGER){this.#fail();return;}
  this.#pending=++this.#credit;
  this.#port.postMessage({kind:'read',generation:this.#stream.generation,credit:this.#pending,frames:BLOCK_FRAMES});
 }
 close(){if(this.#closed)return;this.#closed=true;this.#stream.failMusic(this.#stream.generation);this.#pending=0;this.#port.postMessage({kind:'closed',generation:this.#stream.generation});this.#port.onmessage=null;this.#port.onmessageerror=null;this.#port.close();}
}
export class RecordedMusicSender {
 #port;#decoder=null;#generation=0;#busy=false;#closed=false;#credit=0;
 constructor(port){this.#port=port;port.onmessage=({data})=>this.#receive(data);port.onmessageerror=()=>this.#fail();port.start();}
 #retire(){this.#decoder?.close();this.#decoder=null;this.#busy=false;}
 #fail(){if(this.#closed)return;this.#retire();this.#port.postMessage({kind:'failed',generation:this.#generation});}
 start(next,decoder){
  if(this.#closed||!generation(next)||next<=this.#generation||!decoder||typeof decoder.read!=='function'||typeof decoder.close!=='function')return false;
  this.#retire();this.#generation=next;this.#decoder=decoder;this.#port.postMessage({kind:'begin',generation:next});return true;
 }
 stop(next){if(this.#closed||!generation(next)||next<=this.#generation)return false;this.#retire();this.#generation=next;this.#port.postMessage({kind:'stop',generation:next});return true;}
 async #receive(data){
  if(this.#closed||!data)return;
  if(data.kind==='closed'){this.close();return;}
  if(data.generation!==this.#generation)return;
  if(data.kind==='failed'){this.#retire();return;}
  if(data.kind!=='read'||!generation(data.credit)||data.credit<=this.#credit||data.frames!==BLOCK_FRAMES||!this.#decoder||this.#busy){this.#fail();return;}
  const decoder=this.#decoder,current=this.#generation;this.#credit=data.credit;this.#busy=true;
  try{
   const values=await decoder.read(BLOCK_FRAMES*2);
   if(this.#closed||this.#generation!==current||this.#decoder!==decoder)return;
   if(!pcm(values))throw Error('Invalid credited PCM block');
   const ended=values.length===0;this.#busy=false;this.#port.postMessage({kind:'pcm',generation:current,credit:data.credit,values},[values.buffer]);
   if(ended)this.#retire();
  }catch{
   if(!this.#closed&&this.#generation===current&&this.#decoder===decoder)this.#fail();
  }
 }
 close(){if(this.#closed)return;this.#closed=true;this.#retire();this.#port.postMessage({kind:'closed',generation:this.#generation});this.#port.onmessage=null;this.#port.onmessageerror=null;this.#port.close();}
}
