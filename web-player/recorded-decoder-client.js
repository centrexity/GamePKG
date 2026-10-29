// Trusted worker supervisor. Decoder execution never occupies the game/UI thread.
import {MAX_DECODER_MEMORY,MAX_ENCODED_BYTES,MAX_PCM_VALUES} from './recorded-decoder.js';
import {RecordedBudget} from './recorded-budget.js';
const STARTUP_MS=5000,EXECUTION_MS=100,CREATE=Symbol();
function owned(bytes,maximum){const size=bytes?.byteLength;if(!(bytes instanceof Uint8Array)||!Number.isInteger(size)||size<1||size>maximum)throw Error('Invalid decoder byte snapshot');const copy=new Uint8Array(size);copy.set(new Uint8Array(bytes.buffer,bytes.byteOffset,size));return copy;}
export class RecordedDecoderClient {
 #worker;#pending=null;#serial=0;#closed=false;#ready;#reservation=null;
 constructor(token){
  if(token!==CREATE)throw Error('Use RecordedDecoderClient.open');
  this.#worker=new Worker(new URL('./recorded-decoder-worker.js',import.meta.url),{type:'module'});
  this.#ready=new Promise((resolve,reject)=>{this.#pending={kind:'ready',resolve,reject,expires:performance.now()+STARTUP_MS,timer:setTimeout(()=>this.#fail(Error('Decoder startup deadline exceeded')),STARTUP_MS)};});
  this.#worker.onmessage=({data})=>this.#reply(data);
  this.#worker.onerror=event=>{event.preventDefault();this.#fail(Error('Decoder worker failed'));};
  this.#worker.onmessageerror=()=>this.#fail(Error('Invalid decoder worker message'));
 }
 static async open(wasm,encoded,limit=MAX_DECODER_MEMORY,signal,budget){
  if(!(wasm instanceof Uint8Array)||wasm.length<8||wasm.length>4194304||!(encoded instanceof Uint8Array)||encoded.length<1||encoded.length>MAX_ENCODED_BYTES||!Number.isInteger(limit)||limit<65536||limit>MAX_DECODER_MEMORY)throw Error('Invalid decoder input');
  if(signal!==undefined&&!(signal instanceof AbortSignal))throw Error('Invalid decoder cancellation signal');
  if(signal?.aborted)throw Error('Decoder open cancelled');
  if(budget!==undefined&&!(budget instanceof RecordedBudget))throw Error('Invalid audio memory allowance');
  // Linear memory, two module snapshots, one 64 KiB transfer and three PCM blocks.
  // Caller-owned source buffers and browser compiler/JIT overhead are separate.
  const reservation=budget?.reserve(limit+wasm.byteLength*2+65536+MAX_PCM_VALUES*2*3);
  let client;try{client=new RecordedDecoderClient(CREATE);client.#reservation=reservation;}catch(error){reservation?.release();throw error;}
  const abort=()=>client.close();signal?.addEventListener('abort',abort,{once:true});
  try{
   await client.#ready;const module=owned(wasm,4194304);await client.#request('compile',{bytes:module,limit},[module.buffer]);
   // Upload is trusted bounded copying plus inter-worker scheduling, not codec
   // execution. Bound the whole upload and every individual acknowledgement;
   // codec open/read/rewind retain their independent 100 ms execution deadline.
   const expires=performance.now()+STARTUP_MS;
   await client.#request('reserve',{length:encoded.length},[],Math.min(expires,performance.now()+EXECUTION_MS));
   for(let at=0;at<encoded.length;at+=65536){const bytes=owned(encoded.subarray(at,Math.min(at+65536,encoded.length)),65536);await client.#request('input',{offset:at,bytes},[bytes.buffer],Math.min(expires,performance.now()+EXECUTION_MS));}
   await client.#request('open');return client;
  }
  catch(error){client.close();throw error;}finally{signal?.removeEventListener('abort',abort);}
 }
 get closed(){return this.#closed;}
 #fail(error){if(this.#closed)return;this.#closed=true;this.#worker.terminate();this.#reservation?.release();this.#reservation=null;const pending=this.#pending;this.#pending=null;if(pending){clearTimeout(pending.timer);pending.reject(error);}}
 #reply(data){
  if(this.#closed)return;
  const pending=this.#pending;
  if(!pending||!data||typeof data!=='object'||Array.isArray(data)){this.#fail(Error('Unexpected decoder response'));return;}
  if(performance.now()>pending.expires){this.#fail(Error('Decoder execution deadline exceeded'));return;}
  if(pending.kind==='ready'){
   if(data.ready!==true||Object.keys(data).length!==1){this.#fail(Error('Invalid decoder startup'));return;}
  }else{
   if(data.id!==pending.id||typeof data.ok!=='boolean'){this.#fail(Error('Invalid decoder response identity'));return;}
   if(!data.ok){this.#fail(Error(typeof data.error==='string'?data.error.slice(0,512):'Decoder failed'));return;}
   if(pending.kind==='compile'&&(!Number.isInteger(data.maximumMemoryBytes)||data.maximumMemoryBytes<65536||data.maximumMemoryBytes>pending.limit)){this.#fail(Error('Invalid decoder memory response'));return;}
   if(pending.kind==='read'&&(!(data.values instanceof Int16Array)||!(data.values.buffer instanceof ArrayBuffer)||data.values.length>pending.length||data.values.length%2||data.values.byteOffset!==0||data.values.buffer.byteLength!==data.values.length*2)){this.#fail(Error('Invalid decoder PCM response'));return;}
  }
  clearTimeout(pending.timer);this.#pending=null;pending.resolve(pending.kind==='read'?data.values:undefined);
 }
 #request(kind,fields={},transfer=[],deadline){
  if(this.#closed)return Promise.reject(Error('Decoder is closed'));
  if(this.#pending)return Promise.reject(Error('Decoder request already pending'));
  if(this.#serial>=Number.MAX_SAFE_INTEGER){this.close();return Promise.reject(Error('Decoder request counter exhausted'));}
  const id=++this.#serial,expires=deadline??performance.now()+(kind==='compile'?STARTUP_MS:EXECUTION_MS),timeout=Math.max(0,expires-performance.now());
  if(!timeout){this.#fail(Error('Decoder execution deadline exceeded'));return Promise.reject(Error('Decoder execution deadline exceeded'));}
  return new Promise((resolve,reject)=>{
   this.#pending={id,kind,length:fields.length,limit:fields.limit,resolve,reject,expires,timer:setTimeout(()=>this.#fail(Error('Decoder execution deadline exceeded')),timeout)};
   try{this.#worker.postMessage({id,kind,...fields},transfer);}catch(error){this.#fail(error);}
  });
 }
 read(length=MAX_PCM_VALUES){if(!Number.isInteger(length)||length<2||length>MAX_PCM_VALUES||length%2)return Promise.reject(Error('Invalid PCM read size'));return this.#request('read',{length});}
 rewind(){return this.#request('rewind');}
 close(){this.#fail(Error('Decoder closed'));}
}
