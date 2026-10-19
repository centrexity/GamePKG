import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const source=readFileSync(new URL('../web-player/progress-store.js',import.meta.url),'utf8');
const {normalizeSave,normalizeHistory,ProgressDataError,ProgressStore}=await import('data:text/javascript;base64,'+Buffer.from(source).toString('base64'));
for(const value of [[],[0,255],new Uint8Array([3,7])]){
 const copy=normalizeSave(value);assert(copy instanceof Uint8Array);assert.deepEqual(Array.from(copy),Array.from(value));assert.notEqual(copy,value);
}
for(const value of [null,{},[256],[-1],[1.5],[NaN],new Array(2),new Uint8Array(1048577)])assert.throws(()=>normalizeSave(value),ProgressDataError);
const maximum=normalizeSave(new Uint8Array(1048576));assert.equal(maximum.length,1048576);
for(const value of [{},{played_ms:123,unlocked:['legacy-achievement']},{played_ms:Number.MAX_SAFE_INTEGER,unlocked:[]}]){
 const copy=normalizeHistory(value);assert(Number.isSafeInteger(copy.played_ms));assert(Array.isArray(copy.unlocked));assert.notEqual(copy,value);
}
for(const value of [null,[],42,{played_ms:-1},{played_ms:1.5},{played_ms:Infinity},{played_ms:Number.MAX_SAFE_INTEGER+1},{played_ms:null},{unlocked:null},{unlocked:[42]},{unlocked:['../bad']},{unlocked:['x','x']},{extra:1},{unlocked:Array.from({length:1025},(_,i)=>'a'+i)}])assert.throws(()=>normalizeHistory(value),ProgressDataError);
assert.throws(()=>normalizeHistory({unlocked:new Array(1)}),ProgressDataError);
for(const record of [{},{played_ms:0},{unlocked:[]}])assert.throws(()=>normalizeHistory(record,false),ProgressDataError);
const originals={played_ms:1,unlocked:['original']},copied=normalizeHistory(originals);copied.unlocked.push('new');assert.deepEqual(originals.unlocked,['original']);
console.log('Browser progress validation preserves exact bounded bytes, legacy defaults, immutable ownership and safe integer/achievement/history limits.');

const descriptor=Object.getOwnPropertyDescriptor(globalThis,'localStorage');
Object.defineProperty(globalThis,'localStorage',{configurable:true,get(){throw Error('legacy storage denied');}});
try{assert.doesNotThrow(()=>new ProgressStore('gamepkg:save:fixture:creator:1','gamepkg:history:fixture:creator',{}),'constructing a database client read unused legacy storage');}
finally{if(descriptor)Object.defineProperty(globalThis,'localStorage',descriptor);else delete globalThis.localStorage;}
