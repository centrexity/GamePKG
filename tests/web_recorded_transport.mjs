import assert from 'node:assert/strict';
import {MessageChannel} from 'node:worker_threads';
import fs from 'node:fs';
const url=code=>'data:text/javascript;base64,'+Buffer.from(code).toString('base64');
const read=name=>fs.readFileSync(new URL('../web-player/'+name,import.meta.url),'utf8');
const queueURL=url(read('recorded-queue.js'));
const budgetURL=url(read('recorded-budget.js'));
const {RecordedBudget}=await import(budgetURL);
const {RecordedQueue}=await import(queueURL);
const {RecordedMusicReceiver,RecordedMusicSender}=await import(url(read('recorded-transport.js').replace("'./recorded-queue.js'",JSON.stringify(queueURL)).replace("'./recorded-budget.js'",JSON.stringify(budgetURL))));
const wait=async test=>{const end=performance.now()+2000;while(!test()){assert.ok(performance.now()<end,'transport stalled');await new Promise(resolve=>setTimeout(resolve,1));}};
function fixture(){const queue=new RecordedQueue(),stream={get generation(){return queue.generation;},get musicStatus(){return queue.status;},get availableFrames(){return queue.availableFrames;},begin:g=>queue.begin(g),stopMusic:g=>queue.stop(g),failMusic:g=>queue.fail(g),finishMusic:g=>queue.finish(g),push:(g,v)=>queue.push(g,v)};const {port1,port2}=new MessageChannel();return {queue,port1,port2,receiver:new RecordedMusicReceiver(port1,stream)};}
function decoder(total,value){return {reads:0,closed:false,async read(size){assert.equal(size,1600);this.reads++;const n=Math.min(total,size);total-=n;return new Int16Array(n).fill(value);},close(){this.closed=true;}};}
{
 const {queue,port2,receiver}=fixture(),sender=new RecordedMusicSender(port2),source=decoder(12000,1234);
 assert.ok(sender.start(1,source));await wait(()=>queue.queuedFrames===4800);assert.equal(source.reads,6);
 for(let i=0;i<10;i++)receiver.poll();await new Promise(resolve=>setTimeout(resolve,10));assert.equal(source.reads,6,'full queue must not decode ahead');
 const all=[];while(queue.status!=='ended'){const out=new Int16Array(1600),n=queue.read(out);all.push(...out.subarray(0,n*2));receiver.poll();await new Promise(resolve=>setTimeout(resolve,1));}
 assert.equal(all.length,12000);assert.ok(all.every(v=>v===1234));await wait(()=>source.closed);assert.equal(source.reads,9);sender.close();receiver.close();
}
{
 const {queue,port2,receiver}=fixture(),sender=new RecordedMusicSender(port2);let resolve;
 const old={closed:false,read(){return new Promise(r=>resolve=r);},close(){this.closed=true;}};
 sender.start(1,old);await wait(()=>resolve);const next=decoder(1600,-2345);sender.start(2,next);assert.ok(old.closed);
 resolve(new Int16Array(1600).fill(999));await wait(()=>queue.queuedFrames===800);const out=new Int16Array(1600);assert.equal(queue.read(out),800);assert.ok(out.every(v=>v===-2345));receiver.poll();await wait(()=>queue.status==='ended');
 assert.ok(sender.stop(3));await wait(()=>queue.generation===3);assert.equal(queue.status,'stopped');assert.equal(sender.start(2,decoder(2,1)),false);sender.close();receiver.close();
}
{
 const {queue,port2,receiver}=fixture(),sender=new RecordedMusicSender(port2);let reject;
 const source={closed:false,read(){return new Promise((_,r)=>reject=r);},close(){this.closed=true;}};
 sender.start(1,source);await wait(()=>reject);sender.stop(2);reject(Error('retired failure'));await wait(()=>queue.generation===2);assert.equal(queue.status,'stopped');assert.ok(source.closed);sender.close();receiver.close();
}
{
 const {queue,port2,receiver}=fixture();port2.postMessage({kind:'begin',generation:1});let credit;port2.onmessage=({data})=>credit=data;await wait(()=>credit);
 port2.postMessage({kind:'pcm',generation:1,credit:credit.credit+1,values:new Int16Array(2)});await wait(()=>queue.status==='failed');assert.equal(queue.queuedFrames,0);receiver.close();port2.close();
}
{
 const {queue,port2,receiver}=fixture(),sender=new RecordedMusicSender(port2),source={closed:false,async read(){throw Error('codec trap');},close(){this.closed=true;}};
 sender.start(1,source);await wait(()=>queue.status==='failed');assert.ok(source.closed);sender.close();receiver.close();
}
{
 const {queue,port2,receiver}=fixture(),sender=new RecordedMusicSender(port2);let pending;
 const source={closed:false,read(){return new Promise(resolve=>pending=resolve);},close(){this.closed=true;}};
 sender.start(1,source);await wait(()=>pending);receiver.close();await wait(()=>source.closed);pending(new Int16Array(2));sender.close();assert.equal(queue.queuedFrames,0);
}
{
 const {port2,receiver}=fixture(),sender=new RecordedMusicSender(port2),source=decoder(1600,12);
 sender.start(1,source);receiver.close();await wait(()=>source.closed);sender.close();
}
{
 const {queue,port2,receiver}=fixture(),sender=new RecordedMusicSender(port2);let pending;
 sender.start(1,{read(){return new Promise(resolve=>pending=resolve);},close(){}});await wait(()=>pending);sender.close();await wait(()=>queue.status==='failed');receiver.close();pending(new Int16Array(2));
}
{
 const {queue,port2,receiver}=fixture(),sender=new RecordedMusicSender(port2),source=decoder(20000,987);
 sender.start(1,source);await wait(()=>queue.queuedFrames===4800);receiver.close();assert.equal(queue.queuedFrames,0,'disconnect must discard queued PCM');assert.equal(queue.status,'failed');await wait(()=>source.closed);sender.close();
}
{
 const {queue,port2,receiver}=fixture(),sender=new RecordedMusicSender(port2),source=decoder(1600,333);
 sender.start(1,source);await wait(()=>source.closed);assert.equal(queue.queuedFrames,800);receiver.close();assert.equal(queue.queuedFrames,0);assert.equal(queue.status,'failed');sender.close();
}
{
 const {queue,port2,receiver}=fixture(),sender=new RecordedMusicSender(port2),source=decoder(1600,333);
 sender.start(1,source);await wait(()=>source.closed);queue.read(new Int16Array(1600));await wait(()=>queue.status==='ended');receiver.close();assert.equal(queue.status,'ended','completed tracks retain their terminal status');sender.close();
}
{
 const {queue,port2,receiver}=fixture(),budget=new RecordedBudget(3200),sender=new RecordedMusicSender(port2,budget);let release;
 const old={read(){return new Promise(resolve=>release=resolve);},close(){}};
 sender.start(1,old);await wait(()=>release);assert.equal(budget.used,3200);
 const next=decoder(1600,99);sender.start(2,next);await wait(()=>queue.status==='failed');assert.equal(next.reads,0,'overcommitted read reached decoder');assert.ok(next.closed);assert.equal(budget.used,3200,'replacement released an outstanding old block');
 sender.close();assert.equal(budget.used,3200);release(new Int16Array(1600));await wait(()=>budget.used===0);receiver.close();
}
{
 const {queue,port2,receiver}=fixture(),budget=new RecordedBudget(3200),sender=new RecordedMusicSender(port2,budget),source=decoder(12000,42);
 sender.start(1,source);await wait(()=>queue.queuedFrames===4800);await wait(()=>budget.used===0);assert.equal(source.reads,6);
 while(queue.status!=='ended'){queue.read(new Int16Array(1600));receiver.poll();await new Promise(resolve=>setTimeout(resolve,1));}
 await wait(()=>budget.used===0);sender.close();receiver.close();
}
{
 const {queue,port2,receiver}=fixture(),sender=new RecordedMusicSender(port2),source=decoder(1600,678);
 // The committed worklet batch establishes the generation before transport attach.
 queue.begin(1);sender.start(1,source);await wait(()=>queue.queuedFrames===800);await wait(()=>source.closed);assert.equal(queue.read(new Int16Array(1600)),800);await wait(()=>queue.status==='ended');sender.close();receiver.close();
}
{
 const {queue,port2,receiver}=fixture(),sender=new RecordedMusicSender(port2),source=decoder(20000,456);
 queue.begin(1);queue.pause(true);sender.start(1,source);await wait(()=>queue.queuedFrames===4800);assert.equal(queue.status,'paused');assert.equal(queue.playedFrames,0);assert.equal(source.reads,6);
 port2.postMessage({kind:'begin',generation:1});await new Promise(resolve=>setTimeout(resolve,5));assert.equal(source.reads,6);
 queue.stop(2);sender.stop(2);await wait(()=>source.closed);assert.equal(queue.queuedFrames,0);assert.equal(queue.status,'stopped');sender.close();receiver.close();
}
// A committed replacement can fail before its decoder sends begin. A prior
// terminal generation must not hide the replacement's failure indefinitely.
for(const terminal of ['ended','failed','stopped'])for(const paused of [false,true]){
 const {queue,port2,receiver}=fixture(),sender=new RecordedMusicSender(port2);
 try{
  if(terminal==='stopped'){sender.stop(1);await wait(()=>queue.status==='stopped'&&queue.generation===1);}
  else if(terminal==='failed'){sender.start(1,{async read(){throw Error('old codec failure');},close(){}});await wait(()=>queue.status==='failed');}
  else{sender.start(1,decoder(0,0));await wait(()=>queue.status==='ended');}
  assert.ok(queue.begin(2));if(paused)assert.ok(queue.pause(true));
  port2.postMessage({kind:'failed',generation:1});await new Promise(resolve=>setTimeout(resolve,5));
  assert.equal(queue.status,paused?'paused':'loading','stale failure changed the replacement');
  assert.ok(sender.failGeneration(2));await wait(()=>queue.status==='failed');
  port2.postMessage({kind:'begin',generation:2});await new Promise(resolve=>setTimeout(resolve,5));
  assert.equal(queue.status,'failed','late begin resurrected a failed generation');
  assert.ok(queue.begin(3));assert.ok(sender.start(3,decoder(1600,321)));
  await wait(()=>queue.queuedFrames===800);assert.equal(queue.status,'playing');
 }finally{sender.close();receiver.close();}
}
{
 const {queue,port2,receiver}=fixture(),sender=new RecordedMusicSender(port2);
 try{
  const source=decoder(1600,432);let acknowledgements=0;
  port2.addEventListener('message',({data})=>{if(data.kind==='ack')acknowledgements++;});
  sender.start(1,source);await wait(()=>queue.queuedFrames===800&&source.closed&&acknowledgements===2);
  // EOF is acknowledged while the final PCM block is still queued.
  port2.postMessage({kind:'failed',generation:1});
  await new Promise(resolve=>setTimeout(resolve,10));assert.equal(queue.queuedFrames,800);assert.equal(queue.status,'playing');
  queue.read(new Int16Array(1600));assert.equal(queue.status,'ended');
 }finally{sender.close();receiver.close();}
}
// A committed output generation can grant credit before the sender's begin
// crosses the opposite MessagePort direction. Begin must retain that read.
for(const paused of [false,true])for(const firstDelivered of [false,true])for(const generation of [1,2]){
 const {queue,port2,receiver}=fixture();let begin,release,reads=0;
 const delayed={set onmessage(value){port2.onmessage=value;},set onmessageerror(value){port2.onmessageerror=value;},start(){port2.start();},close(){port2.close();},postMessage(data,...args){if(data.kind==='begin'){begin=data;return;}port2.postMessage(data,...args);}};
 const sender=new RecordedMusicSender(delayed),source={closed:false,read(){reads++;if(firstDelivered&&reads===1)return Promise.resolve(new Int16Array(1600).fill(567));return new Promise(resolve=>release=resolve);},close(){this.closed=true;}};
 try{
  queue.begin(generation);if(paused)queue.pause(true);receiver.poll();assert.ok(sender.start(generation,source));
  await wait(()=>release);assert.equal(reads,firstDelivered?2:1);
  port2.postMessage(begin);await new Promise(resolve=>setTimeout(resolve,10));
  assert.notEqual(queue.status,'failed','Late begin invalidated a current-generation read');
  assert.equal(source.closed,false);assert.equal(reads,firstDelivered?2:1,'Begin issued duplicate credit');
  release(new Int16Array(1600).fill(456));await wait(()=>queue.queuedFrames===(firstDelivered?1600:800));
  assert.equal(queue.status,paused?'paused':'playing');if(paused)queue.pause(false);
  const output=new Int16Array(1600);assert.equal(queue.read(output),800);assert.ok(output.every(value=>value===(firstDelivered?567:456)));
  if(firstDelivered){assert.equal(queue.read(output),800);assert.ok(output.every(value=>value===456));}
 }finally{sender.close();receiver.close();release?.(new Int16Array(0));}
}
// Preserving a current read must not retain credit issued for an older generation.
{
 const {queue,port2,receiver}=fixture(),sender=new RecordedMusicSender(port2),source=decoder(1600,789);
 try{
  queue.begin(1);receiver.poll();queue.begin(2);assert.ok(sender.start(2,source));
  await wait(()=>queue.queuedFrames===800);const output=new Int16Array(1600);assert.equal(queue.read(output),800);assert.ok(output.every(value=>value===789));
 }finally{sender.close();receiver.close();}
}
for(const terminal of ['stopped','ended','failed']){
 const {queue,port2,receiver}=fixture();let reads=0;port2.onmessage=({data})=>{if(data.kind==='read')reads++;};port2.start();
 try{
  queue.begin(1);if(terminal==='stopped')queue.stop(2);else if(terminal==='ended')queue.finish(1);else queue.fail(1);
  receiver.poll();await new Promise(resolve=>setTimeout(resolve,10));assert.equal(reads,0,'Terminal generation received decoder credit');
 }finally{receiver.close();port2.close();}
}
// Credit may also arrive while no decoder has been attached yet. Begin must
// resend it, while the sender must deduplicate an already accepted request.
for(const paused of [false,true]){
 const {queue,port2,receiver}=fixture(),sender=new RecordedMusicSender(port2),source=decoder(1600,654);let early=0;
 port2.addEventListener('message',({data})=>{if(data.kind==='read')early++;});
 try{
  queue.begin(1);if(paused)queue.pause(true);receiver.poll();await wait(()=>early===1);assert.equal(source.reads,0);
  sender.start(1,source);await wait(()=>queue.queuedFrames===800&&source.closed);
  assert.equal(source.reads,2);assert.equal(queue.status,paused?'paused':'playing');if(paused)queue.pause(false);
  const output=new Int16Array(1600);assert.equal(queue.read(output),800);assert.ok(output.every(value=>value===654));
 }finally{sender.close();receiver.close();}
}
// Retransmission is idempotent only for the exact accepted request. It does
// not admit a changed block size or recycle an old generation's credit.
{
 const {port1,port2}=new MessageChannel(),budget=new RecordedBudget(3200),sender=new RecordedMusicSender(port1,budget);let release,reads=0;
 const source={closed:false,read(){reads++;return new Promise(resolve=>release=resolve);},close(){this.closed=true;}};
 try{
  sender.start(1,source);port2.postMessage({kind:'read',generation:1,credit:1,frames:800});await wait(()=>release);
  port2.postMessage({kind:'read',generation:1,credit:1,frames:800});await new Promise(resolve=>setTimeout(resolve,10));
  assert.equal(reads,1);assert.equal(source.closed,false);assert.equal(budget.used,3200);
  port2.postMessage({kind:'read',generation:1,credit:1,frames:799});await wait(()=>source.closed);
  const replacement=decoder(1600,777);sender.start(2,replacement);port2.postMessage({kind:'read',generation:2,credit:1,frames:800});await wait(()=>replacement.closed);assert.equal(replacement.reads,0);
 }finally{sender.close();port2.close();release?.(new Int16Array(0));await wait(()=>budget.used===0);}
}
console.log('Real MessageChannel: bounded demand, complete PCM, replacement, stale success/failure, stop, bad credit, decoder fault, terminal replacement failures, delayed begin credit ownership and recovery passed');
