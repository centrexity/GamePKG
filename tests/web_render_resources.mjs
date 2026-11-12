import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const inflaterURL='data:text/javascript;base64,'+readFileSync(new URL('../web-player/png-inflate.js',import.meta.url)).toString('base64');
const source=readFileSync(new URL('../web-player/asset-store.js',import.meta.url),'utf8').replace("'./png-inflate.js'",JSON.stringify(inflaterURL));
const {renderResources,assetPlan,decodeAssetHex,PackageAssets}=await import('data:text/javascript;base64,'+Buffer.from(source).toString('base64'));
const MiB=1048576,large={limits:{render_profile:'large-images-v1',hard_memory_bytes:256*MiB,recommended_memory_bytes:256*MiB}};
assert.deepEqual(renderResources(),{frameBytes:8*MiB,cacheBytes:32*MiB,assetBytes:32*MiB,fileBytes:32*MiB});
assert.deepEqual(renderResources(large),{frameBytes:32*MiB,cacheBytes:128*MiB,assetBytes:128*MiB,fileBytes:64*MiB});
assert.ok(Object.isFrozen(renderResources(large)));
for(const invalid of [
 {limits:{render_profile:'unlimited',hard_memory_bytes:256*MiB}},
 {limits:{render_profile:'large-images-v2',hard_memory_bytes:256*MiB}},
 {...large,builtin_engine:'zelda3-45a149d8'}, {...large,builtin_engine:''}, {...large,builtin_engine:false},
 {limits:{...large.limits,hard_memory_bytes:64*MiB}},
 {limits:{...large.limits,hard_memory_bytes:1024*MiB}},
 {limits:{...large.limits,hard_memory_bytes:'268435456'}},
 {limits:{...large.limits,recommended_memory_bytes:64*MiB}},
 {limits:{...large.limits,recommended_memory_bytes:512*MiB}},
])assert.throws(()=>renderResources(invalid));
const hex='ab'.repeat(16*MiB),standard={a:hex,b:hex},expanded=Object.fromEntries(Array.from({length:8},(_,i)=>['file-'+i,hex]));
assert.equal(assetPlan(standard).encodedBytes,32*MiB);assert.throws(()=>assetPlan({...standard,extra:'00'}));
assert.equal(assetPlan(expanded,large).encodedBytes,128*MiB);assert.throws(()=>assetPlan({...expanded,extra:'00'},large));assert.throws(()=>assetPlan(expanded));
const excessive='00'.repeat(65*MiB);assert.throws(()=>decodeAssetHex(excessive,large));assert.throws(()=>assetPlan({excessive},large));
assert.deepEqual([...decodeAssetHex('abcd',large)],[171,205]);const assets=new PackageAssets({small:'abcd'},large);assert.equal(assets.encodedBytes,2);assert.equal(assets.allocatedScratchBytes,0);await assets.close();
const context=vm.createContext({TextEncoder,TextDecoder});vm.runInContext(readFileSync(new URL('../web-player/worker.js',import.meta.url),'utf8'),context);
const worker=vm.runInContext(`({select:bytes=>frameImageLimit=bytes,setup:()=>{instance={exports:{memory:new WebAssembly.Memory({initial:17})}};new Uint8Array(instance.exports.memory.buffer).set(new TextEncoder().encode('picture.png'));images.set('picture.png',{width:512,height:512,pixels:new Uint8Array(1048576),precise:true});shaderCount=1;},reset:()=>{commands=[];imageBudget=0;},api,canonical,state:()=>({commands:commands.length,imageBudget})})`,context);
worker.setup();
for(const [policy,expected] of [[renderResources(),8],[renderResources(large),32]]){
 worker.select(policy.frameBytes);worker.reset();
 for(let index=0;index<expected;index++)assert.equal(worker.api.draw_asset_image(0,11,0,0,1,1),0);
 assert.equal(worker.api.draw_asset_image_linear_straight(0,11,0,0,1,1),-1);assert.equal(worker.api.draw_shader_asset_image(0,0,11),-1);assert.equal(worker.api.draw_image_linear(0,0,1,1,512,512,0,1048576),-1);assert.equal(worker.state().imageBudget,expected*MiB);
 worker.reset();for(let index=0;index<expected;index++)assert.equal(worker.canonical['draw-shader-asset-image'](0,0,11),1);assert.equal(worker.canonical['draw-asset-image'](0,11,0,0,1,1),0);
}
console.log('Signed render presets: unchanged defaults, strict source/memory/profile checks, exact32/128MiB aggregate and64MiB file ceilings, and independent core/component/raw/shader frame boundaries pass.');
