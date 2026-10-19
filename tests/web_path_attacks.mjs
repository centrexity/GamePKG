// Invoke the shipped worker imports directly, bypassing all guest wrappers.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const source = readFileSync(new URL('../web-player/worker.js', import.meta.url), 'utf8');
const context = vm.createContext({TextDecoder, TextEncoder, WebAssembly, Uint8Array, DataView});
vm.runInContext(source, context);
vm.runInContext(`instance={exports:{memory:new WebAssembly.Memory({initial:1}),cabi_realloc:()=>55000}};
 assets=new Map([['hello.txt',new Uint8Array([65,66,67])]]);
 images=new Map([['hello.txt',{width:1,height:1,pixels:new Uint8Array([1,2,3,255]),precise:true}]]);
 shaderCount=1;manifest={permissions:{save:false,audio:false}};roms=[];save=new Uint8Array();`, context);
const worker = vm.runInContext('({api,canonical,validAssetPath,assetName,memory:instance.exports.memory})', context);
const memory = new Uint8Array(worker.memory.buffer);
const view = new DataView(worker.memory.buffer);
const encode = value => {const bytes=new TextEncoder().encode(value);memory.set(bytes,256);return bytes.length;};
const rejected = new Set([
 '', '.', '..', '../hello.txt', 'a/../../hello.txt', './hello.txt', '/hello.txt',
 '//server/share', 'C:hello.txt', 'C:/hello.txt', '\\hello.txt', '\\\\server\\share',
 '\\\\?\\C:\\hello.txt', '\\\\.\\NUL', 'a\\hello.txt', 'a/..\\hello.txt',
 'hello.txt:secret', 'hello.txt::$DATA', 'a//hello.txt', 'a/hello.txt/',
 'a/.. /hello.txt', 'a/. /hello.txt', 'hello.txt.', 'hello.txt ', 'hello.txt. ',
 'hello\0.txt', 'hello.txt\0/../secret', 'hello\n.txt', 'hello\r.txt',
 'fi<le', 'fi>le', 'fi"le', 'fi|le', 'fi?le', 'fi*le',
]);
rejected.add('a'.repeat(256));rejected.add(Array(33).fill('a').join('/'));rejected.add(Array(32).fill('a'.repeat(32)).join('/'));
for(const name of ['CON','PRN','AUX','NUL','CONIN$','CONOUT$',
 ...['COM','LPT'].flatMap(prefix=>[...'123456789¹²³'].map(d=>prefix+d))]){
 for(const caseName of [name,name.toLowerCase()])for(const suffix of ['','.txt','.tar.gz',' ','.',' .txt','  .txt'])for(const prefix of ['','a/'])rejected.add(prefix+caseName+suffix);
}
for(let value=0;value<=159;value++)if(value<=31||value>=127)rejected.add('hello'+String.fromCharCode(value)+'.txt');
// A positive read/draw control must work before rejection assertions count.
let length=encode('hello.txt');
assert.equal(worker.api.asset_read(256,length,1,60000,8),2);
assert.deepEqual([...memory.slice(60000,60002)],[66,67]);
assert.equal(worker.api.draw_asset_image(256,length,0,0,1,1),0);
assert.equal(worker.api.draw_asset_image_linear_straight(256,length,0,0,1,1),0);
assert.equal(worker.canonical['draw-asset-image-linear-straight'](256,length,0,0,1,1),1);
assert.equal(worker.api.draw_shader_asset_image(0,256,length),0);
assert.equal(worker.canonical['draw-shader-asset-image'](0,256,length),1);
assert.equal(worker.api.draw_shader_asset_image(99,256,length),-1);
worker.canonical['asset-read'](256,length,0,3,50000);
assert.equal(view.getUint32(50000,true),1);
assert.equal(view.getUint32(50008,true),3);
vm.runInContext('instance.exports.cabi_realloc=()=>{throw Error("Denied read attempted allocation")}',context);
for(const path of rejected){
 assert.equal(worker.validAssetPath(path),false,JSON.stringify(path));
 length=encode(path);memory.fill(0xa5,60000,60008);
 assert.equal(worker.api.asset_read(256,length,0,60000,8),-1,JSON.stringify(path));
 assert.equal(worker.api.draw_asset_image(256,length,0,0,1,1),-1,JSON.stringify(path));
 assert.equal(worker.api.draw_asset_image_linear_straight(256,length,0,0,1,1),-1,JSON.stringify(path));
 assert.equal(worker.canonical['draw-asset-image-linear-straight'](256,length,0,0,1,1),0,JSON.stringify(path));
 assert.equal(worker.api.draw_shader_asset_image(0,256,length),-1,JSON.stringify(path));
 assert.equal(worker.canonical['draw-shader-asset-image'](0,256,length),0,JSON.stringify(path));
 worker.canonical['asset-read'](256,length,0,8,50000);
 assert.equal(view.getUint32(50000,true),0,JSON.stringify(path));
 assert.equal(worker.canonical['draw-asset-image'](256,length,0,0,1,1),0,JSON.stringify(path));
 assert.deepEqual([...memory.slice(60000,60008)],Array(8).fill(0xa5),JSON.stringify(path));
}
for(const path of ['%2e%2e/hello.txt','%252e%252e/hello.txt','..%2fhello.txt','..%5chello.txt','%2fetc%2fpasswd','．．/hello.txt','a∕hello.txt','a／hello.txt','~/hello.txt','$HOME/hello.txt','HELLO.TXT']){
 assert.equal(worker.validAssetPath(path),true,JSON.stringify(path));
 length=encode(path);assert.equal(worker.api.asset_read(256,length,0,60000,8),-1,JSON.stringify(path));
 worker.canonical['asset-read'](256,length,0,8,50000);assert.equal(view.getUint32(50000,true),0);
}
length=encode('hello.txt');
for(const [p,n,offset,out,count] of [
 [-1,length,0,60000,8],[2147483647,length,0,60000,8],[65535,length,0,60000,8],
 [256,-1,0,60000,8],[256,0,0,60000,8],[256,1025,0,60000,8],
 [256,length,-1,60000,8],[256,length,2147483647,60000,8],
 [256,length,0,-1,8],[256,length,0,2147483647,8],[256,length,0,65535,8],
 [256,length,0,60000,-1],[256,length,0,0,65537],
]){
 memory.fill(0xa5,60000,60008);assert.equal(worker.api.asset_read(p,n,offset,out,count),-1);
 assert.deepEqual([...memory.slice(60000,60008)],Array(8).fill(0xa5));
}
for(const invalid of [[255],[192,175],[237,160,128]]){
 memory.set(invalid,256);assert.equal(worker.api.asset_read(256,invalid.length,0,60000,8),-1);
 worker.canonical['asset-read'](256,invalid.length,0,8,50000);assert.equal(view.getUint32(50000,true),0);
}
for(const pointer of [-1,65535,2147483647,50001])assert.throws(()=>worker.canonical['asset-read'](256,1,0,8,pointer));
console.log(`Browser worker: ${rejected.size} hostile paths denied in core and component reads/images; encoding, pointer bounds, and output preservation passed.`);
