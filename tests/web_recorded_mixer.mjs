import assert from 'node:assert/strict';
import fs from 'node:fs';
import {createHash} from 'node:crypto';
const source=fs.readFileSync(new URL('../web-player/recorded-mixer.js',import.meta.url),'utf8'),base=fs.readFileSync(new URL('../web-player/recorded-decoder.js',import.meta.url)).toString('base64');
const {RecordedMixer,mixSample}=await import('data:text/javascript;base64,'+Buffer.from(source.replace("'./recorded-decoder.js'","'data:text/javascript;base64,"+base+"'")).toString('base64'));
const reference=JSON.parse(fs.readFileSync(new URL('fixtures/sdl-s16-mix-reference.json',import.meta.url)));
const wasm=new Uint8Array(fs.readFileSync(process.argv[2]));
for(let volume=0;volume<=128;volume++){
 const hash=createHash('sha256'),bytes=new Uint8Array(4096),view=new DataView(bytes.buffer);
 for(const destination of reference.destinations)for(let first=-32768;first<=32767;first+=2048){for(let i=0;i<2048;i++)view.setInt16(i*2,mixSample(destination,first+i,volume),true);hash.update(bytes);}
 assert.equal(hash.digest('hex'),reference.sha256[volume],'SDL arithmetic volume '+volume);
}
const clip=slot=>{const frames=[1025,2051,4096,777][slot],bytes=new Uint8Array(frames*4),view=new DataView(bytes.buffer);for(let i=0;i<frames;i++)for(let c=0;c<2;c++)view.setInt16(i*4+c*2,(i*(slot*1543+197)+c*12345+slot*313)%65536-32768,true);return bytes;};
for(const record of [...reference.combined_streams,{...reference.combined_streams[0],volume:128,paused:true}])for(const [put,get] of [[1,1],[113,799],[800,71]]){
 const mixer=new RecordedMixer(wasm);assert.equal(mixer.reservedBytes,327680);assert.ok(mixer.volume(record.volume));if(record.paused)assert.ok(mixer.pause(true));
 for(let slot=0;slot<4;slot++){assert.ok(mixer.load(slot,clip(slot)));assert.equal(mixer.play(slot),slot);}
 assert.equal(mixer.play(0),-1);assert.equal(mixer.unload(0),false);
 const hash=createHash('sha256'),output=new Int16Array(get*2);let at=0,frames=0;
 for(;;){output.fill(2345);const count=mixer.render(output);assert.ok(output.subarray(count*2).every(value=>value===2345));if(count){const bytes=new Uint8Array(count*4),view=new DataView(bytes.buffer);for(let i=0;i<count*2;i++)view.setInt16(i*2,output[i],true);hash.update(bytes);frames+=count;continue;}if(at===6000)break;
  const countIn=Math.min(mixer.inputCapacity,put,6000-at);assert.ok(countIn>0);const music=new Int16Array(countIn*2);for(let i=0;i<countIn;i++)for(let c=0;c<2;c++)music[i*2+c]=((at+i)*7919+c*31337)%65536-32768;mixer.append(music);at+=countIn;if(at===6000)mixer.finish();
 }
 assert.equal(frames,record.frames);assert.equal(hash.digest('hex'),record.sha256,`combined volume ${record.volume}, chunks ${put}/${get}`);assert.equal(mixer.active,record.paused?4:0);assert.throws(()=>mixer.append(new Int16Array(2)));mixer.close();assert.equal(mixer.pcmBytes,0);
}
const tiny=new RecordedMixer(wasm,327684),one=new Uint8Array([0x39,0x30,0x60,0xa4]);assert.ok(tiny.load(0,one));one.fill(0);assert.equal(tiny.pcmBytes,4);assert.equal(tiny.load(1,new Uint8Array(4)),false);assert.equal(tiny.load(32,new Uint8Array(4)),false);assert.equal(tiny.play(0),0);assert.equal(tiny.unload(0),false);assert.throws(()=>tiny.append(new Int16Array(1602)));assert.equal(tiny.active,1);assert.ok(tiny.pause(true));tiny.append(new Int16Array(2));assert.equal(tiny.active,1);assert.ok(tiny.pause(false));assert.equal(tiny.volume(129),false);tiny.append(new Int16Array(2));assert.equal(tiny.active,0);assert.ok(tiny.unload(0));assert.ok(tiny.load(1,new Uint8Array([1,0,255,255])));assert.equal(tiny.play(1),0);tiny.stop();assert.equal(tiny.active,0);assert.ok(tiny.unload(1));tiny.finish();const out=new Int16Array(1600);let audible=false;for(;;){const count=tiny.render(out);if(!count)break;audible||=out.subarray(0,count*2).some(value=>value!==0);}assert.ok(audible,'clip storage aliased mutated input');assert.equal(tiny.pcmBytes,0);tiny.close();assert.throws(()=>tiny.render(out));
const muted=new RecordedMixer(wasm);assert.ok(muted.load(0,new Uint8Array([1,0,255,255])));assert.equal(muted.play(0),0);assert.ok(muted.volume(0));muted.append(new Int16Array(2));assert.equal(muted.active,0);muted.finish();for(;;){const n=muted.render(out);if(!n)break;assert.ok(out.subarray(0,n*2).every(value=>value===0));}muted.close();
assert.throws(()=>new RecordedMixer(wasm,327679));assert.throws(()=>new RecordedMixer(wasm,4194305));
const bounds=new RecordedMixer(wasm);assert.equal(bounds.load(0,new Uint8Array(2097156)),false);assert.equal(bounds.load(0,new Uint8Array(2)),false);assert.equal(bounds.pcmBytes,0);bounds.close();
const adopted=new RecordedMixer(wasm,327684);assert.ok(adopted.reserveClip(0,4,7));assert.equal(adopted.pcmBytes,4);assert.equal(adopted.has(0),false);assert.equal(adopted.play(0),-1);assert.equal(adopted.reserveClip(1,4,8),false);
let original=new Uint8Array([57,48,96,164]),transferred=structuredClone(original,{transfer:[original.buffer]});assert.equal(original.byteLength,0);
assert.equal(adopted.adoptClip(0,transferred,6),false);assert.equal(adopted.adoptClip('0',transferred,7),false);assert.ok(adopted.adoptClip(0,transferred,7));assert.ok(adopted.has(0));assert.equal(adopted.cancelClip(0,7),false);assert.equal(adopted.play(0),0);assert.equal(adopted.unload(0),false);adopted.stop();assert.ok(adopted.unload(0));assert.equal(adopted.pcmBytes,0);
assert.ok(adopted.reserveClip(0,4,9));assert.equal(adopted.cancelClip(0,8),false);assert.equal(adopted.adoptClip(0,new Uint8Array(new SharedArrayBuffer(4)),9),false);assert.ok(adopted.cancelClip(0,9));assert.equal(adopted.pcmBytes,0);adopted.close();
console.log('Browser mixer: all 59,179,008 SDL sample/volume/destination combinations, 18 exact combined conversion hashes plus three paused-effect music comparisons, four-slot order/reuse, ownership, budgets, pause/mute and rejected-buffer checks passed.');
