import assert from 'node:assert/strict';import {readFileSync} from 'node:fs';import vm from 'node:vm';
const context=vm.createContext({TextEncoder,TextDecoder,createImageBitmap(){throw Error('precise pixels must not pass through Canvas');}});vm.runInContext(readFileSync(new URL('../web-player/worker.js',import.meta.url),'utf8'),context);
const worker=vm.runInContext('({prepare:prepareImages,set:(hex)=>{assets=new Map([["sample.png",decode(hex)]]);},image:()=>images.get("sample.png"),decode})',context);
worker.set('89504e470d0a1a0a0000000d4948445200000002000000010806000000f4227f8a0000001149444154789c63fccf25f29ff7430023001168036cb0bdb5330000000049454e44ae426082');await worker.prepare({'sample.png':{width:2,height:1,rgba:'ff0a14ff0cfa6400'}});
assert.equal(worker.image().precise,true);assert.deepEqual(Array.from(worker.image().pixels),[255,10,20,255,12,250,100,0]);
for(const value of [{width:3,height:1,rgba:'ff0a14ff0cfa6400'},{width:2,height:1,rgba:'ff'},{width:2,height:1,rgba:'zz0a14ff0cfa6400'}])await assert.rejects(worker.prepare({'sample.png':value}));
assert.deepEqual(Array.from(worker.decode('00ff89abcdef')),[0,255,137,171,205,239]);for(const bad of [null,'f','FF','0g'])assert.throws(()=>worker.decode(bad));
console.log('Precise exported PNG pixels preserve hidden RGB, validate dimensions/length/hex and bypass Canvas conversion.');
