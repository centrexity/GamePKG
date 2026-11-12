import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const worker=vm.createContext({TextEncoder,TextDecoder});
vm.runInContext(readFileSync(new URL('../web-player/worker.js',import.meta.url),'utf8'),worker);
vm.runInContext(`instance={exports:{memory:new WebAssembly.Memory({initial:17})}};manifest={permissions:{save:true}};save=new Uint8Array([9]);`,worker);
const api=vm.runInContext(`({chunk:api.save_chunk,canonical,write:api.save_write,state:()=>({bytes:Array.from(save),dirty:saveDirty,pending:savePending,budget:saveBudget}),reset:()=>{savePending=null;saveBudget=0;saveDirty=false;},put:values=>new Uint8Array(instance.exports.memory.buffer).set(values)})`,worker);
api.put([1,2,3]);
assert.equal(api.chunk(0,6,0,3),0);
assert.deepEqual(Array.from(api.state().bytes),[9]);assert.equal(api.state().dirty,false);
assert.equal(api.chunk(4,6,0,2),-1);assert.equal(api.chunk(3,6,0,3),-1);
api.reset();assert.equal(api.chunk(0,6,0,3),0);
api.put([4,5,6]);assert.equal(api.canonical['save-chunk'](3,6,0,3),1);
assert.deepEqual(Array.from(api.state().bytes),[1,2,3,4,5,6]);assert.equal(api.state().dirty,true);
for(const args of [[0,1048577,0,1],[0,1,-1,1],[0,1,1114112,1],[0,1,0,0],[0,1048576,0,65537],[-1,2,0,1],[1,2,0,2]]) {
 api.reset();assert.equal(api.chunk(...args),-1);assert.deepEqual(Array.from(api.state().bytes),[1,2,3,4,5,6]);
}
api.reset();api.put([7]);
assert.equal(api.chunk(0,2,0,1),0);assert.equal(api.write(0,1),0);assert.equal(api.chunk(1,2,0,1),-1);
api.reset();
for(let offset=0;offset<1048576;offset+=65536)assert.equal(api.chunk(offset,1048576,0,65536),0);
assert.equal(api.state().bytes.length,1048576);assert.equal(api.state().budget,1048576);
assert.equal(api.chunk(0,1,0,1),-1,'chunk writes share the existing per-tick transfer budget');
api.reset();assert.equal(api.chunk(0,0,0,0),0);assert.equal(api.state().bytes.length,0);
vm.runInContext('manifest.permissions.save=false',worker);assert.equal(api.chunk(0,1,0,1),-1);
console.log('Chunked browser saves preserve old data until completion, validate order/ranges/permissions, and retain the shared 1 MiB tick budget.');
