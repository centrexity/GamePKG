import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
// Native GL and Vulkan scissor endpoints round independently. Canvas path
// antialiasing must not halve text alpha at a fractional clipping boundary.
const draws=[];const stack=[];let active=null;let path=null;
const context={
 save(){stack.push(active);},restore(){active=stack.pop();},
 beginPath(){path=null;},rect(...rect){path=rect;},clip(){active=path;},
 fillRect(...rect){draws.push({rect,clip:active});},
};
const player=readFileSync(new URL('../web-player/player.js',import.meta.url),'utf8');
const ui=vm.createContext({context,color:value=>value>>>0,canvas:{dataset:{}},data:{custom_renderer:true},document:{querySelector:()=>({textContent:''})}});
vm.runInContext(player.slice(player.indexOf('function render('),player.indexOf('function playAudio(')),ui);
vm.runInContext(`render({background:0xff,state:'0',commands:[
 ['clip',5,80,168.5,15.5],['rect',0,0,640,480,0xffffffff],
 ['clip',10.49,10.51,.02,.98],['rect',0,0,640,480,0xffffffff],
 ['clip',1.5,2.5,0,0],['rect',0,0,640,480,0xffffffff],
 ['clip-reset'],['rect',0,0,640,480,0xffffffff]
]});`,ui);
assert.deepEqual(draws.slice(1).map(v=>v.clip&&Array.from(v.clip)),[[5,80,169,16],[10,11,1,0],[2,3,0,0],null]);
assert.equal(active,null);assert.equal(stack.length,0);
vm.runInContext(`render({background:0xff,state:'1',commands:[['rect',0,0,640,480,0xffffffff]]});`,ui);
assert.equal(draws.at(-1).clip,null);
console.log('Browser clipping matches rounded native scissors, including fractional glyph edges, empty clips, replacement, reset and fresh frames.');
