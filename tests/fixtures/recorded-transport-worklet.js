import {RecordedStream} from './recorded-stream.js';
import {RecordedMusicReceiver} from './recorded-transport.js';
class TransportAudio extends AudioWorkletProcessor {
 constructor(options){
  super();this.stream=new RecordedStream(options.processorOptions.wasm);this.frames=0;this.started=false;this.output=new Int16Array(256);this.total=options.processorOptions.frames||8192;this.capture=new Int16Array(this.total*2);
  this.port.onmessage=({data})=>{if(data.port&&!this.receiver)this.receiver=new RecordedMusicReceiver(data.port,this.stream);};
  this.stream.effectVolume(options.processorOptions.volume);
  for(let slot=0;slot<4;slot++){const frames=[1025,2051,4096,777][slot],bytes=new Uint8Array(frames*4),view=new DataView(bytes.buffer);for(let i=0;i<frames;i++)for(let c=0;c<2;c++)view.setInt16(i*4+c*2,(i*(slot*1543+197)+c*12345+slot*313)%65536-32768,true);if(!this.stream.load(slot,bytes)||this.stream.play(slot)!==slot)throw Error('clip rejected');}
 }
 process(_,outputs){
  try{
   if(!this.started){if(this.stream.queuedFrames<4800)return true;this.started=true;}
   const frames=Math.min(128,this.total-this.frames),out=this.output.subarray(0,frames*2);this.stream.render(out);this.capture.set(out,this.frames*2);
   for(let i=0;i<frames;i++)for(let c=0;c<2;c++)outputs[0][c][i]=out[i*2+c]/32768;
   this.frames+=frames;this.receiver.poll();
   if(this.frames===this.total){const values=this.capture;this.port.postMessage({values,played:this.stream.playedFrames,status:this.stream.musicStatus},[values.buffer]);this.receiver.close();this.stream.close();return false;}
   return true;
  }catch(error){this.receiver?.close();this.stream.close();this.port.postMessage({error:String(error.stack||error)});return false;}
 }
}
registerProcessor('transport-audio',TransportAudio);
