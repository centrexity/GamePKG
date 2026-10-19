// Trusted original-rate music/effect mix followed by one shared conversion.
import {instantiateCompanion} from './recorded-decoder.js';
export const MAX_CONVERTER_MEMORY=262144,MAX_EFFECT_MEMORY=4194304;
const HOST_BYTES=65536,MAX_CLIP_BYTES=2097152,MAX_VALUES=1600;
const ABI={converter_init:0,converter_input:0,converter_output:0,converter_capacity:0,converter_append:1,converter_render:1,converter_finish:0};
export function mixSample(destination,source,volume){return Math.max(-32768,Math.min(32767,destination+Math.trunc(source*Math.min(volume,128)/128)));}
export class RecordedMixer {
 #api;#maximum;#budget;#bytes=0;#clips=Array(32).fill(null);#voices=Array(4).fill(null);#volume=128;#paused=false;#finished=false;#closed=false;#failed=false;
 constructor(wasm,budget=MAX_EFFECT_MEMORY){
  if(!Number.isInteger(budget)||budget<MAX_CONVERTER_MEMORY+HOST_BYTES||budget>MAX_EFFECT_MEMORY)throw Error('Invalid recorded mix budget');
  const {api,maximumMemoryBytes}=instantiateCompanion(wasm,MAX_CONVERTER_MEMORY,ABI);this.#api=api;this.#maximum=maximumMemoryBytes;this.#budget=budget;
  if(api.converter_init()!==0)throw Error('Converter initialization failed');this.#memory();
 }
 get active(){return this.#voices.filter(Boolean).length;}
 get pcmBytes(){return this.#bytes;}
 get reservedBytes(){return this.#maximum+HOST_BYTES;}
 get failed(){return this.#failed;}
 #ready(){if(this.#closed||this.#failed)throw Error('Recorded mixer is not active');}
 #memory(){const memory=this.#api?.memory;if(!(memory instanceof WebAssembly.Memory)||memory.buffer.byteLength>this.#maximum)throw Error('Converter memory limit exceeded');return memory.buffer;}
 #range(pointer,bytes){const memory=this.#memory();if(!Number.isInteger(pointer)||pointer<=0||pointer+bytes>memory.byteLength)throw Error('Invalid converter buffer');return new DataView(memory,pointer,bytes);}
 #call(operation){try{return operation();}catch(error){this.#failed=true;throw error;}}
 load(id,pcm){
  this.#ready();const length=pcm?.byteLength;
  if(!Number.isInteger(id)||id<0||id>=32||this.#clips[id]||!(pcm instanceof Uint8Array)||!Number.isInteger(length)||length<4||length>MAX_CLIP_BYTES||length%4||length>this.#budget-this.reservedBytes-this.#bytes)return false;
  const owned=new Uint8Array(length);owned.set(new Uint8Array(pcm.buffer,pcm.byteOffset,length));this.#clips[id]=new DataView(owned.buffer);this.#bytes+=length;return true;
 }
 unload(id){this.#ready();if(!Number.isInteger(id)||id<0||id>=32||!this.#clips[id]||this.#voices.some(voice=>voice?.id===id))return false;this.#bytes-=this.#clips[id].byteLength;this.#clips[id]=null;return true;}
 play(id){this.#ready();if(!Number.isInteger(id)||id<0||id>=32||!this.#clips[id])return -1;const slot=this.#voices.indexOf(null);if(slot<0)return -1;this.#voices[slot]={id,offset:0};return slot;}
 volume(value){this.#ready();if(!Number.isInteger(value)||value<0||value>128)return false;this.#volume=value;return true;}
 pause(value){this.#ready();if(typeof value!=='boolean')return false;this.#paused=value;return true;}
 stop(){this.#ready();this.#voices.fill(null);}
 get inputCapacity(){this.#ready();return this.#call(()=>{const count=this.#api.converter_capacity();if(!Number.isInteger(count)||count<0||count>800)throw Error('Invalid converter capacity');return count;});}
 append(music){
  this.#ready();if(this.#finished||!(music instanceof Int16Array)||music.length>MAX_VALUES||music.length%2)throw Error('Invalid mixer input');
  const frames=music.length/2;if(frames>this.inputCapacity)throw Error('Converter input is full');
  this.#call(()=>{const target=this.#range(this.#api.converter_input(),music.length*2);
   for(let frame=0;frame<frames;frame++){
    let left=music[frame*2],right=music[frame*2+1];
    if(!this.#paused)for(let slot=0;slot<4;slot++){const voice=this.#voices[slot];if(!voice)continue;const clip=this.#clips[voice.id];left=mixSample(left,clip.getInt16(voice.offset,true),this.#volume);right=mixSample(right,clip.getInt16(voice.offset+2,true),this.#volume);voice.offset+=4;if(voice.offset===clip.byteLength)this.#voices[slot]=null;}
    target.setInt16(frame*4,left,true);target.setInt16(frame*4+2,right,true);
   }
   if(this.#api.converter_append(frames)!==0)throw Error('Converter rejected mixed input');
  });
 }
 render(output){
  this.#ready();if(!(output instanceof Int16Array)||output.length>MAX_VALUES||output.length%2)throw Error('Invalid mixer output');
  return this.#call(()=>{const count=this.#api.converter_render(output.length/2);if(!Number.isInteger(count)||count<0||count>output.length/2)throw Error('Invalid converted frame count');if(!count)return 0;const view=this.#range(this.#api.converter_output(),count*4);for(let i=0;i<count*2;i++)output[i]=view.getInt16(i*2,true);return count;});
 }
 finish(){this.#ready();this.#call(()=>{if(this.#api.converter_finish()!==0)throw Error('Converter finish failed');this.#finished=true;});}
 close(){this.#closed=true;this.#api=null;this.#clips.fill(null);this.#voices.fill(null);this.#bytes=0;}
}
