// Deterministic supervisor clock: message scheduling must not consume codec time.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';

async function check(chunkDelay, encodedLength, stalledKind) {
 let now=0,created=0,terminated=0,inputs=0,openCalls=0;
 const budget={used:0};
 class RecordedBudget {
  reserve(bytes){budget.used+=bytes;let released=false;return {release(){if(!released){released=true;budget.used-=bytes;}}};}
 }
 class Worker {
  constructor(){created++;queueMicrotask(()=>this.onmessage({data:{ready:true}}));}
  postMessage(data){
   if(data.kind==='input'){inputs++;now+=chunkDelay;}
   if(data.kind==='open')openCalls++;
   if(data.kind===stalledKind)now+=101;
   queueMicrotask(()=>this.onmessage({data:{id:data.id,ok:true,...(data.kind==='compile'?{maximumMemoryBytes:6291456}:{})}}));
  }
  terminate(){terminated++;}
 }
 const context=vm.createContext({Worker,RecordedBudget,Uint8Array,Int16Array,ArrayBuffer,AbortSignal,URL,Symbol,performance:{now:()=>now},setTimeout,clearTimeout,MAX_DECODER_MEMORY:6291456,MAX_ENCODED_BYTES:4194304,MAX_PCM_VALUES:1600});
 const source=fs.readFileSync(new URL('../web-player/recorded-decoder-client.js',import.meta.url),'utf8').replace(/^import .*$/gm,'').replace('export class RecordedDecoderClient','class RecordedDecoderClient').replaceAll('import.meta.url',JSON.stringify(import.meta.url));
 vm.runInContext(source+';globalThis.Client=RecordedDecoderClient;',context);
 let error=null;
 try{const client=await context.Client.open(new Uint8Array(8),new Uint8Array(encodedLength),6291456,undefined,new RecordedBudget());client.close();}catch(failure){error=failure;}
 assert.equal(created,terminated,'worker cleanup');assert.equal(budget.used,0,'memory cleanup');
 return {error,now,inputs,openCalls};
}
const delayed=await check(60,131072);
assert.equal(delayed.error,null,'two quick copies with 120 ms total scheduling delay must load');
assert.equal(delayed.inputs,2);assert.equal(delayed.openCalls,1);
const aggregate=await check(90,4194304);
assert.match(String(aggregate.error),/deadline/);assert.equal(aggregate.openCalls,0);
assert.ok(aggregate.now>=5000&&aggregate.inputs<64,'upload cannot renew its aggregate deadline');
for(const kind of ['reserve','input','open'])assert.match(String((await check(0,65536,kind)).error),/deadline/,kind+' retains 100 ms supervision');
console.log('Decoder upload: scheduling tolerance, fixed aggregate bound, per-operation execution limits and complete worker/memory cleanup passed.');
