import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const source=readFileSync(new URL('../web-player/player.js',import.meta.url),'utf8');
const mainCalls=[],stageCalls=[],helpers=[],spriteClips=[];let time=0,allocations=0,failFinish=false;
const drawing=calls=>({save(){calls.push('save');},restore(){calls.push('restore');},fillRect(...args){time+=3;calls.push(['rect',...args]);},drawImage(...args){time+=3;calls.push(['image',...args]);},beginPath(){},arc(){},fill(){time+=3;},rect(){},clip(){}});
const main=drawing(mainCalls),stage=drawing(stageCalls),canvas={dataset:{state:'previous'}},score={textContent:'previous score'},surface={getContext:()=>stage};stage.canvas=surface;
const ui=vm.createContext({context:main,canvas,data:{custom_renderer:true},color:()=> '#fff',Uint8Array,setTimeout,performance:{now:()=>time},
 document:{querySelector:()=>score,createElement:()=>{allocations++;return surface;}},
 shaderFrame(index,...args){if(index===9)throw Error('fixture graphics failure');helpers.push(['shader',args.at(-1)]);time+=3;},
 linearBegin(target){helpers.push(['begin',target]);},linearFinish(target){helpers.push(['finish',target]);if(failFinish)throw Error('fixture batch publication failure');},linearSprite(...args){helpers.push(['sprite',args[8]]);spriteClips.push(args[10]&&Array.from(args[10]));time+=3;},
});
vm.runInContext(source.slice(source.indexOf('function render('),source.indexOf('function playAudio(')),ui);
// Exercise the old synchronous preparation as a baseline before the new entry
// exists, so this test initially fails on observable scheduling behavior.
const prepare=vm.runInContext('typeof prepareFrameCooperatively==="function"?prepareFrameCooperatively:async(frame)=>{prepareFrame(frame);return true;}',ui);
const frame={state:'42',background:0xffffffff,commands:Array.from({length:30},(_,index)=>['rect',index,0,1,1,0xffffffff])};
let heartbeats=0;
assert.equal(await prepare(frame,()=>true,()=>new Promise(resolve=>setTimeout(()=>{heartbeats++;resolve();},0)),()=>time),true);
assert.ok(heartbeats>0,'Complete offscreen rendering starved other browser tasks');
assert.equal(stageCalls.filter(call=>Array.isArray(call)&&call[0]==='rect').length,31,'Cooperation dropped drawing commands');
assert.equal(mainCalls.length,0);assert.equal(canvas.dataset.state,'previous');assert.equal(score.textContent,'previous score');assert.equal(allocations,1);
// Cancel while suspended in a filtered batch. A replacement waits for cleanup,
// then draws on the same bounded surface without publishing either early.
let live=true,resume,secondStarted=false;const before=stageCalls.length;
const filtered={state:'43',background:0xffffffff,commands:[['linear-image',0,0,10,10,1,1,new Uint8Array(4)],['linear-image',10,0,10,10,1,1,new Uint8Array(4)],...frame.commands]};
const first=prepare(filtered,()=>live,()=>new Promise(resolve=>resume=resolve),()=>time);
while(!resume)await new Promise(resolve=>setTimeout(resolve,0));
live=false;const replacement=prepare({state:'44',background:0xffffffff,commands:[['rect',100,0,1,1,0xffffffff]]},()=>{secondStarted=true;return true;},()=>Promise.resolve(),()=>time);
await Promise.resolve();assert.equal(secondStarted,false,'Replacement interleaved a suspended framebuffer');resume();
assert.equal(await first,false);assert.equal(await replacement,true);
assert.ok(stageCalls.length-before<35,'Cancelled frame kept drawing');assert.equal(mainCalls.length,0);assert.equal(allocations,1);
assert.equal(stageCalls.filter(call=>call==='save').length,stageCalls.filter(call=>call==='restore').length);
for(const [,target]of helpers)assert.equal(target,stage,'Suspended GPU drawing touched visible canvas');
ui.frame={state:'44'};vm.runInContext('publishFrame(frame)',ui);assert.equal(mainCalls.length,1);assert.equal(canvas.dataset.state,'44');
// A drawing or scheduler error must restore the context and leave the previous
// committed image intact; neither error poisons subsequent preparation.
await assert.rejects(prepare({state:'45',commands:[...frame.commands.slice(0,3),['shader',9]]},()=>true,()=>Promise.resolve(),()=>time),/fixture graphics failure/);
await assert.rejects(prepare(frame,()=>true,()=>Promise.reject(Error('fixture scheduling failure')),()=>time),/fixture scheduling failure/);
assert.equal(mainCalls.length,1);assert.equal(canvas.dataset.state,'44');
assert.equal(await prepare({state:'46',commands:[]},()=>true,()=>Promise.resolve(),()=>time),true);
assert.equal(stageCalls.filter(call=>call==='save').length,stageCalls.filter(call=>call==='restore').length);assert.equal(allocations,1);
// A suspended run keeps each replacement scissor without splitting the batch.
const clipped={state:'47',background:0xffffffff,commands:Array.from({length:24},(_,index)=>[
 ['clip',index+.5,index+.25,20.5,30.75],['linear-image',0,0,40,40,1,1,new Uint8Array(4)],['clip-reset'],
]).flat()};
const finishes=helpers.filter(([kind])=>kind==='finish').length,clipStart=spriteClips.length;
assert.equal(await prepare(clipped,()=>true,()=>Promise.resolve(),()=>time),true);
assert.equal(helpers.filter(([kind])=>kind==='finish').length-finishes,1);
assert.deepEqual(spriteClips.slice(clipStart),Array.from({length:24},(_,index)=>[index+.5,index+.25,20.5,30.75]));
assert.equal(mainCalls.length,1);assert.equal(canvas.dataset.state,'44');
failFinish=true;
await assert.rejects(prepare(clipped,()=>true,()=>Promise.resolve(),()=>time),/fixture batch publication failure/);
failFinish=false;
assert.equal(await prepare({state:'48',commands:[]},()=>true,()=>Promise.resolve(),()=>time),true);
assert.equal(stageCalls.filter(call=>call==='save').length,stageCalls.filter(call=>call==='restore').length);
assert.equal(mainCalls.length,1);assert.equal(canvas.dataset.state,'44');
console.log('Offscreen frames yield without dropping commands, serialize cancellation/replacement, restore failed draws and preserve committed pixels.');
