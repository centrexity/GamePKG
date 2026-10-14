// The guest sees only these typed imports, never JavaScript objects or browser APIs.
let instance, manifest, assets, roms, save, state=0n, commands=[], background=null, unlocks=[],saveDirty=false,saveBudget=0,images=new Map(),imageBudget=0,audioSamples=[],shaderCount=0;
function imageCommand(x,y,w,h,pw,ph,pixels){if(!finite(x,y,w,h)||w<0||h<0||!Number.isInteger(pw)||!Number.isInteger(ph)||pw<1||ph<1||pw>512||ph>512||pixels.length!==pw*ph*4||imageBudget+pixels.length>8388608)return -1;imageBudget+=pixels.length;return command(['image',x,y,w,h,pw,ph,pixels.slice()]);}
async function prepareImages(){images=new Map();let total=0;for(const [name,pixels] of assets){if(!name.toLowerCase().endsWith('.png')||pixels.length>2097152||pixels.length<33)continue;const signature=[137,80,78,71,13,10,26,10];if(!signature.every((byte,index)=>pixels[index]===byte))continue;const header=new DataView(pixels.buffer,pixels.byteOffset,pixels.byteLength),w=header.getUint32(16),h=header.getUint32(20);if(w<1||h<1||w>512||h>512||total+w*h*4>33554432)continue;let bitmap;try{bitmap=await createImageBitmap(new Blob([pixels],{type:'image/png'}));if(bitmap.width!==w||bitmap.height!==h)continue;const surface=new OffscreenCanvas(w,h),context=surface.getContext('2d');context.drawImage(bitmap,0,0);images.set(name,{width:w,height:h,pixels:context.getImageData(0,0,w,h).data});total+=w*h*4;}catch{}finally{bitmap?.close();}}}
function assetName(pointer,length){const value=bytes(pointer,length,1024);if(!value||!length)return null;let name;try{name=new TextDecoder('utf-8',{fatal:true}).decode(value);}catch{return null;}return name&&!name.startsWith('/')&&!name.includes('\\')&&!name.split('/').some(p=>!p||p==='.'||p==='..')&&!/[\x00-\x1f]/.test(name)?name:null;}
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
 draw_image:(x,y,w,h,pw,ph,pointer,length)=>{const pixels=bytes(pointer,length,1048576);return pixels?imageCommand(x,y,w,h,pw,ph,pixels):-1;},
 draw_asset_image:(pointer,length,x,y,w,h)=>{const image=images.get(assetName(pointer,length));return image?imageCommand(x,y,w,h,image.width,image.height,image.pixels):-1;},
 draw_shader:index=>Number.isInteger(index)&&index>=0&&index<shaderCount?command(['shader',index]):-1,
 draw_shader_image:(index,pw,ph,pointer,length)=>{const pixels=bytes(pointer,length,1048576);if(!Number.isInteger(index)||index<0||index>=shaderCount||!pixels||!Number.isInteger(pw)||!Number.isInteger(ph)||pw<1||ph<1||pw>512||ph>512||length!==pw*ph*4||imageBudget+length>8388608)return -1;imageBudget+=length;return command(['shader-image',index,pw,ph,pixels.slice()]);},
 audio_submit:(pointer,count)=>{const samples=bytes(pointer,count*2,3200);if(!manifest.permissions.audio||!Number.isInteger(count)||count<0||count>1600||audioSamples.length+count>1600||!samples)return -1;const view=new DataView(samples.buffer,samples.byteOffset,samples.byteLength);for(let index=0;index<count;index++)audioSamples.push(view.getInt16(index*2,true));return 0;},
 achievement_unlock:index=>{const item=manifest.achievements?.[index];if(!item||unlocks.length>=1024)return 0;unlocks.push(item.id);return 1;}
};
function decode(hex){if(typeof hex!=='string'||hex.length%2||!/^[a-f0-9]*$/.test(hex))throw Error('Invalid packaged bytes');return Uint8Array.from(hex.match(/../g)||[],byte=>parseInt(byte,16));}
onmessage=async ({data})=>{try{
 if(data.type==='start'){
  shaderCount=data.shaderCount||0;manifest=data.manifest;assets=new Map(Object.entries(data.assets).map(([key,value])=>[key,decode(value)]));roms=data.roms.map(value=>new Uint8Array(value));save=new Uint8Array(data.save);state=0n;
  await prepareImages();
  const module=await WebAssembly.compile(data.wasm);
  const namespace=manifest.runtime_profile==='component-v1'?'gamepkg:engine/host@0.1.0':'gamepkg',imports=manifest.runtime_profile==='component-v1'?canonical:api;
  for(const item of WebAssembly.Module.imports(module))if(item.module!==namespace||item.kind!=='function'||!Object.hasOwn(imports,item.name))throw Error('Forbidden host import');
  instance=await WebAssembly.instantiate(module,{[namespace]:imports});
  if(typeof instance.exports[manifest.runtime_profile==='component-v1'?'tick':'gamepkg_tick']!=='function')throw Error('Missing game entry');
  postMessage({type:'ready'});
 }else if(data.type==='tick'){
  commands=[];background=null;unlocks=[];saveDirty=false;saveBudget=0;imageBudget=0;audioSamples=[];
  state=instance.exports[manifest.runtime_profile==='component-v1'?'tick':'gamepkg_tick'](manifest.permissions.input?data.input:0,state);
  if(instance.exports.memory&&instance.exports.memory.buffer.byteLength>manifest.limits.hard_memory_bytes)throw Error('Memory limit exceeded');
  if(saveDirty)postMessage({type:'save',bytes:save});
  postMessage({type:'frame',commands,background,state:String(state),unlocks,audio:audioSamples});
 }
}catch(error){postMessage({type:'error',error:String(error)});}};

// Canonical list results are allocated in guest memory through its bounded
// allocator. Guest bindings copy and free these results immediately.
function optionSize(pointer,size){if(!range(pointer,8)||pointer%4)throw Error('Invalid result pointer');const view=new DataView(instance.exports.memory.buffer);view.setUint32(pointer,size<0?0:1,true);view.setUint32(pointer+4,size<0?0:size,true);}
function optionBytes(pointer,source,offset,length){
 if(!range(pointer,12)||pointer%4)throw Error('Invalid result pointer');
 let part=null,address=0;
 if(source&&Number.isInteger(offset)&&offset>=0&&offset<=source.length&&Number.isInteger(length)&&length>=0&&length<=65536){part=source.subarray(offset,offset+length);if(part.length){address=instance.exports.cabi_realloc(0,0,1,part.length)>>>0;if(!range(address,part.length))throw Error('Invalid allocation');bytes(address,part.length).set(part);}}
 const view=new DataView(instance.exports.memory.buffer);view.setUint32(pointer,part?1:0,true);view.setUint32(pointer+4,address,true);view.setUint32(pointer+8,part?.length??0,true);
}
const canonical={
 'rom-count':api.rom_count,
 'rom-size':pointer=>optionSize(pointer,api.rom_size()),
 'rom-size-at':(index,pointer)=>optionSize(pointer,api.rom_size_at(index)),
 'rom-read':(offset,length,pointer)=>optionBytes(pointer,roms[0],offset,length),
 'rom-read-at':(index,offset,length,pointer)=>optionBytes(pointer,roms[index],offset,length),
 'save-size':pointer=>optionSize(pointer,api.save_size()),
 'save-read':(offset,length,pointer)=>optionBytes(pointer,manifest.permissions.save?save:null,offset,length),
 'save-write':(pointer,length)=>api.save_write(pointer,length)===0?1:0,
 'asset-read':(pointer,length,offset,count,result)=>{let source=null;const encoded=bytes(pointer,length,1024);if(encoded){let name;try{name=new TextDecoder('utf-8',{fatal:true}).decode(encoded);}catch{}if(name&&!name.startsWith('/')&&!name.includes('\\')&&!name.split('/').some(p=>!p||p==='.'||p==='..')&&!/[\x00-\x1f]/.test(name))source=assets.get(name);}optionBytes(result,source,offset,count);},
 'achievement-unlock':api.achievement_unlock,
 'draw-clear':rgba=>api.draw_clear(rgba)===0?1:0,
 'draw-rect':(...args)=>api.draw_rect(...args)===0?1:0,
 'draw-circle':(...args)=>api.draw_circle(...args)===0?1:0,
 'draw-image':(...args)=>api.draw_image(...args)===0?1:0,
 'draw-asset-image':(...args)=>api.draw_asset_image(...args)===0?1:0,
 'audio-submit':(...args)=>api.audio_submit(...args)===0?1:0,
 'draw-shader':index=>api.draw_shader(index)===0?1:0,
 'draw-shader-image':(...args)=>api.draw_shader_image(...args)===0?1:0
};
