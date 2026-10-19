import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
const source=fs.readFileSync(new URL('../web-player/recorded-decoder.js',import.meta.url));
const {RecordedDecoder,inspectCompanion}=await import('data:text/javascript;base64,'+source.toString('base64'));
const [fixture,input,companion]=process.argv.slice(2);
assert.ok(fixture&&input&&companion,'Use tools/check_browser_audio_decoder.py');
const load=name=>new Uint8Array(fs.readFileSync(path.join(fixture,name+'.wasm')));
for(const mode of ['incomplete','duplicate','oversize','shared']){
 const staged=new RecordedDecoder(load('valid'));staged.beginInput(2);
 if(mode==='incomplete')assert.throws(()=>staged.finishInput());
 if(mode==='duplicate'){staged.writeInput(0,new Uint8Array([1]));assert.throws(()=>staged.writeInput(0,new Uint8Array([1])));}
 if(mode==='oversize')assert.throws(()=>staged.writeInput(0,new Uint8Array(65537)));
 if(mode==='shared')assert.throws(()=>staged.writeInput(0,new Uint8Array(new SharedArrayBuffer(2))));
 assert.ok(staged.failed,mode+' did not poison incomplete input');assert.throws(()=>staged.read(new Int16Array(2)));staged.close();
}
const staged=new RecordedDecoder(load('valid'));staged.beginInput(2);staged.writeInput(0,new Uint8Array([1]));staged.writeInput(1,new Uint8Array([2]));staged.finishInput();const stagedPCM=new Int16Array(2);assert.equal(staged.read(stagedPCM),2);assert.deepEqual(Array.from(stagedPCM),[4660,-292]);staged.close();
const header=[0,97,115,109,1,0,0,0];
const module=memory=>new Uint8Array([...header,5,memory.length,...memory]);
assert.equal(inspectCompanion(module([1,1,1,96])),6291456);
for(const bad of [new Uint8Array(),module([1,0,1]),module([1,1,1,97]),module([1,3,1,96]),module([1,1,97,96]),module([2,1,1,96]),new Uint8Array([...header,8,1,0]),new Uint8Array([...header,5,4,1])])assert.throws(()=>inspectCompanion(bad));
assert.throws(()=>inspectCompanion(module([1,1,1,96]),65536));
assert.throws(()=>new RecordedDecoder(module([1,1,1,96])));
for(const name of ['signature','import','table','unbounded','memory-max','start','multi-memory','memory-export'])assert.throws(()=>new RecordedDecoder(load(name)),name);
const supplied=load('valid'),BaseModule=WebAssembly.Module;
function MutatingModule(bytes){supplied.fill(0);return new BaseModule(bytes);}
Object.setPrototypeOf(MutatingModule,BaseModule);WebAssembly.Module=MutatingModule;
try{const snapshot=new RecordedDecoder(supplied);snapshot.open(new Uint8Array([1]));const out=new Int16Array(2);assert.equal(snapshot.read(out),2);assert.deepEqual([...out],[4660,-292]);snapshot.close();}finally{WebAssembly.Module=BaseModule;}
const valid=new RecordedDecoder(load('valid'));
assert.equal(valid.maximumMemoryBytes,6291456);
assert.throws(()=>valid.read(new Int16Array(2)));
assert.throws(()=>valid.open(new Uint8Array()));
valid.open(new Uint8Array([1]));
assert.throws(()=>valid.open(new Uint8Array([1])));
for(const output of [new Int16Array(),new Int16Array(1),new Int16Array(1602),new Uint16Array(2)])assert.throws(()=>valid.read(output));
assert.equal(valid.failed,false);
const samples=new Int16Array(8).fill(2345);assert.equal(valid.read(samples),2);assert.deepEqual([...samples],[4660,-292,2345,2345,2345,2345,2345,2345]);
valid.rewind();valid.close();assert.throws(()=>valid.read(samples));assert.throws(()=>valid.rewind());
for(const name of ['negative','odd','oversize','range','work','open','rewind']){
 const decoder=new RecordedDecoder(load(name)),out=new Int16Array(2).fill(2345);
 if(name==='open')assert.throws(()=>decoder.open(new Uint8Array([1])));
 else {decoder.open(new Uint8Array([1]));if(name==='rewind')assert.throws(()=>decoder.rewind());else assert.throws(()=>decoder.read(out));}
 assert.equal(decoder.failed,true);assert.deepEqual([...out],[2345,2345]);assert.throws(()=>decoder.read(out));assert.throws(()=>decoder.rewind());assert.throws(()=>decoder.open(new Uint8Array([1])));decoder.close();
}
const wasm=new Uint8Array(fs.readFileSync(companion));
for(const record of JSON.parse(fs.readFileSync(path.join(fixture,'reference.json')))){
 const encoded=new Uint8Array(fs.readFileSync(path.join(input,record.name)));
 assert.equal(createHash('sha256').update(encoded).digest('hex'),record.input_sha256);
 const decoder=new RecordedDecoder(wasm);decoder.open(encoded);
 let count=0,step=0;const hash=createHash('sha256'),output=new Int16Array(1600),bytes=new Uint8Array(3200),view=new DataView(bytes.buffer);let first;
 for(;;){output.fill(2345);const requested=[2,226,14,1598,1600][step++%5],n=decoder.read(output.subarray(0,requested));assert.ok(output.subarray(n).every(value=>value===2345));if(n===0)break;
  if(!first)first=output.slice(0,n);for(let i=0;i<n;i++)view.setInt16(i*2,output[i],true);hash.update(bytes.subarray(0,n*2));count+=n;
 }
 assert.equal(count,record.samples);assert.equal(hash.digest('hex'),record.sha256);
 decoder.rewind();const repeated=new Int16Array(first.length);assert.equal(decoder.read(repeated),first.length);assert.deepEqual(repeated,first);decoder.close();
 console.log('JavaScript/native decoder complete PCM matches:',record.name,count,'samples');
}
console.log('Decoder resource, endian, output ownership, error, sticky failure, rewind and all nine complete-track hashes passed.');
