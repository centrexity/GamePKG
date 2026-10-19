import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const source=readFileSync(new URL('../web-player/worker.js',import.meta.url),'utf8');
function fixture(){
 const messages=[];
 const worker=vm.createContext({URL,TextEncoder,TextDecoder,Uint8Array,performance:{now:()=>0},postMessage:value=>messages.push(value)});
 vm.runInContext(source,worker);
 vm.runInContext(`manifest={permissions:{input:true,save:false,files:[{id:'patterns',title:'Patterns',extensions:['txt'],max_bytes:1024,read:true,write:true}]},homepage:'https://example.org/project',limits:{hard_memory_bytes:65536}};
 let actions=[],askFile=false,fileOperation=0;
 instance={exports:{memory:{buffer:new ArrayBuffer(65536)},gamepkg_tick:(input,state)=>{
   if(askFile){if(fileOperation===0)api.file_call(0,0,4,64,4);else{
     const name=new TextEncoder().encode('state.txt'),request=new Uint8Array(8+name.length+3);
     new DataView(request.buffer).setUint32(4,name.length,true);request.set(name,8);request.set([1,2,3],8+name.length);fileCall(1,request,4);
   }}
   for(const [op,value] of actions)windowRequest(op,value);
   return state+1n;
 }}};`,worker);
 const state=()=>JSON.parse(vm.runInContext('JSON.stringify(windowCurrent)',worker));
 const send=async data=>worker.onmessage({data});
 const actions=values=>{worker.nextActions=values;vm.runInContext('actions=nextActions;',worker);};
 return {worker,messages,state,send,actions};
}
const f=fixture();f.actions([[0,0],[1,1],[2,0],[4,0],[3,0]]);
await f.send({type:'tick',input:4,events:[],fullscreen:false});
let frame=f.messages.at(-1);assert.equal(frame.type,'frame');assert.equal(frame.serial,1);assert.equal(frame.epoch,0);
assert.equal(frame.window.closed,true);assert.equal(frame.window.visible,false);assert.equal(frame.window.homepage_request,1);
assert.equal(f.state().closed,false);assert.equal(f.state().visible,true,'Unacknowledged cursor change committed');
for(const reply of [{epoch:1,serial:1,success:true},{epoch:0,serial:2,success:true},{epoch:0,serial:1,success:1}]){
 await f.send({type:'window-commit',...reply});assert.equal(f.state().closed,false);
 assert.equal(vm.runInContext('windowPending.serial',f.worker),1);
}
await f.send({type:'window-commit',epoch:0,serial:1,success:false});
assert.equal(f.state().closed,false);assert.equal(f.state().visible,true);assert.equal(f.state().confined,false);assert.equal(f.state().fullscreen_toggle,false);assert.equal(f.state().homepage_request,0);
f.actions([[0,0],[1,1],[2,0],[4,0]]);
await f.send({type:'tick',input:4,events:[],fullscreen:false});frame=f.messages.at(-1);assert.equal(frame.serial,2);
await f.send({type:'window-commit',epoch:0,serial:1,success:true});assert.equal(f.state().visible,true,'Replayed acknowledgement committed another frame');
await f.send({type:'window-commit',epoch:0,serial:2,success:true});assert.equal(f.state().visible,false);assert.equal(f.state().confined,true);assert.equal(f.state().fullscreen_toggle,true);assert.equal(f.state().homepage_request,1);
// Rollback must restore a nondefault prior state, not synthesize defaults.
f.actions([[0,1],[1,0],[2,0]]);await f.send({type:'tick',input:4,events:[],fullscreen:true});
await f.send({type:'window-commit',epoch:0,serial:3,success:false});
assert.equal(f.state().visible,false);assert.equal(f.state().confined,true);assert.equal(f.state().fullscreen_toggle,true);
// Restart invalidates the old pending frame even when its serial is reused.
f.actions([[3,0]]);await f.send({type:'tick',input:4,events:[],fullscreen:false});
await f.send({type:'restart',homepageEpoch:1});assert.equal(f.messages.at(-1).type,'restarted');assert.equal(f.state().visible,true);
f.actions([[0,0]]);await f.send({type:'tick',input:4,events:[],fullscreen:false});assert.equal(f.messages.at(-1).serial,1);
await f.send({type:'window-commit',epoch:0,serial:1,success:true});assert.equal(f.state().visible,true);
await f.send({type:'window-commit',epoch:1,serial:1,success:true});assert.equal(f.state().visible,false);
// Picker jobs are not published before persistence acknowledges their frame.
for(const operation of [0,1])for(const success of [false,true]){
 const file=fixture();vm.runInContext(`askFile=true;fileOperation=${operation};`,file.worker);
 await file.send({type:'tick',input:4,events:[],fullscreen:false});
 assert(!file.messages.some(message=>message.type==='file-request'));
 await file.send({type:'window-commit',epoch:0,serial:1,success});
 assert.equal(file.messages.filter(message=>message.type==='file-request').length,success?1:0);
 if(success){assert.equal(file.messages.at(-1).epoch,0);assert.equal(file.messages.at(-1).operation,operation);if(operation===1)assert.deepEqual(Array.from(file.messages.at(-1).bytes),[1,2,3]);}
 else assert.equal(vm.runInContext('fileEntry',file.worker),null);
}
const closing=fixture();vm.runInContext('askFile=true;',closing.worker);closing.actions([[3,0]]);
await closing.send({type:'tick',input:4,events:[],fullscreen:false});await closing.send({type:'window-commit',epoch:0,serial:1,success:true});
assert.equal(closing.state().closed,true);assert(!closing.messages.some(message=>message.type==='file-request'));
assert.equal(vm.runInContext('fileEntry',closing.worker),null);
const overlap=fixture();overlap.actions([[0,0],[3,0]]);await overlap.send({type:'tick',input:4,events:[],fullscreen:false});await overlap.send({type:'tick',input:4,events:[],fullscreen:false});
assert.equal(overlap.messages.at(-1).type,'error');assert.match(overlap.messages.at(-1).error,/not acknowledged/);
await overlap.send({type:'window-commit',epoch:0,serial:1,success:true});assert.equal(overlap.state().visible,true);assert.equal(overlap.state().closed,false);
console.log('Browser window/file publication waits for a matching persistence acknowledgement, rolls back denial, and rejects stale/overlapping frames.');

const player=readFileSync(new URL('../web-player/player.js',import.meta.url),'utf8');
const history=vm.createContext({historyKey:'fixture',stored:()=>history.record});
vm.runInContext(player.slice(player.indexOf('function playHistory'),player.indexOf('function tick')),history);
for(const record of [{},{played_ms:123,unlocked:['legacy-achievement']}]){
 history.record=record;const value=vm.runInContext('playHistory()',history);assert.equal(typeof value.played_ms,'number');assert(Array.isArray(value.unlocked));
}
for(const record of [[],42,{played_ms:-1,unlocked:[]},{played_ms:null,unlocked:[]},{played_ms:0,unlocked:null},{played_ms:0,unlocked:[42]}]){
 history.record=record;assert.throws(()=>vm.runInContext('playHistory()',history),/Invalid browser play history/);
}
console.log('Malformed history is rejected before publication, while missing legacy fields receive defaults.');
