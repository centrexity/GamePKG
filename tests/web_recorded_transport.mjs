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
function fixture(){const queue=new RecordedQueue(),stream={get generation(){return queue.generation;},get availableFrames(){return queue.availableFrames;},begin:g=>queue.begin(g),stopMusic:g=>queue.stop(g),failMusic:g=>queue.fail(g),finishMusic:g=>queue.finish(g),push:(g,v)=>queue.push(g,v)};const {port1,port2}=new MessageChannel();return {queue,port1,port2,receiver:new RecordedMusicReceiver(port1,stream)};}
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
 sender.start(1,source);await wait(()=>source.closed);queue.read(new Int16Array(1600));assert.equal(queue.status,'ended');receiver.close();assert.equal(queue.status,'ended','completed tracks retain their terminal status');sender.close();
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
console.log('Real MessageChannel: bounded demand, complete PCM, replacement, stale success/failure, stop, bad credit, decoder fault passed');
