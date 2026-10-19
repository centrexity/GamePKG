import assert from 'node:assert/strict';import {readFileSync} from 'node:fs';import vm from 'node:vm';
const worker=vm.createContext({TextEncoder,TextDecoder});vm.runInContext(readFileSync(new URL('../web-player/worker.js',import.meta.url),'utf8'),worker);
vm.runInContext('manifest={permissions:{input:true}};instance={exports:{memory:{buffer:new ArrayBuffer(65536)},cabi_realloc:()=>1024}};',worker);
const api=vm.runInContext('({api,canonical,set:events=>{eventBytes=encodeEvents(events);},size:()=>eventBytes.length,view:()=>new DataView(instance.exports.memory.buffer)})',worker);
api.set([[1,123.5,44.25,3],[7,0,0,0x1f600]]);assert.equal(api.api.input_events(65530,16),-1);assert.equal(api.size(),32);assert.equal(api.api.input_events(0,17),-1);assert.equal(api.size(),32);
assert.equal(api.api.input_events(0,16),16);assert.equal(api.view().getFloat32(4,true),123.5);assert.equal(api.view().getUint32(12,true),3);
api.canonical['input-events'](16,64);assert.equal(api.view().getUint32(64,true),1);assert.equal(api.view().getUint32(72,true),16);assert.equal(api.view().getUint32(1036,true),0x1f600);assert.equal(api.size(),0);
api.set([[9,0,0,0x80000000+114],[10,0,0,0x40000000+80]]);assert.equal(api.api.input_events(0,16),16);assert.equal(api.view().getUint32(12,true),0x80000000+114);
for(const event of [[9,0,0,0x40000200],[9,0,0,0xd800],[10,0,0,0x80000072],[99,0,0,0],[1,NaN,0,0],[1,641,0,0],[5,0,0,512],[7,0,0,0xd800],[2,0,0,3]]){api.set([event]);assert.equal(api.size(),0);}
vm.runInContext('manifest.permissions.input=false',worker);assert.equal(api.api.input_events(0,16),-1);api.canonical['input-events'](16,64);assert.equal(api.view().getUint32(64,true),0);
const handlers=new Map(),windowHandlers=new Map();const canvas={addEventListener:(name,fn)=>handlers.set(name,fn),getBoundingClientRect:()=>({left:100,top:50,width:1280,height:960}),setPointerCapture(){},hasPointerCapture:()=>false,focus(){document.activeElement=canvas;}};let focused=true;const document={activeElement:canvas,hasFocus:()=>focused};let resets=0;
const ui=vm.createContext({canvas,document,running:true,resetKeyboard(){resets++;},window:{addEventListener:(name,fn)=>windowHandlers.set(name,fn)}});
const source=readFileSync(new URL('../web-player/player.js',import.meta.url),'utf8');vm.runInContext(source.slice(source.indexOf('// Ordered input')),ui);const recorder=vm.runInContext('({values:()=>surfaceEvents,reset:()=>{surfaceEvents=[];}})',ui);
handlers.get('pointermove')({clientX:740,clientY:530,buttons:0});handlers.get('keydown')({code:'KeyN',key:'n',repeat:false});handlers.get('compositionend')({data:'é😀'});
assert.deepEqual(Array.from(recorder.values(),v=>v[0]),[1,5,9,7,7,7]);assert.deepEqual(Array.from(recorder.values()[0]),[1,320,240,0]);assert.equal(recorder.values()[5][3],0x1f600);
recorder.reset();focused=false;handlers.get('keydown')({code:'KeyN',key:'n'});assert.equal(recorder.values().length,0);focused=true;document.activeElement={};handlers.get('keydown')({code:'KeyN',key:'n'});assert.equal(recorder.values().length,0);handlers.get('blur')();assert.deepEqual(Array.from(recorder.values(),v=>Array.from(v)),[[0,0,0,0]]);assert.equal(resets,1);
document.activeElement=canvas;
for (const [rect,point,expected] of [[{left:100,top:50,width:1800,height:900},[700,275],[160,120]],[{left:100,top:50,width:900,height:1800},[550,950],[320,240]]]) {
 canvas.getBoundingClientRect=()=>rect;recorder.reset();handlers.get('pointermove')({clientX:point[0],clientY:point[1],buttons:0});assert.deepEqual(Array.from(recorder.values()[0]),[1,...expected,0]);
}
canvas.getBoundingClientRect=()=>({left:100,top:50,width:1284,height:964});Object.assign(canvas,{clientLeft:2,clientTop:2,clientWidth:1280,clientHeight:960});recorder.reset();handlers.get('pointermove')({clientX:742,clientY:532,buttons:0});assert.deepEqual(Array.from(recorder.values()[0]),[1,320,240,0]);
canvas.getBoundingClientRect=()=>({left:100,top:50,width:1800,height:900});Object.assign(canvas,{clientLeft:0,clientTop:0,clientWidth:1800,clientHeight:900});document.pointerLockElement=canvas;recorder.reset();handlers.get('pointermove')({movementX:300,movementY:225,buttons:0});assert.deepEqual(Array.from(recorder.values()[0]),[1,480,360,0]);handlers.get('pointerdown')({movementX:300,movementY:225,button:0,pointerId:1});assert.deepEqual(Array.from(recorder.values()[1]),[2,480,360,1],'locked button press counted movement twice');
console.log('Ordered browser input preserves coordinates, Unicode and focus; denied/malformed reads stay bounded.');
