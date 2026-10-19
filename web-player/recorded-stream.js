// Trusted live output-clock adapter. Missing music is silence; effects continue.
import {RecordedMixer,MAX_EFFECT_MEMORY} from './recorded-mixer.js';
import {RecordedQueue} from './recorded-queue.js';
export class RecordedStream {
 #mixer;#queue;#input;#output;#closed=false;#failed=false;#rendered=0;
 constructor(wasm,budget=MAX_EFFECT_MEMORY){this.#mixer=new RecordedMixer(wasm,budget);this.#queue=new RecordedQueue();this.#input=new Int16Array(128);this.#output=new Int16Array(1600);}
 get generation(){return this.#queue.generation;}
 get queuedFrames(){return this.#queue.queuedFrames;}
 get availableFrames(){return this.#queue.availableFrames;}
 get playedFrames(){return this.#queue.playedFrames;}
 get renderedFrames(){return this.#rendered;}
 get musicStatus(){return this.#queue.status;}
 get activeEffects(){return this.#mixer.active;}
 get pcmBytes(){return this.#mixer.pcmBytes;}
 get fixedBufferBytes(){return this.#queue.storageBytes+(this.#input?.byteLength||0)+(this.#output?.byteLength||0);}
 get reservedBytes(){return this.#mixer.reservedBytes;}
 get failed(){return this.#failed||this.#mixer.failed;}
 begin(generation){return !this.#closed&&this.#queue.begin(generation);}
 stopMusic(generation){return !this.#closed&&this.#queue.stop(generation);}
 push(generation,samples){return !this.#closed&&this.#queue.push(generation,samples);}
 finishMusic(generation){return !this.#closed&&this.#queue.finish(generation);}
 failMusic(generation){return !this.#closed&&this.#queue.fail(generation);}
 musicVolume(value){return !this.#closed&&this.#queue.volume(value);}
 musicPause(value){return !this.#closed&&this.#queue.pause(value);}
 load(id,pcm){return !this.#closed&&this.#mixer.load(id,pcm);}
 unload(id){return !this.#closed&&this.#mixer.unload(id);}
 play(id){return this.#closed?-1:this.#mixer.play(id);}
 effectVolume(value){return !this.#closed&&this.#mixer.volume(value);}
 effectPause(value){return !this.#closed&&this.#mixer.pause(value);}
 stopEffects(){if(!this.#closed)this.#mixer.stop();}
 render(output){
  if(!(output instanceof Int16Array)||output.length>1600||output.length%2)throw Error('Invalid live audio output');
  if(this.#closed)return 0;
  if(this.failed)throw Error('Live audio is not active');
  const frames=output.length/2;if(this.#rendered>Number.MAX_SAFE_INTEGER-frames)throw Error('Output frame counter exhausted');
  try{
   let at=0;
   for(let iteration=0;iteration<32&&at<frames;iteration++){
    const count=this.#mixer.render(this.#output.subarray(at*2,frames*2));if(count){at+=count;continue;}
    const refill=Math.min(64,this.#mixer.inputCapacity);if(!refill)throw Error('Live converter stalled');
    const input=this.#input.subarray(0,refill*2);input.fill(0);this.#queue.read(input);this.#mixer.append(input);
   }
   if(at!==frames)throw Error('Live conversion refill limit exceeded');
   output.set(this.#output.subarray(0,frames*2));this.#rendered+=frames;return frames;
  }catch(error){this.#failed=true;throw error;}
 }
 close(){if(this.#closed)return;this.#closed=true;this.#queue.close();this.#mixer.close();this.#input=null;this.#output=null;}
}
