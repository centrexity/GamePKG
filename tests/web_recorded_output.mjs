import assert from 'node:assert/strict';
import fs from 'node:fs';
const url=code=>'data:text/javascript;base64,'+Buffer.from(code).toString('base64');
const read=name=>fs.readFileSync(new URL('../web-player/'+name,import.meta.url),'utf8');
const budgetURL=url(read('recorded-budget.js'));
const {RecordedBudget}=await import(budgetURL);
let now=0,serial=0,timers=new Map(),moduleDelay=0,replyDelay=0,closeDelay=0,lastPort,closed=0;
const schedule=(fn,delay)=>{const id=++serial;timers.set(id,{at:now+delay,fn});return id;};
const flush=async()=>{for(let i=0;i<30;i++)await Promise.resolve();};
async function advance(ms){const end=now+ms;await flush();while(true){const next=[...timers].filter(([,t])=>t.at<=end).sort((a,b)=>a[1].at-b[1].at||a[0]-b[0])[0];if(!next)break;timers.delete(next[0]);now=next[1].at;next[1].fn();await flush();}now=end;await flush();}
class Context {
 constructor(){this.destination={};this.audioWorklet={addModule:()=>new Promise(resolve=>schedule(resolve,moduleDelay))};}
 createGain(){return {gain:{value:1},connect(){},disconnect(){}};}
 close(){closed++;return new Promise(resolve=>schedule(resolve,closeDelay));}
 resume(){return Promise.resolve();}
}
class Node {
 constructor(){lastPort=this.port={close(){},postMessage(data){schedule(()=>this.onmessage?.({data:{session:data.session,id:data.id,ok:true,generation:0,playedFrames:0,queuedFrames:0,activeEffects:0,failed:false,musicStatus:'stopped'}}),replyDelay);}};}
 connect(){} disconnect(){}
}
globalThis.__outputTest={AudioContext:Context,AudioWorkletNode:Node,performance:{now:()=>now},setTimeout:schedule,clearTimeout:id=>timers.delete(id)};
let source=read('recorded-output-client.js').replace("'./recorded-wav.js'",JSON.stringify(url(read('recorded-wav.js')))).replace("'./recorded-budget.js'",JSON.stringify(budgetURL)).replace("'./recorded-transport.js'",JSON.stringify(url('export class RecordedMusicSender {}'))).replaceAll('import.meta.url',JSON.stringify(new URL('../web-player/recorded-output-client.js',import.meta.url).href));
source='const {AudioContext,AudioWorkletNode,performance,setTimeout,clearTimeout}=globalThis.__outputTest;\n'+source;
const {RecordedOutputClient}=await import(url(source));delete globalThis.__outputTest;
const wasm=new Uint8Array([0,97,115,109,1,0,0,0]);
function start(){const budget=new RecordedBudget(400000);let value,error;const promise=RecordedOutputClient.open(wasm,327680,budget).then(v=>value=v,e=>error=e);return {budget,promise,get value(){return value;},get error(){return error;}};}
function reset(){assert.equal(timers.size,0,'fixture left pending timers');moduleDelay=replyDelay=closeDelay=0;}
// Processor construction/compilation belongs to startup, not steady-state control.
{
 reset();moduleDelay=100;replyDelay=400;const f=start();await advance(499);assert.equal(f.value,undefined);assert.equal(f.error,undefined);await advance(1);await f.promise;assert.ok(f.value,f.error?.message);assert.equal(f.budget.used,327704);const closing=f.value.close();await advance(0);await closing;
}
// The same five-second startup budget covers BOTH module loading and first ack.
{
 reset();moduleDelay=4900;replyDelay=200;const f=start();await advance(5000);await f.promise;assert.match(f.error?.message??'',/deadline/);assert.equal(f.budget.used,0);await advance(100);
}
// A stalled module cannot consume more than the same startup allowance.
{
 reset();moduleDelay=5100;const f=start();await advance(5000);await f.promise;assert.match(f.error.message,/startup deadline/);assert.equal(f.budget.used,0);await advance(100);
}
// An ordinary request still has exactly 250 ms, even if a caller supplies fields.
{
 reset();const f=start();await advance(0);await f.promise;assert.ok(f.value);replyDelay=300;let error;const request=f.value.request('snapshot',{timeout:5000}).catch(e=>error=e);await advance(250);await request;assert.match(error.message,/deadline/);assert.equal(f.value.closed,true);assert.equal(f.budget.used,0);await advance(50);
}
// Cancellation while awaiting initial readiness closes the context before release.
{
 reset();replyDelay=400;closeDelay=100;const budget=new RecordedBudget(400000),abort=new AbortController();let error;const promise=RecordedOutputClient.open(wasm,327680,budget,abort.signal).catch(e=>error=e);await advance(100);abort.abort();await advance(0);assert.ok(budget.used>0);await advance(100);await promise;assert.match(error.message,/cancelled|closed/);assert.equal(budget.used,0);await advance(200);
}
// A malformed first reply fails closed and releases the reservation.
{
 reset();replyDelay=100;const f=start();await advance(0);lastPort.onmessage({data:{session:-1,id:1,ok:true}});await advance(0);await f.promise;assert.match(f.error.message,/Invalid/);assert.equal(f.budget.used,0);await advance(100);
}
assert.ok(closed>=5);reset();console.log('Audio output startup uses one five-second budget; steady requests retain 250 ms, cancellation and malformed replies close before memory release.');
