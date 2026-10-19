// Trusted source PCM FIFO. Fixed backing storage; no incoming packet is retained.
export const QUEUE_FRAMES=4800,BLOCK_FRAMES=800;
export class RecordedQueue {
 #data=new Int16Array(QUEUE_FRAMES*2);#head=0;#count=0;#generation=0;#played=0;#active=false;#started=false;#eof=false;#failed=false;#paused=false;#volume=128;#closed=false;
 get generation(){return this.#generation;}
 get queuedFrames(){return this.#count;}
 get availableFrames(){return this.#closed?0:QUEUE_FRAMES-this.#count;}
 get playedFrames(){return this.#played;}
 get storageBytes(){return this.#data?.byteLength||0;}
 get status(){if(this.#closed||!this.#active)return 'stopped';if(this.#failed)return 'failed';if(this.#eof&&!this.#count)return 'ended';if(this.#paused)return 'paused';return this.#started?'playing':'loading';}
 #reset(generation,active){
  if(this.#closed||!Number.isSafeInteger(generation)||generation<=this.#generation)return false;
  this.#generation=generation;this.#active=active;this.#head=0;this.#count=0;this.#played=0;this.#started=false;this.#eof=false;this.#failed=false;this.#paused=false;return true;
 }
 begin(generation){return this.#reset(generation,true);}
 stop(generation){return this.#reset(generation,false);}
 volume(value){if(this.#closed||!Number.isInteger(value)||value<0||value>128)return false;this.#volume=value;return true;}
 pause(value){if(this.#closed||typeof value!=='boolean'||!['loading','playing','paused'].includes(this.status))return false;this.#paused=value;return true;}
 push(generation,samples){
  if(this.#closed||!this.#active||this.#failed||this.#eof||generation!==this.#generation||!(samples instanceof Int16Array)||!(samples.buffer instanceof ArrayBuffer)||samples.byteOffset!==0||samples.buffer.byteLength!==samples.byteLength||samples.length<2||samples.length>BLOCK_FRAMES*2||samples.length%2||samples.length/2>QUEUE_FRAMES-this.#count)return false;
  const start=(this.#head+this.#count*2)%this.#data.length;for(let i=0;i<samples.length;i++)this.#data[(start+i)%this.#data.length]=samples[i];this.#count+=samples.length/2;this.#started=true;return true;
 }
 finish(generation){if(this.#closed||!this.#active||this.#failed||this.#eof||generation!==this.#generation)return false;this.#eof=true;return true;}
 fail(generation){if(this.#closed||!this.#active||this.#failed||(this.#eof&&!this.#count)||generation!==this.#generation)return false;this.#failed=true;this.#count=0;this.#head=0;return true;}
 read(output){
  if(!(output instanceof Int16Array)||output.length>BLOCK_FRAMES*2||output.length%2)throw Error('Invalid queued PCM read');
  if(this.#closed||!this.#active||this.#failed||this.#paused)return 0;
  const count=Math.min(this.#count,output.length/2);if(this.#played>Number.MAX_SAFE_INTEGER-count)throw Error('Music frame counter exhausted');
  for(let i=0;i<count*2;i++)output[i]=Math.trunc(this.#data[(this.#head+i)%this.#data.length]*this.#volume/128);
  this.#head=(this.#head+count*2)%this.#data.length;this.#count-=count;this.#played+=count;return count;
 }
 close(){this.#closed=true;this.#active=false;this.#count=0;this.#played=0;this.#data=null;}
}
