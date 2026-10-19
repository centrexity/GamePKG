// Trusted dedicated output-context owner; not a guest API.
import {originalEffectPCM} from './recorded-wav.js';
import {RecordedBudget} from './recorded-budget.js';
import {RecordedMusicSender} from './recorded-transport.js';
const STARTUP_MS=5000,REQUEST_MS=250,CREATE=Symbol();
let nextSession=0;
export class RecordedOutputClient {
 #context;#node;#gain;#pending=null;#serial=0;#session;#closed=false;#closing=null;#reservation;#music=null;#parent;
 constructor(token,context,reservation,parent){if(token!==CREATE)throw Error('Use RecordedOutputClient.open');this.#context=context;this.#reservation=reservation;this.#parent=parent;this.#session=++nextSession;}
 static async open(wasm,budget=4194304,parent,signal){
  if(!(wasm instanceof Uint8Array)||wasm.length<8||wasm.length>4194304||!Number.isInteger(budget)||budget<327680||budget>4194304||nextSession>=Number.MAX_SAFE_INTEGER)throw Error('Invalid output configuration');
  if(parent!==undefined&&!(parent instanceof RecordedBudget))throw Error('Invalid output memory allowance');
  if(signal!==undefined&&!(signal instanceof AbortSignal))throw Error('Invalid output cancellation signal');
  if(signal?.aborted)throw Error('Audio output startup cancelled');
  const reservation=parent?.reserve(budget+wasm.length*3);let client;
  try{client=new RecordedOutputClient(CREATE,new AudioContext({sampleRate:48000}),reservation,parent);}catch(error){reservation?.release();throw error;}
  let cancel;const cancelled=new Promise((_,reject)=>cancel=reject);const abort=()=>{cancel(Error('Audio output startup cancelled'));void client.close().catch(()=>{});};signal?.addEventListener('abort',abort,{once:true});let timer;
  try{
   await Promise.race([cancelled,client.#context.audioWorklet.addModule(new URL('./recorded-worklet.js',import.meta.url)),new Promise((_,reject)=>{timer=setTimeout(()=>reject(Error('Audio output startup deadline exceeded')),STARTUP_MS);})]);
   if(client.#closed)throw Error('Audio output startup cancelled');
   const bytes=wasm.slice();client.#node=new AudioWorkletNode(client.#context,'gamepkg-recorded-output',{numberOfInputs:0,numberOfOutputs:1,outputChannelCount:[2],processorOptions:{session:client.#session,wasm:bytes,budget}});
   client.#node.port.onmessage=({data})=>client.#reply(data);client.#node.port.onmessageerror=()=>client.#fail(Error('Invalid audio output message'));client.#node.onprocessorerror=()=>client.#fail(Error('Audio output processor failed'));
   client.#gain=client.#context.createGain();client.#node.connect(client.#gain);client.#gain.connect(client.#context.destination);
   await client.request('snapshot');return client;
  }catch(error){await client.close();throw error;}finally{clearTimeout(timer);signal?.removeEventListener('abort',abort);}
 }
 async connectMusic(sender,port){if(this.#closed||this.#music||!(sender instanceof RecordedMusicSender))return false;try{const reply=await this.request('music-port',{port},[port]);if(!reply.ok||this.#closed){sender.close();return false;}this.#music=sender;return true;}catch(error){sender.close();throw error;}}
 async loadWav(clip,encoded){
  if(this.#closed||!Number.isInteger(clip)||clip<0||clip>=32)throw Error('Invalid effect clip');
  const source=originalEffectPCM(encoded),reservation=this.#parent?.reserve(source.length);
  try{const pcm=source.slice();return await this.request('load',{clip,pcm},[pcm.buffer]);}
  finally{if(this.#closed&&this.#closing)await this.#closing;reservation?.release();}
 }
 get closed(){return this.#closed;}
 volume(value){if(this.#closed||!Number.isInteger(value)||value<0||value>128)return false;this.#gain.gain.value=value/128;return true;}
 resume(){if(this.#closed)return Promise.reject(Error('Audio output is closed'));return this.#context.resume();}
 #fail(error){const pending=this.#pending;this.#pending=null;if(pending){clearTimeout(pending.timer);pending.reject(error);}void this.close().catch(()=>{});}
 #reply(data){
  if(this.#closed)return;const pending=this.#pending;
  if(!pending||!data||data.session!==this.#session||data.id!==pending.id||typeof data.ok!=='boolean'||performance.now()>pending.expires){this.#fail(Error('Invalid or late audio output response'));return;}
  if(data.closed&&pending.kind!=='close'){this.#fail(Error('Audio output closed unexpectedly'));return;}
  if(!Number.isSafeInteger(data.generation)||data.generation<0||!Number.isSafeInteger(data.playedFrames)||data.playedFrames<0||!Number.isInteger(data.queuedFrames)||data.queuedFrames<0||data.queuedFrames>4800||!Number.isInteger(data.activeEffects)||data.activeEffects<0||data.activeEffects>4||typeof data.failed!=='boolean'||!['loading','playing','paused','ended','failed','stopped'].includes(data.musicStatus)){this.#fail(Error('Invalid audio output state'));return;}
  clearTimeout(pending.timer);this.#pending=null;pending.resolve(data);
 }
 request(kind,fields={},transfer=[]){
  if(this.#closed)return Promise.reject(Error('Audio output is closed'));
  if(this.#pending)return Promise.reject(Error('Audio output request already pending'));
  if(!['snapshot','load','unload','controls','music-port'].includes(kind))return Promise.reject(Error('Invalid audio output operation'));
  if(this.#serial>=Number.MAX_SAFE_INTEGER){this.#fail(Error('Audio request counter exhausted'));return Promise.reject(Error('Audio request counter exhausted'));}
  const id=++this.#serial;
  return new Promise((resolve,reject)=>{
   this.#pending={id,kind,resolve,reject,expires:performance.now()+REQUEST_MS,timer:setTimeout(()=>this.#fail(Error('Audio output deadline exceeded')),REQUEST_MS)};
   try{this.#node.port.postMessage({...fields,session:this.#session,id,kind},transfer);}catch(error){this.#fail(error);}
  });
 }
 close(){
  if(this.#closing)return this.#closing;this.#closed=true;
  const pending=this.#pending;this.#pending=null;if(pending){clearTimeout(pending.timer);pending.reject(Error('Audio output closed'));}
  this.#music?.close();this.#music=null;this.#node?.disconnect();this.#node?.port.close();this.#gain?.disconnect();
  this.#closing=Promise.resolve().then(()=>this.#context.close()).then(()=>{this.#reservation?.release();this.#reservation=null;});return this.#closing;
 }
}
