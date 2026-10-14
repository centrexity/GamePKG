// The guest sees only these typed imports, never JavaScript objects or browser APIs.
let instance, manifest, assets, roms, save, state=0n, commands=[], background=null, unlocks=[],saveDirty=false,saveBudget=0;
const bad=()=>-1;
function range(pointer,length,max=65536){return Number.isInteger(pointer)&&Number.isInteger(length)&&pointer>=0&&length>=0&&length<=max&&instance?.exports.memory&&pointer+length<=instance.exports.memory.buffer.byteLength;}
function bytes(pointer,length,max){if(!range(pointer,length,max))return null;return new Uint8Array(instance.exports.memory.buffer,pointer,length);}
function read(source,offset,pointer,length){if(!source||offset<0||offset>source.length||!range(pointer,length))return -1;const part=source.subarray(offset,offset+length);bytes(pointer,part.length).set(part);return part.length;}
function finite(...values){return values.every(Number.isFinite)&&values.every(v=>Math.abs(v)<=16384);}
function command(value){if(commands.length>=1024)return -1;commands.push(value);return 0;}
const api={
 asset_read:(p,n,offset,out,length)=>{const name=bytes(p,n,1024);if(!name||n===0)return -1;let path;try{path=new TextDecoder('utf-8',{fatal:true}).decode(name);}catch{return -1;}if(!path||path.startsWith('/')||path.includes('\\')||path.split('/').some(p=>!p||p==='.'||p==='..')||/[\x00-\x1f]/.test(path))return -1;return read(assets.get(path),offset,out,length);},
 rom_count:()=>roms.length,rom_size:()=>roms[0]?.length??-1,rom_size_at:index=>roms[index]?.length??-1,
 rom_read:(offset,out,length)=>read(roms[0],offset,out,length),rom_read_at:(index,offset,out,length)=>read(roms[index],offset,out,length),
 save_size:()=>manifest.permissions.save?save.length:-1,
 save_read:(offset,out,length)=>manifest.permissions.save?read(save,offset,out,length):-1,
 save_write:(pointer,length)=>{const data=bytes(pointer,length,1048576);if(!manifest.permissions.save||!data)return -1;if(saveBudget+length>1048576)return -1;saveBudget+=length;save=data.slice();saveDirty=true;return 0;},
 draw_clear:rgba=>{background=rgba>>>0;return 0;},
 draw_rect:(x,y,w,h,rgba)=>finite(x,y,w,h)&&w>=0&&h>=0?command(['rect',x,y,w,h,rgba>>>0]):-1,
 draw_circle:(x,y,r,rgba)=>finite(x,y,r)&&r>=0?command(['circle',x,y,r,rgba>>>0]):-1,
 achievement_unlock:index=>{const item=manifest.achievements?.[index];if(!item||unlocks.length>=1024)return 0;unlocks.push(item.id);return 1;}
};
function decode(hex){if(typeof hex!=='string'||hex.length%2||!/^[a-f0-9]*$/.test(hex))throw Error('Invalid packaged bytes');return Uint8Array.from(hex.match(/../g)||[],byte=>parseInt(byte,16));}
onmessage=async ({data})=>{try{
 if(data.type==='start'){
  manifest=data.manifest;assets=new Map(Object.entries(data.assets).map(([key,value])=>[key,decode(value)]));roms=data.roms.map(value=>new Uint8Array(value));save=new Uint8Array(data.save);state=0n;
  const module=await WebAssembly.compile(data.wasm);
  for(const item of WebAssembly.Module.imports(module))if(item.module!=='gamepkg'||item.kind!=='function'||!Object.hasOwn(api,item.name))throw Error('Forbidden host import');
  instance=await WebAssembly.instantiate(module,{gamepkg:api});
  if(typeof instance.exports.gamepkg_tick!=='function')throw Error('Missing game entry');
  postMessage({type:'ready'});
 }else if(data.type==='tick'){
  commands=[];background=null;unlocks=[];saveDirty=false;saveBudget=0;
  state=instance.exports.gamepkg_tick(manifest.permissions.input?data.input:0,state);
  if(instance.exports.memory&&instance.exports.memory.buffer.byteLength>manifest.limits.hard_memory_bytes)throw Error('Memory limit exceeded');
  if(saveDirty)postMessage({type:'save',bytes:save});
  postMessage({type:'frame',commands,background,state:String(state),unlocks});
 }
}catch(error){postMessage({type:'error',error:String(error)});}};
