const canvas=document.querySelector('#game'),context=canvas.getContext('2d'),status=document.querySelector('#status'),play=document.querySelector('#play'),stop=document.querySelector('#stop'),restart=document.querySelector('#restart');
let data,wasm,worker,timer,frameTimer,pressed=0,held=0,roms=[],running=false,saveKey,historyKey,lastFrame=0,audioContext,audioGain,audioNext=0,muted=false,audioNodes=new Set(),shaderSurface,gl,programs=[],texture;
const keys={ArrowLeft:1,a:1,ArrowRight:2,d:2,ArrowUp:4,w:4,ArrowDown:8,s:8};
function color(rgba){return `rgba(${rgba>>>24},${rgba>>>16&255},${rgba>>>8&255},${(rgba&255)/255})`;}
function halt(message){for(const node of audioNodes){try{node.stop();}catch{}}audioNodes.clear();audioNext=0;clearTimeout(timer);clearTimeout(frameTimer);worker?.terminate();worker=null;running=false;held=pressed=0;play.disabled=false;stop.disabled=true;restart.disabled=false;status.textContent=message;}
function watch(delay){clearTimeout(timer);const active=worker;timer=setTimeout(()=>{if(worker===active)halt('Game stopped: execution deadline exceeded.');},delay);}
function stored(key,fallback){try{return JSON.parse(localStorage.getItem(key))??fallback;}catch{return fallback;}}
function persist(key,value){try{localStorage.setItem(key,JSON.stringify(value));}catch{status.textContent='Browser storage is unavailable; progress may not persist.';}}
function tick(){if(!running)return;watch(250);worker.postMessage({type:'tick',input:pressed|(held<<8)});pressed=0;}
function shader(type,source){const value=gl.createShader(type);gl.shaderSource(value,source);gl.compileShader(value);if(!gl.getShaderParameter(value,gl.COMPILE_STATUS)){const error=gl.getShaderInfoLog(value);gl.deleteShader(value);throw Error('WebGL shader compilation failed: '+error);}return value;}
function setupShaders(sources){if(!sources.length)return;shaderSurface=new OffscreenCanvas(640,480);gl=shaderSurface.getContext('webgl2',{alpha:true,depth:false,antialias:false,premultipliedAlpha:false,preserveDrawingBuffer:true});if(!gl)throw Error('This game requires WebGL 2');const vertex=shader(gl.VERTEX_SHADER,'#version 300 es\nvoid main(){vec2 p=vec2((gl_VertexID<<1)&2,gl_VertexID&2);gl_Position=vec4(p*2.0-1.0,0.0,1.0);}');for(const source of sources){const adapted=source.replace(/^#version[^\n]*\n/,'#version 300 es\nprecision highp float;\nprecision highp int;\n').replace(/layout\(row_major\) (uniform|buffer);/g,'').replace(/layout\(binding\s*=\s*0\)\s*/g,'');const fragment=shader(gl.FRAGMENT_SHADER,adapted),program=gl.createProgram();gl.attachShader(program,vertex);gl.attachShader(program,fragment);gl.linkProgram(program);gl.deleteShader(fragment);if(!gl.getProgramParameter(program,gl.LINK_STATUS))throw Error('WebGL shader link failed: '+gl.getProgramInfoLog(program));for(let i=0;i<gl.getProgramParameter(program,gl.ACTIVE_UNIFORMS);i++){const uniform=gl.getActiveUniform(program,i);if(uniform.name!=='gamepkgTexture_0'||uniform.type!==gl.SAMPLER_2D||uniform.size!==1)throw Error('Forbidden shader resource');}programs.push(program);}gl.deleteShader(vertex);texture=gl.createTexture();gl.bindTexture(gl.TEXTURE_2D,texture);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.NEAREST);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.NEAREST);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE);gl.bindVertexArray(gl.createVertexArray());}
function shaderFrame(index,width=1,height=1,pixels=new Uint8Array(4)){if(!programs[index]||gl.isContextLost())throw Error('Shader graphics context unavailable');gl.viewport(0,0,640,480);gl.clearColor(0,0,0,0);gl.clear(gl.COLOR_BUFFER_BIT);gl.useProgram(programs[index]);gl.activeTexture(gl.TEXTURE0);gl.bindTexture(gl.TEXTURE_2D,texture);gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA8,width,height,0,gl.RGBA,gl.UNSIGNED_BYTE,pixels);const sampler=gl.getUniformLocation(programs[index],'gamepkgTexture_0');if(sampler!==null)gl.uniform1i(sampler,0);gl.drawArrays(gl.TRIANGLES,0,3);context.drawImage(shaderSurface,0,0);}
function render(frame){
 if(frame.background!==null){context.fillStyle=color(frame.background);context.fillRect(0,0,640,480);}
 for(const entry of frame.commands){const [kind,x,y,a,b,rgba]=entry;if(kind==='shader'){shaderFrame(x);}else if(kind==='shader-image'){shaderFrame(x,y,a,b);}else if(kind==='rect'){context.fillStyle=color(rgba);context.fillRect(x,y,a,b);}else if(kind==='image'){const [,,,,,width,height,pixels]=entry,surface=document.createElement('canvas');surface.width=width;surface.height=height;surface.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray(pixels),width,height),0,0);context.imageSmoothingEnabled=false;context.drawImage(surface,x,y,a,b);}else{context.fillStyle=color(b);context.beginPath();context.arc(x,y,a,0,Math.PI*2);context.fill();}}
 if(!data.custom_renderer&&frame.commands.length===0&&frame.background===null){const state=BigInt(frame.state);context.fillStyle='#111a2c';context.fillRect(0,0,640,480);const x=Number(state&15n),y=Number(state>>8n&255n),tx=Number(state>>16n&255n),ty=Number(state>>24n&255n);context.fillStyle='#ffd45d';context.fillRect(tx*40,ty*40,32,32);context.fillStyle='#6ee7ac';context.fillRect(x*40,y*40,32,32);}
 document.querySelector('#score').textContent=data.custom_renderer?'':`Score ${BigInt(frame.state)>>32n}`;
 canvas.dataset.state=frame.state;
}
function playAudio(samples){if(!audioContext||audioContext.state!=='running'||!samples.length||samples.length>1600)return;audioNext=Math.max(audioContext.currentTime,audioNext);if(audioNext>audioContext.currentTime+.1)return;const buffer=audioContext.createBuffer(1,samples.length,48000),channel=buffer.getChannelData(0);for(let i=0;i<samples.length;i++)channel[i]=samples[i]/32768;const source=audioContext.createBufferSource();source.buffer=buffer;source.connect(audioGain);audioNodes.add(source);source.onended=()=>audioNodes.delete(source);source.start(audioNext);audioNext+=samples.length/48000;canvas.dataset.audioSamples=String(Number(canvas.dataset.audioSamples||0)+samples.length);}
async function start(){
 if(!data)return;
 if(roms.some(value=>!value)){status.textContent='Select all required ROM files before playing.';return;}
 if(data.manifest.permissions.audio){try{if(!audioContext){audioContext=new AudioContext({sampleRate:48000});audioGain=audioContext.createGain();audioGain.gain.value=muted?0:.5;audioGain.connect(audioContext.destination);}await Promise.race([audioContext.resume(),new Promise(resolve=>setTimeout(resolve,2000))]);}catch{}}
 halt('Starting game…');play.disabled=true;restart.disabled=true;stop.disabled=false;running=true;
 worker=new Worker('worker.js');watch(10000);
 const active=worker;
 worker.onerror=()=>{if(worker===active)halt('Game worker failed.');};
 worker.onmessage=({data:message})=>{
  if(!running||worker!==active)return;
  if(['error','ready','frame'].includes(message.type))clearTimeout(timer);
  if(message.type==='error'){halt(message.error);return;}
  if(message.type==='ready'){lastFrame=performance.now();status.textContent='Playing';canvas.focus();tick();}
  if(message.type==='save')persist(saveKey,Array.from(message.bytes));
  if(message.type==='frame'){
   try{render(message);playAudio(message.audio||[]);}catch(error){halt(String(error));return;}const history=stored(historyKey,{played_ms:0,unlocked:[]});const now=performance.now();history.played_ms+=Math.max(0,Math.min(250,Math.round(now-lastFrame)));lastFrame=now;for(const id of message.unlocks)if(!history.unlocked.includes(id))history.unlocked.push(id);persist(historyKey,history);
   frameTimer=setTimeout(tick,16);
  }
 };
 let saved=stored(saveKey,[]);if(!Array.isArray(saved)||saved.length>1048576||saved.some(v=>!Number.isInteger(v)||v<0||v>255))saved=[];
 worker.postMessage({type:'start',wasm,manifest:data.manifest,assets:data.assets,shaderCount:(data.shaders||[]).length,roms,save:saved});
}
canvas.addEventListener('keydown',event=>{const bit=keys[event.key]??keys[event.key.toLowerCase()];if(bit){event.preventDefault();if(!(held&bit))pressed|=bit;held|=bit;}});
canvas.addEventListener('keyup',event=>{const bit=keys[event.key]??keys[event.key.toLowerCase()];if(bit){event.preventDefault();held&=~bit;}});
window.addEventListener('blur',()=>{held=pressed=0;});
document.querySelector('#mute').onclick=()=>{muted=!muted;if(audioGain)audioGain.gain.value=muted?0:.5;document.querySelector('#mute').textContent=muted?'Unmute':'Mute';document.querySelector('#mute').setAttribute('aria-pressed',String(muted));};
play.onclick=start;restart.onclick=start;stop.onclick=()=>halt('Stopped');
try{
 const response=await fetch('game.json',{cache:'no-store'});if(!response.ok)throw Error('Missing export metadata');data=await response.json();if(data.format_version!==1)throw Error('Unsupported web export');
 const binary=await fetch('game.wasm',{cache:'no-store'});if(!binary.ok)throw Error('Missing compiled game');wasm=await binary.arrayBuffer();
 const digest=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',wasm)),byte=>byte.toString(16).padStart(2,'0')).join('');if(digest!==data.wasm_sha256)throw Error('Compiled game integrity check failed');
 setupShaders(data.shaders||[]);
 const manifest=data.manifest;document.querySelector('#mute').disabled=!manifest.permissions.audio;document.querySelector('#title').textContent=manifest.id;document.querySelector('#identity').textContent=`Version ${manifest.version} · Creator ${manifest.creator_key}`;
 saveKey=`gamepkg:save:${manifest.id}:${manifest.creator_key}:${manifest.save_schema}`;historyKey=`gamepkg:history:${manifest.id}:${manifest.creator_key}`;
 const required=manifest.roms?.length?manifest.roms:manifest.requires_rom?[{title:'ROM'}]:[];roms=Array(required.length).fill(null);
 required.forEach((requirement,index)=>{const label=document.createElement('label');label.textContent=`Select ${requirement.title}`;const input=document.createElement('input');input.type='file';input.onchange=async()=>{const file=input.files[0];if(!file||file.size>67108864){status.textContent='ROM must be at most 64 MiB.';roms[index]=null;return;}const selected=new Uint8Array(await file.arrayBuffer());if(roms.reduce((total,value,i)=>total+(i===index?0:value?.length||0),0)+selected.length>134217728){status.textContent='Combined ROM files must be at most 128 MiB.';roms[index]=null;return;}roms[index]=selected;};label.append(input);document.querySelector('#roms').append(label);});
 status.textContent='Ready to play';play.disabled=false;restart.disabled=false;
}catch(error){status.textContent=String(error);}
