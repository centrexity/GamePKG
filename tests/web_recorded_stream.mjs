import assert from 'node:assert/strict';
import fs from 'node:fs';
import {createHash} from 'node:crypto';
const url=code=>'data:text/javascript;base64,'+Buffer.from(code).toString('base64');
const read=name=>fs.readFileSync(new URL('../web-player/'+name,import.meta.url),'utf8');
const decoder=url(read('recorded-decoder.js')),queue=url(read('recorded-queue.js')),mixer=url(read('recorded-mixer.js').replace("'./recorded-decoder.js'",JSON.stringify(decoder)));
const {RecordedStream}=await import(url(read('recorded-stream.js').replace("'./recorded-mixer.js'",JSON.stringify(mixer)).replace("'./recorded-queue.js'",JSON.stringify(queue))));
const wasm=new Uint8Array(fs.readFileSync(process.argv[2])),reference=JSON.parse(fs.readFileSync(new URL('fixtures/sdl-live-audio-reference.json',import.meta.url)));
const clip=slot=>{const frames=[1025,2051,4096,777][slot],bytes=new Uint8Array(frames*4),view=new DataView(bytes.buffer);for(let i=0;i<frames;i++)for(let c=0;c<2;c++)view.setInt16(i*4+c*2,(i*(slot*1543+197)+c*12345+slot*313)%65536-32768,true);return bytes;};
const pcm=(at,frames)=>Int16Array.from({length:frames*2},(_,i)=>((at+Math.floor(i/2))*7919+(i%2)*31337)%65536-32768);
for(const record of reference.cases)for(const get of [1,113,800]){
 const stream=new RecordedStream(wasm);assert.ok(stream.fixedBufferBytes<=65536);assert.ok(stream.begin(1));assert.ok(stream.push(1,new Int16Array([1000,2000])));assert.ok(stream.begin(2));assert.equal(stream.push(1,new Int16Array([3000,4000])),false);
 assert.ok(stream.musicVolume(record.music_volume));assert.ok(stream.effectVolume(record.effect_volume));for(let slot=0;slot<4;slot++){assert.ok(stream.load(slot,clip(slot)));assert.equal(stream.play(slot),slot);}
 let input=0,finished=false,frames=0;const output=new Int16Array(get*2),hash=createHash('sha256');
 while(frames<record.frames){while(input<6000&&stream.availableFrames>=800){const count=Math.min(800,6000-input);assert.ok(stream.push(2,pcm(input,count)));input+=count;}if(input===6000&&!finished){assert.ok(stream.finishMusic(2));finished=true;}
  const n=Math.min(get,record.frames-frames);output.fill(2345);assert.equal(stream.render(output.subarray(0,n*2)),n);assert.ok(output.subarray(n*2).every(value=>value===2345));const bytes=new Uint8Array(n*4),view=new DataView(bytes.buffer);for(let i=0;i<n*2;i++)view.setInt16(i*2,output[i],true);hash.update(bytes);frames+=n;assert.ok(stream.queuedFrames<=4800);
 }
 assert.equal(hash.digest('hex'),record.sha256,`live music ${record.music_volume}, effects ${record.effect_volume}, reads ${get}`);assert.equal(stream.playedFrames,6000);assert.equal(stream.renderedFrames,8192);assert.equal(stream.musicStatus,'ended');assert.equal(stream.activeEffects,0);stream.close();assert.equal(stream.fixedBufferBytes,0);assert.equal(stream.pcmBytes,0);output.fill(2345);assert.equal(stream.render(output),0);assert.ok(output.every(value=>value===2345));
}
for(const mode of ['pause','fail','stop','underrun']){
 const stream=new RecordedStream(wasm);stream.begin(1);stream.musicVolume(128);stream.effectVolume(64);for(let slot=0;slot<4;slot++){stream.load(slot,clip(slot));stream.play(slot);}if(mode!=='underrun')for(let at=0;at<4800;at+=800)assert.ok(stream.push(1,pcm(at,800)));
 if(mode==='pause')assert.ok(stream.musicPause(true));else if(mode==='fail')assert.ok(stream.failMusic(1));else if(mode==='stop')assert.ok(stream.stopMusic(2));
 const hash=createHash('sha256'),out=new Int16Array(256);for(let at=0;at<8192;at+=128){assert.equal(stream.render(out),128);const bytes=new Uint8Array(out.length*2),view=new DataView(bytes.buffer);for(let i=0;i<out.length;i++)view.setInt16(i*2,out[i],true);hash.update(bytes);}
 assert.equal(hash.digest('hex'),reference.cases[6].sha256,mode+' interrupted effects');assert.equal(stream.playedFrames,0);assert.equal(stream.musicStatus,{pause:'paused',fail:'failed',stop:'stopped',underrun:'loading'}[mode]);assert.equal(stream.queuedFrames,mode==='pause'?4800:0);assert.equal(stream.activeEffects,0);const before=stream.renderedFrames;assert.throws(()=>stream.render(new Int16Array(1602)));assert.equal(stream.renderedFrames,before);stream.close();
}
{
 const left=new RecordedStream(wasm),right=new RecordedStream(wasm);
 for(const stream of [left,right]){stream.load(0,clip(0));stream.play(0);stream.begin(1);stream.push(1,pcm(0,800));}
 for(const batch of [[[1,0],[7,2],[0,99]],[[3,0],[7,2],[6,1]],Array(33).fill([1,64]),[[0,0],[0,0],[0,0],[0,0]],[[4,2],[5,129]],[[4,2],[7,2]],[[1,NaN]],[[6,2]]]){
  assert.equal(left.applyControls(batch),false);assert.equal(left.generation,1);assert.equal(left.queuedFrames,800);assert.equal(left.activeEffects,1);
 }
 const a=new Int16Array(256),b=new Int16Array(256);left.render(a);right.render(b);assert.deepEqual(a,b,'rejected batch changed audible output');
 assert.ok(left.applyControls([[3,0],[0,0],[0,0],[4,2],[5,64],[6,1]]));assert.equal(left.activeEffects,2);assert.equal(left.generation,2);assert.equal(left.musicStatus,'paused');assert.equal(left.queuedFrames,0);
 assert.ok(left.failMusic(2));assert.ok(left.applyControls([[1,32],[6,0]],2));assert.equal(left.musicStatus,'failed');assert.equal(left.applyControls([[6,0]],1),false);
 left.close();right.close();assert.equal(left.applyControls([]),false);
}
console.log('Live queue/mix/conversion: 24 exact SDL prefixes, finite-track-to-silence, epoch replacement, pause/failure/stop with continuing effects, fixed buffers, rejected reads and close passed.');
