// Trusted package bytes: compressed WAVs borrow one bounded reusable workspace.
import {AssetInflater,INFLATE_SCRATCH_BYTES,MAX_INFLATED_BYTES} from './png-inflate.js';
const CREATE=Symbol();
const STANDARD_RENDER=Object.freeze({frameBytes:8388608,cacheBytes:33554432,assetBytes:33554432,fileBytes:33554432});
const LARGE_RENDER=Object.freeze({frameBytes:33554432,cacheBytes:134217728,assetBytes:134217728,fileBytes:67108864});
export function renderResources(manifest=null){
 const profile=manifest?.limits?.render_profile??null;
 if(profile===null)return STANDARD_RENDER;
 if(profile!=='large-images-v1'||(manifest.builtin_engine!==undefined&&manifest.builtin_engine!==null)||!Number.isSafeInteger(manifest.limits.hard_memory_bytes)||manifest.limits.hard_memory_bytes<268435456||manifest.limits.hard_memory_bytes>536870912)throw Error('Invalid render profile or insufficient source-game memory');
 const recommended=manifest.limits.recommended_memory_bytes;
 if(recommended!==undefined&&recommended!==null&&(!Number.isSafeInteger(recommended)||recommended<268435456||recommended>manifest.limits.hard_memory_bytes))throw Error('Invalid recommended render memory');
 return LARGE_RENDER;
}
export function decodeAssetHex(hex,manifest=null){
 const limit=renderResources(manifest).fileBytes;
 if(typeof hex!=='string'||hex.length%2||hex.length/2>limit||!/^[a-f0-9]*$/.test(hex))throw Error('Invalid packaged asset bytes');
 const output=new Uint8Array(hex.length/2);for(let i=0;i<output.length;i++){const a=hex.charCodeAt(i*2),b=hex.charCodeAt(i*2+1);output[i]=((a<=57?a-48:a-87)<<4)|(b<=57?b-48:b-87);}return output;
}
export function assetPlan(entries,manifest=null){
 const resources=renderResources(manifest);
 if(!entries||typeof entries!=='object'||Array.isArray(entries))throw Error('Invalid package asset map');
 let total=0,maximum=0;
 for(const [path,value] of Object.entries(entries)){
  let hex=value;
  if(typeof value!=='string'){
   if(!value||Array.isArray(value)||Object.keys(value).sort().join(',')!=='length,zlib'||!path.endsWith('.wav')||!Number.isInteger(value.length)||value.length<48||value.length>MAX_INFLATED_BYTES)throw Error('Invalid compressed WAV descriptor');
   hex=value.zlib;if(typeof hex!=='string'||hex.length<12||hex.length>4194304)throw Error('Invalid compressed WAV input');maximum=Math.max(maximum,value.length);
  }
  if(typeof hex!=='string'||hex.length%2||hex.length/2>resources.fileBytes||(total+=hex.length/2)>resources.assetBytes||!/^[a-f0-9]*$/.test(hex))throw Error('Invalid or oversized package assets');
 }
 return Object.freeze({encodedBytes:total,maximumDecodedBytes:maximum,scratchBytes:maximum?maximum+INFLATE_SCRATCH_BYTES:0});
}
export class CompressedAsset {
 #owner;#encoded;#length;
 constructor(token,owner,encoded,length){if(token!==CREATE)throw Error('Use PackageAssets');this.#owner=owner;this.#encoded=encoded;this.#length=length;}
 get length(){return this.#length;}
 borrow(use){return this.#owner.borrow(CREATE,this.#encoded,this.#length,use);}
}
export class PackageAssets extends Map {
 #maximum;#scratch;#encodedBytes;#workspace=null;#busy=false;#pending=null;#closed=false;#closing=null;#lastEncoded=null;#decoded=null;
 constructor(entries,manifest=null){
  super();const plan=assetPlan(entries,manifest);this.#maximum=plan.maximumDecodedBytes;this.#scratch=plan.scratchBytes;this.#encodedBytes=plan.encodedBytes;
  for(const [path,value] of Object.entries(entries)){const encoded=decodeAssetHex(typeof value==='string'?value:value.zlib,manifest);super.set(path,typeof value==='string'?encoded:new CompressedAsset(CREATE,this,encoded,value.length));}
 }
 set(){throw Error("Package asset entries are immutable");}
 delete(){throw Error("Package asset entries are immutable");}
 clear(){throw Error("Package asset entries are immutable");}
 get encodedBytes(){return this.#encodedBytes;}
 get scratchBytes(){return this.#scratch;}
 get allocatedScratchBytes(){return this.#workspace?.reservedBytes??0;}
 get closed(){return this.#closed;}
 borrow(token,encoded,length,use){
  if(token!==CREATE||this.#closed||this.#busy||typeof use!=='function')throw Error('Package asset decoder unavailable');
  this.#busy=true;
  try{
   if(!this.#workspace)this.#workspace=new AssetInflater(this.#maximum);
   if(this.#lastEncoded!==encoded){this.#lastEncoded=null;this.#decoded=null;this.#decoded=this.#workspace.decode(encoded,length);this.#lastEncoded=encoded;}
   const result=use(this.#decoded);
   if(result&&typeof result.then==='function'){const pending=Promise.resolve(result).finally(()=>{this.#busy=false;this.#pending=null;});this.#pending=pending;return pending;}
   this.#busy=false;return result;
  }catch(error){this.#busy=false;throw error;}
 }
 close(){
  if(this.#closing)return this.#closing;this.#closed=true;
  this.#closing=Promise.resolve(this.#pending).catch(()=>{}).then(()=>{this.#workspace?.close();this.#workspace=null;this.#lastEncoded=null;this.#decoded=null;super.clear();});return this.#closing;
 }
}
