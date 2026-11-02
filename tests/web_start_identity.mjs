import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';

const source=readFileSync(new URL('../web-player/player.js',import.meta.url),'utf8');
const start=source.slice(source.indexOf('async function start(){'),source.indexOf("canvas.addEventListener('keydown',keyboardDown)"));
for(const recorded of [false,true])for(const blocked of ['progress','closing']){
 const pending=new Promise(()=>{}),canvas={dataset:{state:'previous-frame'}};
 let loads=0;
 const context=vm.createContext({
  canvas,data:{manifest:{permissions:{audio:recorded},limits:recorded?{effects_memory_bytes:1}:{}}},roms:[],
  play:{disabled:false},restart:{disabled:false},stop:{disabled:true},status:{},
  startSerial:0,running:true,audioClosing:blocked==='closing'?pending:Promise.resolve(),stagedRendering:Promise.resolve(),
  progressStore:{load(){loads++;return blocked==='progress'?pending:Promise.resolve({save:new Uint8Array()});}},
 });
 vm.runInContext("function halt(){startSerial++;running=false;}"+start+'\nvoid start();',context);
 assert.equal(loads,1,'The actual startup must reach its asynchronous load');
 assert.equal(canvas.dataset.state,undefined,`${recorded?'recorded':'ordinary'} startup retained stale frame identity while awaiting ${blocked}`);
 assert.equal(context.play.disabled,true);
 assert.equal(context.restart.disabled,true);
}
console.log('Startup invalidates the previous frame before pending progress/audio shutdown for ordinary and recorded sessions.');
