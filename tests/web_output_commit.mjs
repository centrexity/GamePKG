import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const source=readFileSync(new URL('../web-player/player.js',import.meta.url),'utf8');
const mainCalls=[],stagedCalls=[],helpers=[];
function drawing(calls){return {save(){calls.push('save');},restore(){calls.push('restore');},fillRect(){calls.push('fill');},drawImage(...args){calls.push(['image',...args]);},beginPath(){},arc(){},fill(){},rect(){},clip(){}};}
const main=drawing(mainCalls),stage=drawing(stagedCalls),canvas={dataset:{state:'previous'}},score={textContent:'previous score'};
const stageCanvas={getContext:()=>stage};stage.canvas=stageCanvas;
let allocations=0;
const ui=vm.createContext({context:main,canvas,score,data:{custom_renderer:true},color:()=> '#fff',Uint8Array,
 document:{querySelector:()=>score,createElement:()=>{allocations++;return stageCanvas;}},
 shaderFrame(...args){if(args[0]===9)throw Error('fixture graphics failure');helpers.push(['shader',args.at(-1)]);},
 linearSprite(...args){helpers.push(['linear',args.at(-1)]);},
});
vm.runInContext(source.slice(source.indexOf('function render('),source.indexOf('function playAudio(')),ui);
ui.frame={state:'42',background:0xffffffff,commands:[['rect',0,0,10,10,0xffffffff],['shader',0],['shader-image',1,2,1,new Uint8Array(8)],['linear-image',0,0,10,10,1,1,new Uint8Array(4)]]};
vm.runInContext('prepareFrame(frame)',ui);
assert.equal(mainCalls.length,0,'preparation touched the visible canvas');
assert(stagedCalls.includes('fill'));
assert.equal(canvas.dataset.state,'previous');assert.equal(score.textContent,'previous score');
assert.equal(allocations,1);assert.equal(stageCanvas.width,640);assert.equal(stageCanvas.height,480);
assert.equal(helpers.length,3);for(const [,target] of helpers)assert.equal(target,stage,'GPU draw used the visible framebuffer');
vm.runInContext('prepareFrame(frame)',ui);assert.equal(allocations,1,'staging surface grew per frame');
vm.runInContext('publishFrame(frame)',ui);
assert.deepEqual(mainCalls,[['image',stageCanvas,0,0]]);assert.equal(canvas.dataset.state,'42');assert.equal(score.textContent,'');
ui.frame={state:'99',background:0xffffffff,commands:[['shader',9]]};
assert.throws(()=>vm.runInContext('prepareFrame(frame)',ui),/fixture graphics failure/);
assert.equal(mainCalls.length,1);assert.equal(canvas.dataset.state,'42');
assert.equal(stagedCalls.filter(v=>v==='save').length,stagedCalls.filter(v=>v==='restore').length);
const stopped=[];
const audio=vm.createContext({audioNodes:new Set([{stop(){stopped.push(1);}},{stop(){stopped.push(2);throw Error('already ended');}},{stop(){stopped.push(3);}}]),audioNext:123});
vm.runInContext(source.slice(source.indexOf('function clearAudio('),source.indexOf('function halt(')),audio);
vm.runInContext('clearAudio();clearAudio()',audio);
assert.deepEqual(stopped,[1,2,3]);assert.equal(audio.audioNodes.size,0);assert.equal(audio.audioNext,0);
console.log('Browser frames stage once in a bounded surface, keep GPU sampling offscreen, publish metadata only on commit, and cancel queued audio despite ended-node errors.');
