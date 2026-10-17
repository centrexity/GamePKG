let eventBytes=new Uint8Array();
// The guest sees only these typed imports, never JavaScript objects or browser APIs.
let instance, manifest, assets, roms, save, state=0n, commands=[], background=null, unlocks=[],saveDirty=false,saveBudget=0,images=new Map(),imageBudget=0,audioSamples=[],shaderCount=0;
function imageCommand(x,y,w,h,pw,ph,pixels){if(!finite(x,y,w,h)||w<0||h<0||!Number.isInteger(pw)||!Number.isInteger(ph)||pw<1||ph<1||pw>512||ph>512||pixels.length!==pw*ph*4||imageBudget+pixels.length>8388608)return -1;imageBudget+=pixels.length;return command(['image',x,y,w,h,pw,ph,pixels.slice()]);}
async function prepareImages(){images=new Map();let total=0;for(const [name,pixels] of assets){if(!name.toLowerCase().endsWith('.png')||pixels.length>2097152||pixels.length<33)continue;const signature=[137,80,78,71,13,10,26,10];if(!signature.every((byte,index)=>pixels[index]===byte))continue;const header=new DataView(pixels.buffer,pixels.byteOffset,pixels.byteLength),w=header.getUint32(16),h=header.getUint32(20);if(w<1||h<1||w>512||h>512||total+w*h*4>33554432)continue;let bitmap;try{bitmap=await createImageBitmap(new Blob([pixels],{type:'image/png'}));if(bitmap.width!==w||bitmap.height!==h)continue;const surface=new OffscreenCanvas(w,h),context=surface.getContext('2d');context.drawImage(bitmap,0,0);images.set(name,{width:w,height:h,pixels:context.getImageData(0,0,w,h).data});total+=w*h*4;}catch{}finally{bitmap?.close();}}}
function validAssetPath(name){
 if(!name||new TextEncoder().encode(name).length>1024||name.split('/').length>32||name.startsWith('/')||/[\\:<>"|?*\x00-\x1f\x7f-\x9f]/.test(name))return false;
 return name.split('/').every(part=>{
  if(!part||new TextEncoder().encode(part).length>255||/[. ]$/.test(part))return false;
  const base=part.split('.')[0].replace(/ +$/,'').toUpperCase();
  return !/^(CON|PRN|AUX|NUL|CONIN\$|CONOUT\$|COM[1-9¹²³]|LPT[1-9¹²³])$/.test(base);
 });
}
function assetName(pointer,length){const value=bytes(pointer,length,1024);if(!value||!length)return null;let name;try{name=new TextDecoder('utf-8',{fatal:true}).decode(value);}catch{return null;}return validAssetPath(name)?name:null;}
function range(pointer,length,max=65536){return Number.isInteger(pointer)&&Number.isInteger(length)&&pointer>=0&&length>=0&&length<=max&&instance?.exports.memory&&pointer+length<=instance.exports.memory.buffer.byteLength;}
function bytes(pointer,length,max){if(!range(pointer,length,max))return null;return new Uint8Array(instance.exports.memory.buffer,pointer,length);}
function read(source,offset,pointer,length){if(!source||offset<0||offset>source.length||!range(pointer,length))return -1;const part=source.subarray(offset,offset+length);bytes(pointer,part.length).set(part);return part.length;}
function finite(...values){return values.every(Number.isFinite)&&values.every(v=>Math.abs(v)<=16384);}
function command(value){if(commands.length>=1024)return -1;commands.push(value);return 0;}
function takeEvents(capacity){if(!manifest.permissions.input||!Number.isInteger(capacity)||capacity<=0||capacity>32768||capacity%16)return null;const result=eventBytes.slice(0,capacity);eventBytes=eventBytes.slice(result.length);return result;}
function encodeEvents(events){
 if(!Array.isArray(events)||events.length>2048)return new Uint8Array();
 const output=new Uint8Array(events.length*16),view=new DataView(output.buffer);
 for(let i=0;i<events.length;i++){
  const event=events[i];if(!Array.isArray(event)||event.length!==4)return new Uint8Array();
  const [kind,x,y,code]=event;if(!Number.isInteger(kind)||!Number.isInteger(code)||code<0||code>0xffffffff||!Number.isFinite(x)||!Number.isFinite(y))return new Uint8Array();
  const valid=(kind===0||kind===8)?x===0&&y===0&&code===0:kind===1?x>=0&&x<=640&&y>=0&&y<=480&&code<=31:(kind===2||kind===3)?x>=0&&x<=640&&y>=0&&y<=480&&[1,2,4,8,16].includes(code):kind===4?Math.abs(x)<=4096&&Math.abs(y)<=4096&&code<=2:kind===5?x===0&&y===0&&(code&~0x101ff)===0&&(code&65535)>0&&(code&65535)<512:kind===6?x===0&&y===0&&code>0&&code<512:kind===7?x===0&&y===0&&code>0&&code<=0x10ffff&&(code<0xd800||code>0xdfff):(kind===9||kind===10)?x===0&&y===0&&(kind===9||code<0x80000000)&&((code&0x40000000)?(code&~0xc00001ff)===0&&(code&511)!==0:(code&0x7fffffff)>0&&(code&0x7fffffff)<=0x10ffff&&((code&0x7fffffff)<0xd800||(code&0x7fffffff)>0xdfff)):false;
  if(!valid)return new Uint8Array();view.setUint32(i*16,kind,true);view.setFloat32(i*16+4,x,true);view.setFloat32(i*16+8,y,true);view.setUint32(i*16+12,code,true);
 }return output;
}
// Transactional commands for the player's own window; no DOM/device handles.
function defaultWindow(){return {visible:true,confined:false,fullscreen_toggle:false,closed:false};}
let windowCurrent=defaultWindow(),windowPrevious=defaultWindow(),windowStaged=defaultWindow(),windowGesture=false;
function windowBegin(gesture){windowPrevious={...windowCurrent};windowStaged={...windowCurrent};windowGesture=gesture;}
function windowRequest(operation,value){
 if(!manifest.permissions.input||windowStaged.closed||!Number.isInteger(operation)||!Number.isInteger(value))return 0;
 if(operation===0&&(value===0||value===1))windowStaged.visible=value===1;
 else if(operation===1&&(value===0||value===1))windowStaged.confined=value===1;
 else if(operation===2&&value===0&&windowGesture)windowStaged.fullscreen_toggle=!windowStaged.fullscreen_toggle;
 else if(operation===3&&value===0&&windowGesture)windowStaged.closed=true;
 else return 0;return 1;
}
function windowFinish(success){windowCurrent=success?{...windowStaged}:{...windowPrevious};windowGesture=false;return {...windowCurrent};}

// Player-selected file service; no worker filesystem or DOM access.
let fileEntry=null,fileNext=1,fileGeneration=0,fileGesture=false;
function fileNameAllowed(name,policy){if(typeof name!=='string'||!name||new TextEncoder().encode(name).length>255||/[. ]$/.test(name)||/[<>:"/\\|?*\x00-\x1f\x7f-\x9f]/.test(name))return false;const stem=name.split('.')[0].replace(/ +$/,'').toUpperCase();if(/^(CON|PRN|AUX|NUL|CONIN\$|CONOUT\$|COM[1-9¹²³]|LPT[1-9¹²³])$/.test(stem))return false;const split=name.lastIndexOf('.');return split>=0&&policy.extensions.includes(name.slice(split+1).toLowerCase());}
function filePolicy(index){
 const policies=manifest.permissions.files??[];if(!Array.isArray(policies)||policies.length>32||!Number.isInteger(index)||index<0||index>=policies.length)return null;
 const ids=new Set();for(const p of policies){
  if(!p||Object.keys(p).some(key=>!['id','title','extensions','max_bytes','read','write'].includes(key))||typeof p.id!=='string'||!/^[A-Za-z0-9_-]{1,64}$/.test(p.id)||ids.has(p.id)||typeof p.title!=='string'||!p.title.trim()||new TextEncoder().encode(p.title).length>128||/[\x00-\x1f\x7f-\x9f]/.test(p.title)||(!p.read&&!p.write)||(p.read!==undefined&&typeof p.read!=='boolean')||(p.write!==undefined&&typeof p.write!=='boolean')||!Number.isSafeInteger(p.max_bytes)||p.max_bytes<1||p.max_bytes>67108864||(p.write&&p.max_bytes>1048576)||!Array.isArray(p.extensions)||!p.extensions.length||p.extensions.length>8||p.extensions.some(e=>typeof e!=='string'||!/^[a-z0-9]{1,16}$/.test(e))||new Set(p.extensions).size!==p.extensions.length)return null;
  ids.add(p.id);
 }return policies[index];
}
function fileCall(operation,request,capacity){
 if(!Number.isInteger(operation)||operation<0||operation>5||!Number.isInteger(capacity)||capacity<0||capacity>65536||request.length>1048840)return null;
 const view=new DataView(request.buffer,request.byteOffset,request.byteLength),number=at=>at+4<=request.length?view.getUint32(at,true):null;
 if(operation<=1){
  if(!manifest.permissions.input||!fileGesture||fileEntry||capacity<4||fileNext>=0xffffffff)return null;const index=number(0),policy=filePolicy(index);if(!policy)return null;
  let name='',payload=new Uint8Array();
  if(operation===0){if(request.length!==4||!policy.read)return null;}else{
   const length=number(4);if(!policy.write||length===null||length>256||8+length>request.length)return null;
   try{name=new TextDecoder('utf-8',{fatal:true}).decode(request.subarray(8,8+length));}catch{return null;}
   payload=request.slice(8+length);if(!fileNameAllowed(name,policy)||payload.length>1048576||payload.length>policy.max_bytes)return null;
  }
  fileEntry={handle:fileNext++,generation:fileGeneration,index,policy,operation,name,bytes:payload,staged:true,status:1};fileGesture=false;const result=new Uint8Array(4);new DataView(result.buffer).setUint32(0,fileEntry.handle,true);return result;
 }
 const entry=fileEntry;if(!entry||number(0)!==entry.handle)return null;
 if(operation===2&&request.length===4&&capacity>=12){const result=new Uint8Array(12),v=new DataView(result.buffer);v.setUint32(0,entry.status,true);v.setUint32(4,entry.status===2?entry.bytes.length:0,true);v.setUint32(8,entry.status===2?new TextEncoder().encode(entry.name).length:0,true);return result;}
 if(operation===3&&request.length===4&&entry.status===2){const name=new TextEncoder().encode(entry.name);return capacity>=name.length?name:null;}
 if(operation===4&&request.length===8&&entry.status===2){const offset=number(4);return offset<=entry.bytes.length?entry.bytes.slice(offset,offset+capacity):null;}
 if(operation===5&&request.length===4&&capacity===0&&entry.status!==1){fileEntry=null;return new Uint8Array();}return null;
}
function fileResult(data){const entry=fileEntry;if(!entry||entry.status!==1||entry.staged||data.handle!==entry.handle||data.generation!==entry.generation)return;
 if(data.status===4||data.status===5){entry.status=data.status;return;}
 if(entry.operation===1&&data.status===3){entry.status=3;entry.bytes=new Uint8Array();return;}
 if(entry.operation===0&&data.status===2&&data.bytes instanceof Uint8Array&&data.bytes.length<=entry.policy.max_bytes&&data.bytes.length<=67108864&&fileNameAllowed(data.name,entry.policy)){entry.name=data.name;entry.bytes=data.bytes;entry.status=2;return;}entry.status=5;
}
function fileTickBegin(input=0){fileGesture=false;if(manifest.permissions.input){fileGesture=(input&0x000f00ff)!==0;const v=new DataView(eventBytes.buffer,eventBytes.byteOffset,eventBytes.byteLength);for(let at=0;at<eventBytes.length;at+=16){const kind=v.getUint32(at,true),code=v.getUint32(at+12,true);if(kind===2||(kind===5&&!(code&65536))||(kind===9&&!(code&0x80000000)))fileGesture=true;}}}
function fileTickFinish(success){fileGesture=false;if(fileEntry?.staged){if(!success){fileEntry=null;return;}fileEntry.staged=false;postMessage({type:'file-request',handle:fileEntry.handle,generation:fileEntry.generation,index:fileEntry.index,operation:fileEntry.operation,name:fileEntry.name,bytes:fileEntry.operation===1?fileEntry.bytes:new Uint8Array()});}}

const api={
 window_control:windowRequest,
 file_call:(operation,pointer,length,output,capacity)=>{const request=bytes(pointer,length,1048840),out=bytes(output,capacity,65536);if(!request||!out)return -1;const reply=fileCall(operation,request.slice(),capacity);if(!reply)return -1;out.set(reply);return reply.length;},
 input_events:(pointer,capacity)=>{const output=bytes(pointer,capacity,32768);if(!output)return -1;const events=takeEvents(capacity);if(!events)return -1;output.set(events);return events.length;},
 random_u64:()=>{const words=new Uint32Array(2);crypto.getRandomValues(words);return BigInt(words[0])|(BigInt(words[1])<<32n);},
 asset_read:(p,n,offset,out,length)=>{const name=assetName(p,n);return name===null?-1:read(assets.get(name),offset,out,length);},
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
 draw_shader_asset_image:(index,pointer,length)=>{const image=images.get(assetName(pointer,length));if(!Number.isInteger(index)||index<0||index>=shaderCount||!image||imageBudget+image.pixels.length>8388608||commands.length>=1024)return -1;imageBudget+=image.pixels.length;return command(['shader-image',index,image.width,image.height,image.pixels.slice()]);},
 draw_shader_image:(index,pw,ph,pointer,length)=>{const pixels=bytes(pointer,length,1048576);if(!Number.isInteger(index)||index<0||index>=shaderCount||!pixels||!Number.isInteger(pw)||!Number.isInteger(ph)||pw<1||ph<1||pw>512||ph>512||length!==pw*ph*4||imageBudget+length>8388608)return -1;imageBudget+=length;return command(['shader-image',index,pw,ph,pixels.slice()]);},
 audio_submit:(pointer,count)=>{const samples=bytes(pointer,count*2,3200);if(!manifest.permissions.audio||!Number.isInteger(count)||count<0||count>1600||audioSamples.length+count>1600||!samples)return -1;const view=new DataView(samples.buffer,samples.byteOffset,samples.byteLength);for(let index=0;index<count;index++)audioSamples.push(view.getInt16(index*2,true));return 0;},
 achievement_unlock:index=>{const item=manifest.achievements?.[index];if(!item||unlocks.length>=1024)return 0;unlocks.push(item.id);return 1;}
};
function decode(hex){if(typeof hex!=='string'||hex.length%2||!/^[a-f0-9]*$/.test(hex))throw Error('Invalid packaged bytes');return Uint8Array.from(hex.match(/../g)||[],byte=>parseInt(byte,16));}
onmessage=async ({data})=>{try{
 if(data.type==='file-result'){fileResult(data);return;}
 if(data.type==='start'){
  fileEntry=null;fileNext=1;fileGeneration++;fileGesture=false;windowCurrent=defaultWindow();windowPrevious=defaultWindow();windowStaged=defaultWindow();
  shaderCount=data.shaderCount||0;manifest=data.manifest;assets=new Map(Object.entries(data.assets).map(([key,value])=>[key,decode(value)]));roms=data.roms.map(value=>new Uint8Array(value));save=new Uint8Array(data.save);state=0n;
  await prepareImages();
  const module=await WebAssembly.compile(data.wasm);
  const namespace=manifest.runtime_profile==='component-v1'?'gamepkg:engine/host@0.1.0':'gamepkg',imports=manifest.runtime_profile==='component-v1'?canonical:api;
  for(const item of WebAssembly.Module.imports(module))if(item.module!==namespace||item.kind!=='function'||!Object.hasOwn(imports,item.name))throw Error('Forbidden host import');
  instance=await WebAssembly.instantiate(module,{[namespace]:imports});
  if(typeof instance.exports[manifest.runtime_profile==='component-v1'?'tick':'gamepkg_tick']!=='function')throw Error('Missing game entry');
  postMessage({type:'ready'});
 }else if(data.type==='restart'){
  eventBytes=new Uint8Array();state=0n;fileEntry=null;fileGeneration++;fileGesture=false;windowCurrent=defaultWindow();windowPrevious=defaultWindow();windowStaged=defaultWindow();
 }else if(data.type==='tick'){
  eventBytes=manifest.permissions.input?encodeEvents(data.events??[]):new Uint8Array();
  commands=[];background=null;unlocks=[];saveDirty=false;saveBudget=0;imageBudget=0;audioSamples=[];fileTickBegin(data.input);windowBegin(fileGesture);
  state=instance.exports[manifest.runtime_profile==='component-v1'?'tick':'gamepkg_tick'](manifest.permissions.input?data.input:0,state);
  if(instance.exports.memory&&instance.exports.memory.buffer.byteLength>manifest.limits.hard_memory_bytes)throw Error('Memory limit exceeded');
  const windowState=windowFinish(true);fileTickFinish(!windowState.closed);
  if(saveDirty)postMessage({type:'save',bytes:save});
  postMessage({type:'frame',commands,background,state:String(state),unlocks,audio:audioSamples,window:windowState});
 }
}catch(error){windowFinish(false);fileTickFinish(false);postMessage({type:'error',error:String(error)});}};

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
 'window-control':windowRequest,
 'file-call':(operation,pointer,length,capacity,result)=>{if(!range(result,12)||result%4)throw Error('Invalid result pointer');const request=bytes(pointer,length,1048840);let reply=null;if(request&&Number.isInteger(capacity)&&capacity>=0&&capacity<=65536)reply=fileCall(operation,request.slice(),capacity);let address=0;if(reply?.length){address=instance.exports.cabi_realloc(0,0,1,reply.length)>>>0;if(!range(address,reply.length))throw Error('Invalid allocation');bytes(address,reply.length).set(reply);}const v=new DataView(instance.exports.memory.buffer);v.setInt32(result,reply?reply.length:-1,true);v.setUint32(result+4,address,true);v.setUint32(result+8,reply?.length??0,true);},
 'input-events':(capacity,pointer)=>{if(!range(pointer,12)||pointer%4)throw Error('Invalid result pointer');const events=takeEvents(capacity);optionBytes(pointer,events,0,events?.length??0);},
 'random-u64':api.random_u64,
 'rom-count':api.rom_count,
 'rom-size':pointer=>optionSize(pointer,api.rom_size()),
 'rom-size-at':(index,pointer)=>optionSize(pointer,api.rom_size_at(index)),
 'rom-read':(offset,length,pointer)=>optionBytes(pointer,roms[0],offset,length),
 'rom-read-at':(index,offset,length,pointer)=>optionBytes(pointer,roms[index],offset,length),
 'save-size':pointer=>optionSize(pointer,api.save_size()),
 'save-read':(offset,length,pointer)=>optionBytes(pointer,manifest.permissions.save?save:null,offset,length),
 'save-write':(pointer,length)=>api.save_write(pointer,length)===0?1:0,
 'asset-read':(pointer,length,offset,count,result)=>{const name=assetName(pointer,length);optionBytes(result,name===null?null:assets.get(name),offset,count);},
 'achievement-unlock':api.achievement_unlock,
 'draw-clear':rgba=>api.draw_clear(rgba)===0?1:0,
 'draw-rect':(...args)=>api.draw_rect(...args)===0?1:0,
 'draw-circle':(...args)=>api.draw_circle(...args)===0?1:0,
 'draw-image':(...args)=>api.draw_image(...args)===0?1:0,
 'draw-asset-image':(...args)=>api.draw_asset_image(...args)===0?1:0,
 'audio-submit':(...args)=>api.audio_submit(...args)===0?1:0,
 'draw-shader':index=>api.draw_shader(index)===0?1:0,
 'draw-shader-asset-image':(...args)=>api.draw_shader_asset_image(...args)===0?1:0,
 'draw-shader-image':(...args)=>api.draw_shader_image(...args)===0?1:0
};
