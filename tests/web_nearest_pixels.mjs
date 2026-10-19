import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const draws=[];
const context={save(){},restore(){},fillRect(...rect){draws.push({rect,color:this.fillStyle});}};
const ui=vm.createContext({context,color:value=>value>>>0,canvas:{dataset:{}},data:{custom_renderer:true},document:{querySelector:()=>({textContent:''})}});
const player=readFileSync(new URL('../web-player/player.js',import.meta.url),'utf8');
vm.runInContext(player.slice(player.indexOf('function render('),player.indexOf('function playAudio(')),ui);
vm.runInContext(`render({background:null,state:'0',commands:[
 ['image',20,100,600.5,.5,1,1,new Uint8Array([80,80,100,255])],
 ['image',20.5,100.5,.5,100.5,1,1,new Uint8Array([80,80,100,255])],
 ['image',-.25,-.25,1,1,1,1,new Uint8Array([255,0,0,128])],
 ['image',12.5,13.5,0,0,1,1,new Uint8Array([0,0,0,0])]
]});`,ui);
assert.deepEqual(draws.slice(1).map(v=>v.rect.map(value=>value===0?0:value)),[[20,100,601,1],[21,101,0,100],[0,0,1,1],[13,14,0,0]]);
assert.equal(draws[1].color,0x505064ff);
assert.equal(draws[3].color,0xff000080);
assert.equal(draws[4].color,0);
console.log('Single-texel nearest images use native pixel-center coverage without fractional edge blending.');

vm.runInContext(`render({background:null,state:'1',commands:[
 ['image',0,30,.5,1,1,1,new Uint8Array([255,0,0,255])],
 ['image',.5,30,.5,1,1,1,new Uint8Array([0,255,0,255])]
]});`,ui);
assert.deepEqual(draws.slice(-2).map(v=>v.rect.map(value=>value===0?0:value)),[[0,30,1,1],[1,30,0,1]]);
console.log('Adjacent half-pixel strips share the nearest sampler tie rule without overlapping.');
