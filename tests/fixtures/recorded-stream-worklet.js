import {RecordedStream} from './recorded-stream.js';
class ReferenceLiveStream extends AudioWorkletProcessor {
 constructor(options){
  super();this.input=0;this.frames=0;this.finished=false;this.failed=false;this.output=new Int16Array(1600);this.mode=options.processorOptions.mode||'';this.halted=false;
  try{
   this.stream=new RecordedStream(options.processorOptions.wasm);this.stream.begin(1);this.stream.push(1,new Int16Array([1234,-5678]));this.stream.begin(2);
   if(this.stream.push(1,new Int16Array([9999,-9999])))throw Error('stale generation accepted');
   this.stream.musicVolume(options.processorOptions.music_volume);this.stream.effectVolume(options.processorOptions.effect_volume);
   for(let slot=0;slot<4;slot++){const frames=[1025,2051,4096,777][slot],bytes=new Uint8Array(frames*4),view=new DataView(bytes.buffer);for(let i=0;i<frames;i++)for(let c=0;c<2;c++)view.setInt16(i*4+c*2,(i*(slot*1543+197)+c*12345+slot*313)%65536-32768,true);if(!this.stream.load(slot,bytes)||this.stream.play(slot)!==slot)throw Error('clip rejected');}
   this.feed();if(this.mode==='pause')this.stream.musicPause(true);if(this.mode==='pause-effects')this.stream.effectPause(true);if(this.mode==='fail')this.stream.failMusic(2);if(this.mode==='stop')this.stream.stopMusic(3);this.halted=['fail','stop'].includes(this.mode);
  }catch(error){this.failed=true;this.port.postMessage({error:String(error.stack||error)});}
 }
 feed(){
  if(this.halted||this.mode==='underrun')return;
  while(this.input<6000&&this.stream.availableFrames>=800){const count=Math.min(800,6000-this.input),pcm=new Int16Array(count*2);for(let i=0;i<count;i++)for(let c=0;c<2;c++)pcm[i*2+c]=((this.input+i)*7919+c*31337)%65536-32768;if(!this.stream.push(2,pcm))throw Error('FIFO rejected available data');this.input+=count;}
  if(this.input===6000&&!this.finished){if(!this.stream.finishMusic(2))throw Error('EOF rejected');this.finished=true;}
 }
 process(_,outputs){
  if(this.failed)return false;
  try{
   this.feed();const output=outputs[0],frames=Math.min(output[0].length,8192-this.frames),count=this.stream.render(this.output.subarray(0,frames*2));if(count!==frames)throw Error('short live render');
   for(let i=0;i<count;i++)for(let c=0;c<2;c++)output[c][i]=this.output[i*2+c]/32768;this.frames+=count;
   if(this.frames===8192){const report={frames:this.frames,played:this.stream.playedFrames,queued:this.stream.queuedFrames,status:this.stream.musicStatus,active:this.stream.activeEffects,buffers:this.stream.fixedBufferBytes};this.stream.close();if(this.stream.fixedBufferBytes||this.stream.pcmBytes)throw Error('owned buffers retained after close');this.port.postMessage(report);return false;}
   return true;
  }catch(error){this.failed=true;this.stream?.close();this.port.postMessage({error:String(error.stack||error)});return false;}
 }
}
registerProcessor('reference-live-stream',ReferenceLiveStream);
