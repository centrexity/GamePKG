import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {deflateSync} from 'node:zlib';
import vm from 'node:vm';
const inflaterURL='data:text/javascript;base64,'+readFileSync(new URL('../web-player/png-inflate.js',import.meta.url)).toString('base64');
const decoderSource=readFileSync(new URL('../web-player/png-exact.js',import.meta.url),'utf8').replace("'./png-inflate.js'",JSON.stringify(inflaterURL));
const decoder=await import('data:text/javascript;base64,'+Buffer.from(decoderSource).toString('base64'));
let decodes=0,validations=0;
const decoderModule={validateExactPNG:bytes=>{validations++;return decoder.validateExactPNG(bytes);},decodeExactPNG:async bytes=>{decodes++;return decoder.decodeExactPNG(bytes);},decodeExactPNGSync:bytes=>{decodes++;return decoder.decodeExactPNGSync(bytes);}};
function crc(bytes){let value=0xffffffff;for(const byte of bytes){value^=byte;for(let bit=0;bit<8;bit++)value=value&1?0xedb88320^(value>>>1):value>>>1;}return(value^0xffffffff)>>>0;}
function chunk(kind,data){const out=Buffer.alloc(data.length+12);out.writeUInt32BE(data.length);out.write(kind,4);out.set(data,8);out.writeUInt32BE(crc(out.subarray(4,-4)),out.length-4);return out;}
function png(color){const header=Buffer.alloc(13);header.writeUInt32BE(512);header.writeUInt32BE(512,4);header[8]=8;header[9]=6;const row=Buffer.alloc(2049);for(let x=0;x<512;x++)row.set(color,1+x*4);return new Uint8Array(Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]),chunk('IHDR',header),chunk('IDAT',deflateSync(Buffer.concat(Array(512).fill(row)))),chunk('IEND',Buffer.alloc(0))]));}
const assets=new Map(),expected=new Map();
for(let i=0;i<36;i++){const name=`image-${i}.png`,color=[i*7&255,i*17&255,i*29&255,i%3===0?0:255];assets.set(name,png(color));const pixels=Buffer.alloc(1048576);for(let at=0;at<pixels.length;at+=4)pixels.set(color,at);expected.set(name,createHash('sha256').update(pixels).digest('hex'));}
const exact=[...assets.keys()];
assets.set('fallback.png',assets.get(exact[0]));const rgba='ab7f33ff'.repeat(512*512),precise={'fallback.png':{width:512,height:512,rgba}};expected.set('fallback.png',createHash('sha256').update(Buffer.from(rgba,'hex')).digest('hex'));
const context=vm.createContext({TextEncoder,TextDecoder,Uint8Array,ArrayBuffer,DataView,decoderModule,sourceAssets:assets});
const source=readFileSync(process.argv[2]??new URL('../web-player/worker.js',import.meta.url),'utf8').replace('(await import("./png-exact.js"))','decoderModule');
vm.runInContext(source,context);
vm.runInContext('assets=sourceAssets;instance={exports:{memory:new WebAssembly.Memory({initial:1})}};shaderCount=1;',context);
const worker=vm.runInContext(`({prepare:prepareImages,api,canonical,setName(name){const bytes=new TextEncoder().encode(name);new Uint8Array(instance.exports.memory.buffer).set(bytes);return bytes.length;},reset(){commands=[];imageBudget=0;},state:()=>({commands,imageBudget,bytes:typeof imageCacheBytes==='number'?imageCacheBytes:[...images.values()].reduce((n,v)=>n+v.pixels.length,0),keys:[...images.keys()]}),blockFrame(){commands=[];imageBudget=8388608;}})`,context);
await worker.prepare(precise,exact);
assert.ok(worker.state().bytes<=33554432);
if(typeof decoder.validateExactPNG==='function'){assert.ok(decodes<=32,'Startup pixel decoding must stay within the warm cache allowance');}
// The old first-32 preparation silently loses this real valid package image.
let length=worker.setName(exact.at(-1));worker.reset();assert.equal(worker.api.draw_asset_image(0,length,0,0,1,1),0,'A valid image after the first 32 MiB must remain drawable');assert.equal(validations,36);
for(let round=0;round<2;round++)for(const name of assets.keys()){
 worker.reset();length=worker.setName(name);assert.equal(worker.api.draw_asset_image_linear_straight(0,length,0,0,1,1),0,name);
 const command=worker.state().commands[0];assert.equal(command[0],'straight-linear-image');assert.equal(createHash('sha256').update(command[7]).digest('hex'),expected.get(name),name);
 assert.ok(worker.state().bytes<=33554432);assert.equal(worker.state().imageBudget,1048576);
}
const name=exact[0];assert.ok(!worker.state().keys.includes(name));
const prior=worker.state(),count=decodes;worker.blockFrame();length=worker.setName(name);assert.equal(worker.api.draw_asset_image(0,length,0,0,1,1),-1);assert.equal(decodes,count);assert.deepEqual(worker.state().keys,prior.keys);
worker.reset();assert.equal(worker.api.draw_asset_image(0,length,NaN,0,1,1),-1);assert.equal(decodes,count);
worker.reset();assert.equal(worker.canonical['draw-asset-image-linear'](0,length,0,0,1,1),1);assert.equal(worker.state().commands[0][0],'linear-image');
worker.reset();assert.equal(worker.canonical['draw-shader-asset-image'](0,0,length),1);assert.equal(worker.state().commands[0][0],'shader-image');
// Evict, corrupt the encoded source, and prove a failed reload is not cached.
for(const other of [...exact.slice(1),'fallback.png']){worker.reset();length=worker.setName(other);assert.equal(worker.api.draw_asset_image(0,length,0,0,1,1),0);}
assert.ok(!worker.state().keys.includes(name));const encoded=assets.get(name);encoded[encoded.length-1]^=1;worker.reset();length=worker.setName(name);assert.throws(()=>worker.api.draw_asset_image(0,length,0,0,1,1),/CRC/);assert.ok(!worker.state().keys.includes(name));assert.equal(worker.state().commands.length,0);assert.equal(worker.state().imageBudget,0);
encoded[encoded.length-1]^=1;assert.equal(worker.api.draw_asset_image(0,length,0,0,1,1),0);assert.equal(createHash('sha256').update(worker.state().commands[0][7]).digest('hex'),expected.get(name));
for(const [raw,names] of [[{},['missing.png']],[{},[exact[0],exact[0]]],[precise,[...exact,'fallback.png']],[{'fallback.png':{width:512,height:512,rgba:'zz'+rgba.slice(2)}},exact],[{},['../image-0.png']]])await assert.rejects(worker.prepare(raw,names));
const cold=assets.get(exact.at(-1));cold[cold.length-1]^=1;await assert.rejects(worker.prepare(precise,exact),/CRC/);cold[cold.length-1]^=1;
await worker.prepare(precise,exact);assert.ok(worker.state().bytes<=33554432);
console.log('Bounded startup decoding and 37 MiB of exact/precise PNGs remain drawable across two complete cycles in a 32 MiB LRU cache; exact pixels, hidden RGB, preflight rejection without decode, core/component/shader bindings, corrupt reload/no publication and fresh preparation pass.');
