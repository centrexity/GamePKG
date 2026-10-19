// Trusted output endpoint. Guest code never receives this port or its objects.
import {RecordedStream} from './recorded-stream.js';
import {RecordedMusicReceiver} from './recorded-transport.js';
class RecordedOutput extends AudioWorkletProcessor {
 #stream;#music=null;#session;#serial=0;#closed=false;#prepared=null;#frameEpoch=0;#frameSerial=0;#output=new Int16Array(1600);
 constructor(options){
  super();const config=options.processorOptions;
  if(!Number.isSafeInteger(config?.session)||config.session<1)throw Error('Invalid audio session');
  this.#session=config.session;this.#stream=new RecordedStream(config.wasm,config.budget);
  this.port.onmessage=({data})=>this.#receive(data);
  this.port.onmessageerror=()=>this.#close();
 }
 #snapshot(){return {generation:this.#stream.generation,musicStatus:this.#stream.musicStatus,playedFrames:this.#stream.playedFrames,queuedFrames:this.#stream.queuedFrames,activeEffects:this.#stream.activeEffects,failed:this.#stream.failed};}
 #close(){if(this.#closed)return;this.#closed=true;this.#prepared=null;this.#music?.close();this.#stream.close();this.#output=null;}
 #prepare(data){
  if(this.#prepared||!Number.isInteger(data.epoch)||data.epoch<0||data.epoch>4294967295||!Number.isInteger(data.frame)||data.frame<1||data.frame>4294967295||data.epoch<this.#frameEpoch||(data.epoch===this.#frameEpoch&&data.frame<=this.#frameSerial))return false;
  this.#frameEpoch=data.epoch;this.#frameSerial=data.frame;
  if(!this.#stream.validateControls(data.commands,data.activeGeneration??null))return false;
  this.#prepared={epoch:data.epoch,frame:data.frame,activeGeneration:data.activeGeneration??null,commands:data.commands.map(command=>command.slice())};return true;
 }
 #finish(data){
  const frame=this.#prepared;if(!frame||data.epoch!==frame.epoch||data.frame!==frame.frame||typeof data.success!=='boolean')return false;
  this.#prepared=null;return !data.success||this.#stream.applyControls(frame.commands,frame.activeGeneration);
 }
 #receive(data){
  if(this.#closed||!data||data.session!==this.#session)return;
  if(!Number.isSafeInteger(data.id)||data.id<=this.#serial)return;
  this.#serial=data.id;let ok=false;
  try{
   if(data.kind==='music-port'&&!this.#music&&data.port instanceof MessagePort){this.#music=new RecordedMusicReceiver(data.port,this.#stream);ok=true;}
   else if(data.kind==='prepare')ok=this.#prepare(data);
   else if(data.kind==='finish')ok=this.#finish(data);
   else if(data.kind==='reserve-clip'&&!this.#prepared)ok=this.#stream.reserveClip(data.clip,data.length,data.id);
   else if(data.kind==='admit-clip'&&!this.#prepared)ok=this.#stream.adoptClip(data.clip,data.pcm,data.token);
   else if(data.kind==='cancel-clip'&&!this.#prepared)ok=this.#stream.cancelClip(data.clip,data.token);
   else if(data.kind==='load'&&!this.#prepared)ok=this.#stream.load(data.clip,data.pcm);
   else if(data.kind==='unload'&&!this.#prepared)ok=this.#stream.unload(data.clip);
   else if(data.kind==='controls'&&!this.#prepared)ok=this.#stream.applyControls(data.commands,data.activeGeneration??null);
   else if(data.kind==='snapshot')ok=true;
   else if(data.kind==='close'){this.#close();this.port.postMessage({session:this.#session,id:data.id,ok:true,closed:true});return;}
   this.port.postMessage({session:this.#session,id:data.id,ok,...this.#snapshot()});
  }catch{
   this.#close();this.port.postMessage({session:this.#session,id:data.id,ok:false,closed:true});
  }
 }
 process(_,outputs){
  if(this.#closed)return false;
  try{
   const channels=outputs[0],frames=channels?.[0]?.length;
   if(channels?.length!==2||!Number.isInteger(frames)||frames<1||frames>800||channels[1].length!==frames)throw Error('Invalid audio quantum');
   const output=this.#output.subarray(0,frames*2);this.#stream.render(output);
   for(let i=0;i<frames;i++){channels[0][i]=output[i*2]/32768;channels[1][i]=output[i*2+1]/32768;}
   this.#music?.poll();return true;
  }catch{
   for(const channel of outputs[0]||[])channel.fill(0);this.#close();this.port.postMessage({session:this.#session,failed:true,closed:true});return false;
  }
 }
}
registerProcessor('gamepkg-recorded-output',RecordedOutput);
