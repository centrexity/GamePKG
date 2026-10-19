import {RecordedMixer} from './recorded-mixer.js';
class ReferenceMixer extends AudioWorkletProcessor {
 constructor(options){
  super();this.at=0;this.frames=0;this.input=new Int16Array(1600);this.output=new Int16Array(1600);this.failed=false;
  try{
   this.mixer=new RecordedMixer(options.processorOptions.wasm);this.mixer.volume(options.processorOptions.volume);if(options.processorOptions.paused)this.mixer.pause(true);
   for(let slot=0;slot<4;slot++){const frames=[1025,2051,4096,777][slot],bytes=new Uint8Array(frames*4),view=new DataView(bytes.buffer);for(let i=0;i<frames;i++)for(let c=0;c<2;c++)view.setInt16(i*4+c*2,(i*(slot*1543+197)+c*12345+slot*313)%65536-32768,true);if(!this.mixer.load(slot,bytes)||this.mixer.play(slot)!==slot)throw Error('fixture clip rejected');}
  }catch(error){this.failed=true;this.port.postMessage({error:String(error.stack||error)});}
 }
 process(_,outputs){
  if(this.failed)return false;
  try{
   const output=outputs[0];let at=0;
   while(at<output[0].length){
    const count=this.mixer.render(this.output.subarray(0,(output[0].length-at)*2));
    if(count){for(let i=0;i<count;i++)for(let c=0;c<2;c++)output[c][at+i]=this.output[i*2+c]/32768;at+=count;this.frames+=count;continue;}
    if(this.at===6000){this.mixer.close();this.port.postMessage({frames:this.frames});return false;}
    const n=Math.min(this.mixer.inputCapacity,800,6000-this.at);if(!n)throw Error('converter stalled');
    for(let i=0;i<n;i++)for(let c=0;c<2;c++)this.input[i*2+c]=((this.at+i)*7919+c*31337)%65536-32768;
    this.mixer.append(this.input.subarray(0,n*2));this.at+=n;if(this.at===6000)this.mixer.finish();
   }
   return true;
  }catch(error){this.failed=true;this.mixer?.close();this.port.postMessage({error:String(error.stack||error)});return false;}
 }
}
registerProcessor('reference-mixer',ReferenceMixer);
