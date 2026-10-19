// Host-owned asynchronous track loading after a successful frame commit.
import {RecordedDecoderClient} from './recorded-decoder-client.js';
import {RecordedMusicSender} from './recorded-transport.js';
import {RecordedBudget} from './recorded-budget.js';
export class RecordedMusicLoader {
 #sender;#wasm;#budget;#generation=0;#abort=null;#pending=new Set();#closed=false;
 constructor(sender,wasm,budget){
  if(!(sender instanceof RecordedMusicSender)||!(wasm instanceof Uint8Array)||wasm.length<8||wasm.length>4194304||!(budget instanceof RecordedBudget))throw Error('Invalid music loader');
  this.#sender=sender;this.#wasm=wasm;this.#budget=budget;
 }
 get loading(){return this.#pending.size!==0;}
 #advance(generation){if(this.#closed||!Number.isSafeInteger(generation)||generation<=this.#generation)return false;this.#generation=generation;this.#abort?.abort();this.#abort=null;this.#sender.retire();return true;}
 play(generation,encoded){
  if(!(encoded instanceof Uint8Array)||encoded.length<1||encoded.length>8388608||!this.#advance(generation))return false;
  const abort=new AbortController();this.#abort=abort;
  const task=(async()=>{
   try{
    const decoder=await RecordedDecoderClient.open(this.#wasm,encoded,6291456,abort.signal,this.#budget);
    if(this.#closed||abort.signal.aborted||generation!==this.#generation||!this.#sender.start(generation,decoder))decoder.close();
   }catch{
    if(!this.#closed&&!abort.signal.aborted&&generation===this.#generation)this.#sender.failGeneration(generation);
   }
  })();
  this.#pending.add(task);void task.finally(()=>this.#pending.delete(task));return true;
 }
 stop(generation){return this.#advance(generation)&&this.#sender.stop(generation);}
 async close(){if(!this.#closed){this.#closed=true;this.#abort?.abort();this.#sender.close();}await Promise.allSettled([...this.#pending]);}
}
