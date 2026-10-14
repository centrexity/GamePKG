const canvas=document.querySelector('#game'),context=canvas.getContext('2d'),status=document.querySelector('#status'),play=document.querySelector('#play'),stop=document.querySelector('#stop'),restart=document.querySelector('#restart');
let data,wasm,worker,timer,frameTimer,pressed=0,held=0,roms=[],running=false,saveKey,historyKey,lastFrame=0;
const keys={ArrowLeft:1,a:1,ArrowRight:2,d:2,ArrowUp:4,w:4,ArrowDown:8,s:8};
function color(rgba){return `rgba(${rgba>>>24},${rgba>>>16&255},${rgba>>>8&255},${(rgba&255)/255})`;}
function halt(message){clearTimeout(timer);clearTimeout(frameTimer);worker?.terminate();worker=null;running=false;held=pressed=0;play.disabled=false;stop.disabled=true;restart.disabled=false;status.textContent=message;}
function watch(delay){clearTimeout(timer);timer=setTimeout(()=>halt('Game stopped: execution deadline exceeded.'),delay);}
function stored(key,fallback){try{return JSON.parse(localStorage.getItem(key))??fallback;}catch{return fallback;}}
function persist(key,value){try{localStorage.setItem(key,JSON.stringify(value));}catch{status.textContent='Browser storage is unavailable; progress may not persist.';}}
function tick(){if(!running)return;watch(250);worker.postMessage({type:'tick',input:pressed|(held<<8)});pressed=0;}
function render(frame){
 if(frame.background!==null){context.fillStyle=color(frame.background);context.fillRect(0,0,640,480);}
 for(const [kind,x,y,a,b,rgba] of frame.commands){if(kind==='rect'){context.fillStyle=color(rgba);context.fillRect(x,y,a,b);}else{context.fillStyle=color(b);context.beginPath();context.arc(x,y,a,0,Math.PI*2);context.fill();}}
 if(frame.commands.length===0&&frame.background===null){const state=BigInt(frame.state);context.fillStyle='#111a2c';context.fillRect(0,0,640,480);const x=Number(state&15n),y=Number(state>>8n&255n),tx=Number(state>>16n&255n),ty=Number(state>>24n&255n);context.fillStyle='#ffd45d';context.fillRect(tx*40,ty*40,32,32);context.fillStyle='#6ee7ac';context.fillRect(x*40,y*40,32,32);}
 canvas.dataset.state=frame.state;
}
async function start(){
 if(!data)return;
 if(roms.some(value=>!value)){status.textContent='Select all required ROM files before playing.';return;}
 halt('Starting game…');play.disabled=true;restart.disabled=true;stop.disabled=false;running=true;
 worker=new Worker('worker.js');watch(10000);
 worker.onerror=()=>halt('Game worker failed.');
 worker.onmessage=({data:message})=>{
  if(['error','ready','frame'].includes(message.type))clearTimeout(timer);
  if(message.type==='error'){halt(message.error);return;}
  if(message.type==='ready'){lastFrame=performance.now();status.textContent='Playing';canvas.focus();tick();}
  if(message.type==='save')persist(saveKey,Array.from(message.bytes));
  if(message.type==='frame'){
   render(message);const history=stored(historyKey,{played_ms:0,unlocked:[]});const now=performance.now();history.played_ms+=Math.max(0,Math.min(250,Math.round(now-lastFrame)));lastFrame=now;for(const id of message.unlocks)if(!history.unlocked.includes(id))history.unlocked.push(id);persist(historyKey,history);
   frameTimer=setTimeout(tick,16);
  }
 };
 let saved=stored(saveKey,[]);if(!Array.isArray(saved)||saved.length>1048576||saved.some(v=>!Number.isInteger(v)||v<0||v>255))saved=[];
 worker.postMessage({type:'start',wasm,manifest:data.manifest,assets:data.assets,roms,save:saved});
}
canvas.addEventListener('keydown',event=>{const bit=keys[event.key];if(bit){event.preventDefault();if(!(held&bit))pressed|=bit;held|=bit;}});
canvas.addEventListener('keyup',event=>{const bit=keys[event.key];if(bit){event.preventDefault();held&=~bit;}});
window.addEventListener('blur',()=>{held=pressed=0;});
play.onclick=start;restart.onclick=start;stop.onclick=()=>halt('Stopped');
try{
 const response=await fetch('game.json',{cache:'no-store'});if(!response.ok)throw Error('Missing export metadata');data=await response.json();if(data.format_version!==1)throw Error('Unsupported web export');
 const binary=await fetch('game.wasm',{cache:'no-store'});if(!binary.ok)throw Error('Missing compiled game');wasm=await binary.arrayBuffer();
 const digest=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',wasm)),byte=>byte.toString(16).padStart(2,'0')).join('');if(digest!==data.wasm_sha256)throw Error('Compiled game integrity check failed');
 const manifest=data.manifest;document.querySelector('#title').textContent=manifest.id;document.querySelector('#identity').textContent=`Version ${manifest.version} · Creator ${manifest.creator_key}`;
 saveKey=`gamepkg:save:${manifest.id}:${manifest.creator_key}:${manifest.save_schema}`;historyKey=`gamepkg:history:${manifest.id}:${manifest.creator_key}`;
 const required=manifest.roms?.length?manifest.roms:manifest.requires_rom?[{title:'ROM'}]:[];roms=Array(required.length).fill(null);
 required.forEach((requirement,index)=>{const label=document.createElement('label');label.textContent=`Select ${requirement.title}`;const input=document.createElement('input');input.type='file';input.onchange=async()=>{const file=input.files[0];if(!file||file.size>67108864){status.textContent='ROM must be at most 64 MiB.';roms[index]=null;return;}roms[index]=new Uint8Array(await file.arrayBuffer());};label.append(input);document.querySelector('#roms').append(label);});
 status.textContent='Ready to play';play.disabled=false;restart.disabled=false;
}catch(error){status.textContent=String(error);}
