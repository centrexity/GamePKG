import {ProgressStore,ProgressDataError} from './progress-store.js';
import {RecordedBudget} from './recorded-budget.js';
const canvas=document.querySelector('#game'),context=canvas.getContext('2d'),status=document.querySelector('#status'),play=document.querySelector('#play'),stop=document.querySelector('#stop'),restart=document.querySelector('#restart');
let data,wasm,worker,timer,frameTimer,pressed=0,held=0,roms=[],running=false,saveKey,historyKey,lastFrame=0,nextTickAt=0,audioContext,audioGain,audioNext=0,muted=false,audioNodes=new Set(),shaderSurface,gl,programs=[],texture;
let recordedSession=null,recordedOpening=null,recordedAbort=null,audioClosing=Promise.resolve();
let filePending=null,tickPending=false,restartPending=false,progressStore=null,startSerial=0,gameBudget=null,guestMemory=null;
const keys={ArrowLeft:1,a:1,ArrowRight:2,d:2,ArrowUp:4,w:4,ArrowDown:8,s:8,z:16,x:32,Enter:64,Tab:128,c:65536,v:131072,q:262144,e:524288};
const heldKeys=new Map();
function resetKeyboard(){heldKeys.clear();held=pressed=0;}
function keyboardDown(event){
 if((event.ctrlKey||event.metaKey||event.altKey)&&/^[a-z]$/i.test(event.key))return;
 const bit=keys[event.key]??keys[event.key.toLowerCase()];
 if(!bit)return;
 event.preventDefault();
 const id=event.code||event.key.toLowerCase();
 if(event.repeat||heldKeys.has(id))return;
 if(!(held&bit))pressed|=bit;
 heldKeys.set(id,bit);held|=bit;
}
function keyboardUp(event){
 const id=event.code||event.key.toLowerCase();
 if(!heldKeys.has(id))return;
 event.preventDefault();heldKeys.delete(id);held=0;
 for(const bit of heldKeys.values())held|=bit;
}
function color(rgba){return `rgba(${rgba>>>24},${rgba>>>16&255},${rgba>>>8&255},${(rgba&255)/255})`;}
function clearAudio(){for(const node of audioNodes){try{node.stop();}catch{}}audioNodes.clear();audioNext=0;}
function halt(message){startSerial++;progressStore?.cancel();tickPending=false;restartPending=false;resetWindowControls();clearFileUI();clearAudio();clearTimeout(timer);clearTimeout(frameTimer);recordedAbort?.abort();recordedAbort=null;const audio=recordedSession,opening=recordedOpening;recordedSession=null;recordedOpening=null;if(audio||opening){const closingBudget=gameBudget;audioClosing=Promise.all([audioClosing,audio?.close(),opening?.then(value=>value.close(),error=>{if(closingBudget?.used)throw error;})]);void audioClosing.catch(()=>{});if(!audio)worker?.terminate();}else worker?.terminate();worker=null;guestMemory?.release();guestMemory=null;gameBudget?.close();gameBudget=null;running=false;resetKeyboard();surfaceEvents=[];play.disabled=false;stop.disabled=true;restart.disabled=true;status.textContent=message;}
function watch(delay){clearTimeout(timer);const active=worker;timer=setTimeout(()=>{if(worker===active)halt('Game stopped: execution deadline exceeded.');},delay);}
const storageWarning='Browser storage is unavailable; progress may not persist.';
function tick(){
 if(!running||restartPending||tickPending)return;tickPending=true;const active=worker,epoch=homepageEpoch;
 const send=recordedAudio=>{if(!running||worker!==active||restartPending||homepageEpoch!==epoch)return;watch(250);active.postMessage({type:'tick',fullscreen:typeof document.fullscreenElement==='undefined'?null:document.fullscreenElement===canvas,input:pressed|((held&255)<<8)|((held&0x000f0000)<<4),events:surfaceEvents,recordedAudio});surfaceEvents=[];pressed=0;};
 if(recordedSession)recordedSession.snapshot().then(send).catch(error=>{if(running&&worker===active)halt(String(error));});else send(null);
}
function shader(type,source){const value=gl.createShader(type);gl.shaderSource(value,source);gl.compileShader(value);if(!gl.getShaderParameter(value,gl.COMPILE_STATUS)){const error=gl.getShaderInfoLog(value);gl.deleteShader(value);throw Error('WebGL shader compilation failed: '+error);}return value;}
function setupShaders(sources){if(!sources.length)return;shaderSurface=new OffscreenCanvas(640,480);gl=shaderSurface.getContext('webgl2',{alpha:true,depth:false,antialias:false,premultipliedAlpha:false,preserveDrawingBuffer:true});if(!gl)throw Error('This game requires WebGL 2');const vertex=shader(gl.VERTEX_SHADER,'#version 300 es\nvoid main(){vec2 p=vec2((gl_VertexID<<1)&2,gl_VertexID&2);gl_Position=vec4(p*2.0-1.0,0.0,1.0);}');for(const source of sources){const adapted=source.replace(/^#version[^\n]*\n/,'#version 300 es\nprecision highp float;\nprecision highp int;\n').replace(/layout\(row_major\) (uniform|buffer);/g,'').replace(/layout\(binding\s*=\s*0\)\s*/g,'');const fragment=shader(gl.FRAGMENT_SHADER,adapted),program=gl.createProgram();gl.attachShader(program,vertex);gl.attachShader(program,fragment);gl.linkProgram(program);gl.deleteShader(fragment);if(!gl.getProgramParameter(program,gl.LINK_STATUS))throw Error('WebGL shader link failed: '+gl.getProgramInfoLog(program));for(let i=0;i<gl.getProgramParameter(program,gl.ACTIVE_UNIFORMS);i++){const uniform=gl.getActiveUniform(program,i);if(uniform.name!=='gamepkgTexture_0'||uniform.type!==gl.SAMPLER_2D||uniform.size!==1)throw Error('Forbidden shader resource');}programs.push(program);}gl.deleteShader(vertex);texture=gl.createTexture();gl.bindTexture(gl.TEXTURE_2D,texture);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.NEAREST);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.NEAREST);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE);gl.bindVertexArray(gl.createVertexArray());}
function shaderFrame(index,width=1,height=1,pixels=new Uint8Array(4),target=context){if(!programs[index]||gl.isContextLost())throw Error('Shader graphics context unavailable');gl.viewport(0,0,640,480);gl.clearColor(0,0,0,0);gl.clear(gl.COLOR_BUFFER_BIT);gl.useProgram(programs[index]);gl.activeTexture(gl.TEXTURE0);gl.bindTexture(gl.TEXTURE_2D,texture);gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA8,width,height,0,gl.RGBA,gl.UNSIGNED_BYTE,pixels);const sampler=gl.getUniformLocation(programs[index],'gamepkgTexture_0');if(sampler!==null)gl.uniform1i(sampler,0);gl.drawArrays(gl.TRIANGLES,0,3);target.drawImage(shaderSurface,0,0);}
// Use one-level GPU bilinear sampling rather than Canvas2D's implementation-
// dependent minification filter. Match desktop encoded-byte premultiplication.
let linearGraphics=null;
function linearSprite(x,y,w,h,width,height,pixels,straight=false,target=context){
 if(w<=0||h<=0)return;
 if(!linearGraphics){
  const surface=new OffscreenCanvas(640,480),g=surface.getContext('webgl2',{alpha:true,depth:false,antialias:false,premultipliedAlpha:true,preserveDrawingBuffer:true});
  if(!g)throw Error('Filtered game images require WebGL 2');
  const compile=(type,source)=>{const shader=g.createShader(type);g.shaderSource(shader,source);g.compileShader(shader);if(!g.getShaderParameter(shader,g.COMPILE_STATUS))throw Error('Filtered image shader failed');return shader;};
  const vertex=compile(g.VERTEX_SHADER,`#version 300 es
  uniform vec4 bounds;uniform vec4 coverage;out vec2 uv;
  void main(){vec2 p[6]=vec2[6](vec2(0,0),vec2(1,0),vec2(0,1),vec2(0,1),vec2(1,0),vec2(1,1));vec2 at=coverage.xy+p[gl_VertexID]*coverage.zw;uv=bounds.xy+p[gl_VertexID]*bounds.zw;gl_Position=vec4(at.x/320.0-1.0,1.0-at.y/240.0,0,1);}`);
  const fragment=compile(g.FRAGMENT_SHADER,`#version 300 es
  precision highp float;uniform sampler2D image;uniform bool straightAlpha;in vec2 uv;out vec4 result;
  void main(){result=texture(image,uv);if(straightAlpha)result.rgb*=result.a;}`);
  const program=g.createProgram();g.attachShader(program,vertex);g.attachShader(program,fragment);g.linkProgram(program);g.deleteShader(vertex);g.deleteShader(fragment);
  if(!g.getProgramParameter(program,g.LINK_STATUS))throw Error('Filtered image program failed');
  const image=g.createTexture();g.bindTexture(g.TEXTURE_2D,image);g.texParameteri(g.TEXTURE_2D,g.TEXTURE_MIN_FILTER,g.LINEAR);g.texParameteri(g.TEXTURE_2D,g.TEXTURE_MAG_FILTER,g.LINEAR);g.texParameteri(g.TEXTURE_2D,g.TEXTURE_WRAP_S,g.CLAMP_TO_EDGE);g.texParameteri(g.TEXTURE_2D,g.TEXTURE_WRAP_T,g.CLAMP_TO_EDGE);g.bindVertexArray(g.createVertexArray());
  linearGraphics={surface,g,program,image,bounds:g.getUniformLocation(program,'bounds')};
 }
 const {surface,g,program,image,bounds}=linearGraphics;if(g.isContextLost())throw Error('Filtered image context unavailable');
 const premultiplied=new Uint8Array(pixels.length);
 for(let i=0;i<pixels.length;i+=4){const a=pixels[i+3];for(let c=0;c<3;c++)premultiplied[i+c]=straight?pixels[i+c]:Math.floor((pixels[i+c]*a+127)/255);premultiplied[i+3]=a;}
 // Blend before framebuffer quantization, matching the desktop GPU. A
 // transparent intermediate rounds sampled color and alpha before compositing.
 g.viewport(0,0,640,480);g.useProgram(program);g.activeTexture(g.TEXTURE0);g.bindTexture(g.TEXTURE_2D,image);g.uniform1i(g.getUniformLocation(program,'image'),0);
 g.uniform1i(g.getUniformLocation(program,'straightAlpha'),0);g.disable(g.BLEND);g.texImage2D(g.TEXTURE_2D,0,g.RGBA8,g.RGBA,g.UNSIGNED_BYTE,target.canvas);g.uniform4f(bounds,0,0,1,1);g.uniform4f(g.getUniformLocation(program,'coverage'),0,0,640,480);g.drawArrays(g.TRIANGLES,0,6);
 g.uniform1i(g.getUniformLocation(program,'straightAlpha'),straight?1:0);g.enable(g.BLEND);g.blendFunc(g.ONE,g.ONE_MINUS_SRC_ALPHA);g.texImage2D(g.TEXTURE_2D,0,g.RGBA8,width,height,0,g.RGBA,g.UNSIGNED_BYTE,premultiplied);const left=Math.ceil(x-.5),top=Math.floor(y-.5)+1,right=Math.ceil(x+w-.5),bottom=Math.floor(y+h-.5)+1;g.uniform4f(bounds,(left-x)/w,(top-y)/h,(right-left)/w,(bottom-top)/h);g.uniform4f(g.getUniformLocation(program,'coverage'),left,top,right-left,bottom-top);g.drawArrays(g.TRIANGLES,0,6);
 target.imageSmoothingEnabled=false;target.drawImage(surface,0,0);
}
function render(frame,target=context,publish=true){
 // Every tick is a fresh frame, matching the native player's black backdrop.
 const background=frame.background??0x10171dff;
 if((background&255)!==255){target.fillStyle='#000000';target.fillRect(0,0,640,480);}
 target.fillStyle=color(background);target.fillRect(0,0,640,480);
 target.save();
 try {for(const entry of frame.commands){const [kind,x,y,a,b,rgba]=entry;if(kind==='clip'||kind==='clip-reset'){target.restore();target.save();if(kind==='clip'){const left=Math.round(x),top=Math.round(y),right=Math.round(x+a),bottom=Math.round(y+b);target.beginPath();target.rect(left,top,right-left,bottom-top);target.clip();}}else if(kind==='shader'){shaderFrame(x,1,1,new Uint8Array(4),target);}else if(kind==='shader-image'){shaderFrame(x,y,a,b,target);}else if(kind==='rect'){const left=Math.ceil(x-.5),top=Math.floor(y-.5)+1,right=Math.ceil(x+a-.5),bottom=Math.floor(y+b-.5)+1;target.fillStyle=color(rgba);target.fillRect(left,top,Math.max(0,right-left),Math.max(0,bottom-top));}else if(kind==='linear-image'||kind==='straight-linear-image'){const [,,,,,width,height,pixels]=entry;linearSprite(x,y,a,b,width,height,pixels,kind==='straight-linear-image',target);}else if(kind==='image'){const [,,,,,width,height,pixels]=entry;if(width===1&&height===1){const left=Math.floor(x-.5)+1,top=Math.floor(y-.5)+1,right=Math.floor(x+a-.5)+1,bottom=Math.floor(y+b-.5)+1;target.fillStyle=color((pixels[0]<<24)|(pixels[1]<<16)|(pixels[2]<<8)|pixels[3]);target.fillRect(left,top,right-left,bottom-top);}else{const surface=document.createElement('canvas');surface.width=width;surface.height=height;surface.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray(pixels),width,height),0,0);target.imageSmoothingEnabled=false;target.drawImage(surface,x,y,a,b);}}else{target.fillStyle=color(b);target.beginPath();target.arc(x,y,a,0,Math.PI*2);target.fill();}}} finally {target.restore();}

 if(!data.custom_renderer&&frame.commands.length===0&&frame.background===null){const state=BigInt(frame.state);target.fillStyle='#111a2c';target.fillRect(0,0,640,480);const x=Number(state&15n),y=Number(state>>8n&255n),tx=Number(state>>16n&255n),ty=Number(state>>24n&255n);target.fillStyle='#ffd45d';target.fillRect(tx*40,ty*40,32,32);target.fillStyle='#6ee7ac';target.fillRect(x*40,y*40,32,32);}
 if(publish)publishFrameIdentity(frame);
}
function publishFrameIdentity(frame){
 document.querySelector('#score').textContent=data.custom_renderer?'':`Score ${BigInt(frame.state)>>32n}`;
 canvas.dataset.state=frame.state;
}
let stagedSurface=null;
function prepareFrame(frame){
 // Reuse one bounded backing surface; failed persistence never touches the
 // visible canvas, including shader/filter draws that sample earlier pixels.
 if(!stagedSurface){stagedSurface=document.createElement('canvas');stagedSurface.width=640;stagedSurface.height=480;}
 const target=stagedSurface.getContext('2d');if(!target)throw Error('Frame staging unavailable');
 render(frame,target,false);
}
function publishFrame(frame){context.drawImage(stagedSurface,0,0);publishFrameIdentity(frame);
}
function scheduleTick(){clearTimeout(frameTimer);const now=performance.now();nextTickAt=Math.max(nextTickAt+1000/60,now);frameTimer=setTimeout(tick,Math.max(0,nextTickAt-now));}
function playAudio(samples,channels=1){if(!audioContext||audioContext.state!=='running'||!samples.length||samples.length>1600||![1,2].includes(channels)||samples.length%channels)return;audioNext=Math.max(audioContext.currentTime,audioNext);if(audioNext>audioContext.currentTime+.1)return;const frames=samples.length/channels,buffer=audioContext.createBuffer(channels,frames,48000);for(let c=0;c<channels;c++){const channel=buffer.getChannelData(c);for(let i=0;i<frames;i++)channel[i]=samples[i*channels+c]/32768;}const source=audioContext.createBufferSource();source.buffer=buffer;source.connect(audioGain);audioNodes.add(source);source.onended=()=>audioNodes.delete(source);source.start(audioNext);audioNext+=frames/48000;canvas.dataset.audioSamples=String(Number(canvas.dataset.audioSamples||0)+samples.length);canvas.dataset.audioChannels=String(channels);}
async function start(){
 if(!data)return;
 if(roms.some(value=>!value)){status.textContent='Select all required ROM files before playing.';return;}
 halt('Loading saved progress…');play.disabled=true;restart.disabled=true;stop.disabled=false;
 const launch=startSerial,needsRecorded=data.manifest.limits.effects_memory_bytes!==undefined||data.manifest.limits.music_memory_bytes!==undefined;let audioReady=Promise.resolve();
 if(data.manifest.permissions.audio&&!needsRecorded){try{if(!audioContext){audioContext=new AudioContext({sampleRate:48000});audioGain=audioContext.createGain();audioGain.gain.value=muted?0:.5;audioGain.connect(audioContext.destination);}audioReady=Promise.race([audioContext.resume(),new Promise(resolve=>setTimeout(resolve,2000))]).catch(()=>{});}catch{}}
 let saved;try{const [record]=await Promise.all([progressStore.load(),audioReady,audioClosing]);saved=record.save;}catch(error){if(launch===startSerial)halt(error instanceof ProgressDataError?'Game stopped: browser progress is invalid.':'Unable to load progress or finish the previous audio session.');return;}
 if(launch!==startSerial)return;
 delete canvas.dataset.state;running=true;
 try{
  gameBudget=new RecordedBudget(data.manifest.limits.hard_memory_bytes);
  if(needsRecorded){
   const parent=gameBudget,abort=new AbortController();recordedAbort=abort;
   const opening=import('./recorded-player-start.js').then(({startRecordedPlayer})=>startRecordedPlayer(data,parent,abort.signal));recordedOpening=opening;
   const session=await opening;if(recordedOpening===opening)recordedOpening=null;
   if(launch!==startSerial){await session.close();return;}recordedSession=session;session.volume(muted?0:64);await session.resume();if(launch!==startSerial)return;
   worker=new Worker('worker.js');if(!session.attachGuest(worker))throw Error('Game worker ownership failed');
  }else{guestMemory=gameBudget.reserve(gameBudget.limit);worker=new Worker('worker.js');}
 }catch(error){if(launch===startSerial)halt(needsRecorded?String(error):'Unable to reserve memory or start the game worker.');return;}watch(10000);
 const active=worker;let saveResult=null,lastWindowSerial=0;
 worker.onerror=()=>{if(worker===active)halt('Game worker failed.');};
 worker.onmessage=async ({data:message})=>{
  if(!running||worker!==active)return;
  if(['save','frame','file-request','restarted'].includes(message.type)&&message.epoch!==homepageEpoch){if(restartPending&&message.type==='frame'){clearTimeout(timer);tickPending=false;}return;}
  if(['error','ready','frame'].includes(message.type))clearTimeout(timer);
  if(message.type==='error'){halt(message.error);return;}
  if(message.type==='ready'){lastFrame=performance.now();nextTickAt=lastFrame;status.textContent='Playing';restart.disabled=false;canvas.focus();tick();}
  if(message.type==='restarted'){restartPending=false;restart.disabled=false;tickPending=false;lastWindowSerial=0;saveResult=null;lastFrame=performance.now();nextTickAt=lastFrame;tick();return;}
  if(message.type==='file-request')showFileRequest(message,active);
  if(message.type==='save'){if(saveResult||!Number.isInteger(message.serial)||message.serial!==lastWindowSerial+1||!(message.bytes instanceof Uint8Array)||message.bytes.length>1048576){halt('Invalid save frame');return;}saveResult={serial:message.serial,bytes:message.bytes};}
  if(message.type==='frame'){
   if(!Number.isInteger(message.serial)||message.serial!==lastWindowSerial+1||message.window?.homepage_epoch!==homepageEpoch){halt('Invalid window frame');return;}
   lastWindowSerial=message.serial;
   try{prepareFrame(message);}catch(error){halt(String(error));return;}
   const frameAudio=recordedSession;
   if(frameAudio){try{if(!await frameAudio.prepare(message))throw Error('Recorded audio frame was rejected');}catch(error){if(running&&worker===active)halt(String(error));return;}if(!running||worker!==active||message.epoch!==homepageEpoch)return;}
   let committed=false,invalid=false;
   const now=performance.now(),elapsed=Math.max(0,Math.min(250,Math.round(now-lastFrame)));lastFrame=now;
   const bytes=saveResult?.serial===message.serial?saveResult.bytes:null;saveResult=null;
   try{await progressStore.commit(bytes,elapsed,message.unlocks);committed=true;}catch(error){invalid=error instanceof ProgressDataError;}
   if(!running||worker!==active||message.epoch!==homepageEpoch)return;
   if(frameAudio){try{if(!await frameAudio.finish(message.epoch,message.serial,committed))throw Error('Recorded audio checkpoint was rejected');}catch(error){if(running&&worker===active)halt(String(error));return;}if(!running||worker!==active||message.epoch!==homepageEpoch)return;}
   tickPending=false;
   if(invalid){active.postMessage({type:'window-commit',epoch:homepageEpoch,serial:message.serial,success:false});halt('Game stopped: browser play history is invalid.');return;}
   if(!committed)status.textContent=storageWarning;
   active.postMessage({type:'window-commit',epoch:homepageEpoch,serial:message.serial,success:committed});
   if(committed){if(status.textContent===storageWarning)status.textContent='Playing';try{publishFrame(message);if(!message.window?.closed)playAudio(message.audio||[],message.audioChannels??1);}catch(error){halt(String(error));return;}}
   if(committed&&!applyWindowControls(message.window))return;
   scheduleTick();
  }
 };
 worker.postMessage({type:'start',wasm,manifest:data.manifest,assets:data.assets,precise_images:data.precise_images??{},shaderCount:(data.shaders||[]).length,roms,save:saved});
}
canvas.addEventListener('keydown',keyboardDown);
canvas.addEventListener('keyup',keyboardUp);
window.addEventListener('blur',resetKeyboard);
document.querySelector('#mute').onclick=()=>{muted=!muted;if(audioGain)audioGain.gain.value=muted?0:.5;recordedSession?.volume(muted?0:64);document.querySelector('#mute').textContent=muted?'Unmute':'Mute';document.querySelector('#mute').setAttribute('aria-pressed',String(muted));};
play.onclick=start;restart.onclick=async()=>{if(recordedSession&&running){await start();return;}if(running&&worker&&!restartPending){
 if(homepageEpoch===4294967295){halt('Restart counter exhausted');return;}
 const active=worker,epoch=++homepageEpoch;restartPending=true;restart.disabled=true;clearAudio();clearTimeout(frameTimer);
 delete canvas.dataset.state;clearFileUI();resetWindowControls(true);resetSurface();
 try{const record=await progressStore.load();if(!running||worker!==active||homepageEpoch!==epoch)return;watch(250);active.postMessage({type:'restart',homepageEpoch:epoch,save:record.save});canvas.focus();}
 catch{if(running&&worker===active&&homepageEpoch===epoch)halt('Unable to reload browser progress.');}
}};stop.onclick=()=>halt('Stopped');
try{
 const response=await fetch('game.json',{cache:'no-store'});if(!response.ok)throw Error('Missing export metadata');data=await response.json();if(data.format_version!==1)throw Error('Unsupported web export');
 const binary=await fetch('game.wasm',{cache:'no-store'});if(!binary.ok)throw Error('Missing compiled game');wasm=await binary.arrayBuffer();
 const digest=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',wasm)),byte=>byte.toString(16).padStart(2,'0')).join('');if(digest!==data.wasm_sha256)throw Error('Compiled game integrity check failed');
 setupShaders(data.shaders||[]);
 const manifest=data.manifest;document.querySelector('#mute').disabled=!manifest.permissions.audio;document.querySelector('#title').textContent=manifest.id;document.querySelector('#identity').textContent=`Version ${manifest.version} · Creator ${manifest.creator_key}`;
 saveKey=`gamepkg:save:${manifest.id}:${manifest.creator_key}:${manifest.save_schema}`;historyKey=`gamepkg:history:${manifest.id}:${manifest.creator_key}`;
 progressStore=new ProgressStore(saveKey,historyKey);
 const required=manifest.roms?.length?manifest.roms:manifest.requires_rom?[{title:'ROM'}]:[];roms=Array(required.length).fill(null);
 required.forEach((requirement,index)=>{const label=document.createElement('label');label.textContent=`Select ${requirement.title}`;const input=document.createElement('input');input.type='file';input.onchange=async()=>{const file=input.files[0];if(!file||file.size>67108864){status.textContent='ROM must be at most 64 MiB.';roms[index]=null;return;}const selected=new Uint8Array(await file.arrayBuffer());if(roms.reduce((total,value,i)=>total+(i===index?0:value?.length||0),0)+selected.length>134217728){status.textContent='Combined ROM files must be at most 128 MiB.';roms[index]=null;return;}roms[index]=selected;};label.append(input);document.querySelector('#roms').append(label);});
 status.textContent='Ready to play';play.disabled=false;restart.disabled=false;
}catch(error){status.textContent=String(error);}

// Host-owned window controls
function homepageURL(value){
 if(typeof value!=='string'||new TextEncoder().encode(value).length>2048||/[\s\u0000-\u001f\u007f-\u009f]/.test(value))return null;
 try{const url=new URL(value),host=url.hostname.toLowerCase().replace(/\.+$/,'');
  if(!host||url.username||url.password)return null;
  const onion=host.endsWith('.onion');if(onion&&!/^[a-z2-7]{56}$/.test(host.slice(0,-6).split('.').at(-1)))return null;
  return (url.protocol==='https:'||onion&&url.protocol==='http:')?value:null;
 }catch{return null;}
}
// Escape remains a game key; Ctrl+Alt+Escape releases capture.
let requestedWindow={visible:true,confined:false,fullscreen_toggle:false,closed:false},lastWindowToggle=false,lastHomepageRequest=0,homepageEpoch=0,pointerSuspended=false,captureAttempted=false,capturePending=false,intentionalUnlock=false,captureEpoch=0,pendingCaptureEpoch=0,fullscreenBusy=false,fullscreenQueued=false;
function windowFocused(){return running&&document.activeElement===canvas&&document.hasFocus?.()!==false;}
function pointerStyle(){canvas.style.cursor=windowFocused()&&!pointerSuspended&&!filePending&&!requestedWindow.visible&&(!requestedWindow.confined||document.pointerLockElement===canvas)?'none':'auto';const button=document.querySelector('#capture');if(button)button.hidden=!(running&&requestedWindow.confined&&document.pointerLockElement!==canvas&&!filePending);}
function releaseWindowPointer(){captureEpoch++;intentionalUnlock=true;capturePending=false;if(document.pointerLockElement===canvas)document.exitPointerLock?.();canvas.style.cursor='auto';}
function resetWindowControls(leaveFullscreen=false){requestedWindow={visible:true,confined:false,fullscreen_toggle:false,closed:false};lastWindowToggle=false;lastHomepageRequest=0;if(!leaveFullscreen)homepageEpoch=0;fullscreenQueued=false;pointerSuspended=false;captureAttempted=false;releaseWindowPointer();if(!leaveFullscreen&&document.fullscreenElement===canvas)document.exitFullscreen?.().catch?.(()=>{});for(const id of ['#capture','#fullscreen']){const button=document.querySelector(id);if(button)button.hidden=true;}}
async function captureWindowPointer(explicit=false){
 if(!windowFocused()||filePending||!requestedWindow.confined||capturePending||document.pointerLockElement===canvas||(!explicit&&(pointerSuspended||captureAttempted)))return;
 captureAttempted=true;capturePending=true;intentionalUnlock=false;const epoch=captureEpoch;pendingCaptureEpoch=epoch;
 try{if(!canvas.requestPointerLock)throw Error('Pointer lock unavailable');await canvas.requestPointerLock();if(epoch!==captureEpoch||!windowFocused()||filePending||pointerSuspended||!requestedWindow.confined)releaseWindowPointer();}catch{pointerSuspended=true;}finally{capturePending=false;pointerStyle();}
}
async function toggleWindowFullscreen(){
 fullscreenQueued=!fullscreenQueued;if(fullscreenBusy)return;fullscreenBusy=true;const button=document.querySelector('#fullscreen');
 try{while(fullscreenQueued&&running&&!filePending){fullscreenQueued=false;
  if(document.fullscreenElement===canvas)await document.exitFullscreen();
  else{if(!canvas.requestFullscreen)throw Error('Fullscreen unavailable');await canvas.requestFullscreen();}
  if(!running||filePending||document.hasFocus?.()===false){fullscreenQueued=false;if(document.fullscreenElement===canvas)await document.exitFullscreen();}
  if(button)button.hidden=true;
 }}catch{fullscreenQueued=false;if(button)button.hidden=false;}finally{fullscreenBusy=false;}
}
function applyWindowControls(requested,homepageCommitted=true){
 if(!requested||typeof requested.visible!=='boolean'||typeof requested.confined!=='boolean'||typeof requested.fullscreen_toggle!=='boolean'||typeof requested.closed!=='boolean')return true;
 const request=requested.homepage_request??0;
 if((requested.homepage_epoch??0)===homepageEpoch&&Number.isInteger(request)&&request>lastHomepageRequest&&request<=4294967295){
  lastHomepageRequest=request;
  const url=homepageURL(data?.manifest?.homepage);
  if(homepageCommitted&&url&&!requested.closed&&!filePending&&windowFocused()&&typeof navigator!=='undefined'&&navigator.userActivation?.isActive)window.open(url,'_blank','noopener,noreferrer');
 }
 requestedWindow=requested;if(requested.closed){halt('Game closed');return false;}
 if(!requested.confined){captureAttempted=false;pointerSuspended=false;releaseWindowPointer();}
 if(filePending){releaseWindowPointer();pointerStyle();return true;}
 if(windowFocused()&&requested.fullscreen_toggle!==lastWindowToggle){lastWindowToggle=requested.fullscreen_toggle;toggleWindowFullscreen();}
 if(requested.confined)captureWindowPointer();pointerStyle();return true;
}
function releaseWindowForFile(){pointerSuspended=true;releaseWindowPointer();if(document.fullscreenElement===canvas)document.exitFullscreen?.().catch?.(()=>{});pointerStyle();}
canvas.addEventListener('pointerdown',event=>{if(requestedWindow.confined){location(event);pointerSuspended=false;captureAttempted=false;captureWindowPointer(true);}});
const captureButton=document.querySelector('#capture');if(captureButton)captureButton.onclick=()=>{canvas.focus();pointerSuspended=false;captureAttempted=false;captureWindowPointer(true);};
const fullscreenButton=document.querySelector('#fullscreen');if(fullscreenButton)fullscreenButton.onclick=()=>{canvas.focus();toggleWindowFullscreen();};
document.addEventListener('pointerlockchange',()=>{
 if(document.pointerLockElement===canvas){if(!windowFocused()||filePending||pointerSuspended||!requestedWindow.confined||pendingCaptureEpoch!==captureEpoch){releaseWindowPointer();return;}intentionalUnlock=false;pointerSuspended=false;pointerStyle();return;}
 const unexpected=!intentionalUnlock&&requestedWindow.confined;intentionalUnlock=false;capturePending=false;
 if(unexpected){pointerSuspended=true;const lost=windowClock();setTimeout(()=>{if(windowFocused()&&!filePending&&requestedWindow.confined&&lastEscapeInput<lost-100){syntheticEscape=windowClock();surface([5,0,0,41]);surface([6,0,0,41]);surface([9,0,0,27]);surface([10,0,0,27]);}},0);}
 pointerStyle();
});
window.addEventListener('blur',()=>{pointerSuspended=true;releaseWindowPointer();pointerStyle();});

// Native browser file selection/downloads remain outside the guest worker.
function chosenFileName(name,extensions){if(typeof name!=='string'||!name||new TextEncoder().encode(name).length>255||/[. ]$/.test(name)||/[<>:"/\\|?*\x00-\x1f\x7f-\x9f]/.test(name))return false;const stem=name.split('.')[0].replace(/ +$/,'').toUpperCase();if(/^(CON|PRN|AUX|NUL|CONIN\$|CONOUT\$|COM[1-9¹²³]|LPT[1-9¹²³])$/.test(stem))return false;const split=name.lastIndexOf('.');return split>=0&&extensions.includes(name.slice(split+1).toLowerCase());}
function clearFileUI(){filePending=null;const box=document.querySelector('#user-files');if(box){box.replaceChildren();box.hidden=true;}}
function showFileRequest(message,active){
 if(filePending||!Number.isInteger(message.handle)||message.handle<1||!Number.isInteger(message.generation)||!Number.isInteger(message.index))return;
 const policy=data.manifest.permissions.files?.[message.index];if(!policy||![0,1].includes(message.operation)||!(message.operation===0?policy.read:policy.write))return;
 if(message.operation===1&&(!(message.bytes instanceof Uint8Array)||message.bytes.length>1048576||message.bytes.length>policy.max_bytes||!chosenFileName(message.name,policy.extensions)))return;
 const box=document.querySelector('#user-files');if(!box)return;releaseWindowForFile();filePending=message;box.replaceChildren();box.hidden=false;
 const label=document.createElement('p');label.textContent=`${data.manifest.title||data.manifest.id}: ${policy.title}`;box.append(label);
 const done=(status,name='',bytes=new Uint8Array())=>{if(!running||worker!==active||filePending!==message)return;active.postMessage({type:'file-result',handle:message.handle,generation:message.generation,status,name,bytes});clearFileUI();canvas.focus();};
 if(message.operation===0){
  const input=document.createElement('input');input.type='file';input.accept=policy.extensions.map(e=>'.'+e).join(',');input.setAttribute('aria-label',`Choose ${policy.title}`);
  input.oncancel=()=>done(4);input.onchange=async()=>{const selected=input.files[0];if(!selected){done(4);return;}if(selected.size>policy.max_bytes||selected.size>67108864||!chosenFileName(selected.name,policy.extensions)){done(5);return;}try{const bytes=new Uint8Array(await selected.arrayBuffer());done(2,selected.name,bytes);}catch{done(5);}};box.append(input);
 }else{
  const button=document.createElement('button');button.textContent=`Save ${policy.title}`;button.onclick=async()=>{button.disabled=true;try{
   if(typeof window.showSaveFilePicker==='function'){
    const handle=await window.showSaveFilePicker({suggestedName:message.name,types:[{description:policy.title,accept:{'application/octet-stream':policy.extensions.map(e=>'.'+e)}}]});
    if(!running||worker!==active||filePending!==message)return;
    if(!chosenFileName(handle.name,policy.extensions))throw Error('Selected file type is not permitted');
    const stream=await handle.createWritable();await stream.write(message.bytes);await stream.close();
   }else{const link=document.createElement('a'),url=URL.createObjectURL(new Blob([message.bytes],{type:'application/octet-stream'}));link.href=url;link.download=message.name;link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}
   done(3);
  }catch(error){done(error?.name==='AbortError'?4:5);}};box.append(button);
 }
 const cancel=document.createElement('button');cancel.textContent='Cancel file request';cancel.onclick=()=>done(4);box.append(cancel);
}

// Ordered input is scoped to this focused canvas, never document-wide typing.
let surfaceEvents=[],pointerPosition=[320,240],lastEscapeInput=-Infinity,syntheticEscape=-Infinity;
function windowClock(){return typeof performance==='object'?performance.now():Date.now();}

const scanKeys={Enter:40,Escape:41,Backspace:42,Tab:43,Space:44,Minus:45,Equal:46,BracketLeft:47,BracketRight:48,Backslash:49,Semicolon:51,Quote:52,Backquote:53,Comma:54,Period:55,Slash:56,CapsLock:57,PrintScreen:70,ScrollLock:71,Pause:72,Insert:73,Home:74,PageUp:75,Delete:76,End:77,PageDown:78,ArrowRight:79,ArrowLeft:80,ArrowDown:81,ArrowUp:82,NumLock:83,NumpadDivide:84,NumpadMultiply:85,NumpadSubtract:86,NumpadAdd:87,NumpadEnter:88,NumpadDecimal:99,ControlLeft:224,ShiftLeft:225,AltLeft:226,MetaLeft:227,ControlRight:228,ShiftRight:229,AltRight:230,MetaRight:231};
function scan(code){if(/^Key[A-Z]$/.test(code))return code.charCodeAt(3)-65+4;if(/^Digit[1-9]$/.test(code))return Number(code[5])+29;if(code==='Digit0')return 39;if(/^F([1-9]|1[0-2])$/.test(code))return Number(code.slice(1))+57;if(/^Numpad[1-9]$/.test(code))return Number(code.slice(6))+88;if(code==='Numpad0')return 98;return scanKeys[code]??0;}
function surface(event){if(!running||document.activeElement!==canvas||document.hasFocus?.()===false)return;if(surfaceEvents.length>=2048){surfaceEvents=[[0,0,0,0]];return;}surfaceEvents.push(event);}
function textInput(text){for(const character of String(text??'').slice(0,4096)){const code=character.codePointAt(0);if(code&&(code<0xd800||code>0xdfff))surface([7,0,0,code]);}}
function surfaceRect(){const outer=canvas.getBoundingClientRect(),width=canvas.clientWidth||outer.width,height=canvas.clientHeight||outer.height,scale=Math.min(width/640,height/480);return{left:outer.left+(canvas.clientLeft||0)+(width-640*scale)/2,top:outer.top+(canvas.clientTop||0)+(height-480*scale)/2,width:640*scale,height:480*scale};}
function location(event,motion=false){const rect=surfaceRect();if(!(rect.width>0&&rect.height>0))return null;if(document.pointerLockElement===canvas){if(!motion)return pointerPosition;const dx=Number.isFinite(event.movementX)?event.movementX:0,dy=Number.isFinite(event.movementY)?event.movementY:0;pointerPosition=[Math.max(0,Math.min(640,pointerPosition[0]+dx/rect.width*640)),Math.max(0,Math.min(480,pointerPosition[1]+dy/rect.height*480))];}else{pointerPosition=[Math.max(0,Math.min(640,(event.clientX-rect.left)/rect.width*640)),Math.max(0,Math.min(480,(event.clientY-rect.top)/rect.height*480))];}return pointerPosition;}
function logical(event){const named={Enter:13,Escape:27,Tab:9,Backspace:8,Delete:127,' ':32};if(named[event.key])return named[event.key];const chars=[...event.key];if(chars.length===1){const ch=chars[0];return /^[A-Z]$/.test(ch)?ch.toLowerCase().codePointAt(0):ch.codePointAt(0);}const code=scan(event.code);return code?code|0x40000000:0;}
canvas.addEventListener('keydown',event=>{if(event.code==='Escape'||event.key==='Escape'){if(event.ctrlKey&&event.altKey){pointerSuspended=true;releaseWindowPointer();pointerStyle();event.preventDefault();return;}const now=windowClock();if(now-syntheticEscape<100)return;lastEscapeInput=now;}const code=scan(event.code);if(code)surface([5,0,0,code|(event.repeat?65536:0)]);const key=logical(event);if(key)surface([9,0,0,(key|(event.repeat?0x80000000:0))>>>0]);if(!event.isComposing&&!event.ctrlKey&&!event.metaKey&&!event.altKey&&[...event.key].length===1)textInput(event.key);});
canvas.addEventListener('keyup',event=>{const code=scan(event.code);if(code)surface([6,0,0,code]);const key=logical(event);if(key)surface([10,0,0,key]);});
canvas.addEventListener('compositionend',event=>textInput(event.data));
canvas.addEventListener('paste',event=>{if(document.activeElement===canvas){event.preventDefault();textInput(event.clipboardData?.getData('text/plain')??'');}});
canvas.addEventListener('pointermove',event=>{const point=location(event,true);if(point)surface([1,...point,event.buttons&31]);});
canvas.addEventListener('pointerdown',event=>{canvas.focus();const point=location(event),code=[1,4,2,8,16][event.button];if(point&&code){surface([2,...point,code]);canvas.setPointerCapture(event.pointerId);}});
canvas.addEventListener('pointerup',event=>{const point=location(event),code=[1,4,2,8,16][event.button];if(point&&code)surface([3,...point,code]);if(canvas.hasPointerCapture(event.pointerId))canvas.releasePointerCapture(event.pointerId);});
canvas.addEventListener('pointercancel',()=>surface([8,0,0,0]));
canvas.addEventListener('pointerleave',event=>{if(!event.buttons)surface([8,0,0,0]);});
canvas.addEventListener('contextmenu',event=>event.preventDefault());
canvas.addEventListener('wheel',event=>{if(document.activeElement===canvas){event.preventDefault();surface([4,Math.max(-4096,Math.min(4096,event.deltaX)),Math.max(-4096,Math.min(4096,-event.deltaY)),event.deltaMode===1?0:event.deltaMode===2?2:1]);}},{passive:false});
function resetSurface(){surfaceEvents=[[0,0,0,0]];resetKeyboard();}
canvas.addEventListener('blur',resetSurface);window.addEventListener('blur',resetSurface);
