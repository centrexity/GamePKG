// Exercise the shipped imports, including unsigned 64-bit canonical ABI values.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
let calls=0;
const crypto={getRandomValues(words){
 assert.equal(words.length,2);
 words[0]=0x89abcdef;words[1]=0xfedcba98;calls++;
 return words;
}};
const context=vm.createContext({crypto,TextEncoder,TextDecoder});
vm.runInContext(readFileSync(new URL('../web-player/worker.js',import.meta.url),'utf8'),context);
const worker=vm.runInContext('({api,canonical})',context);
assert.equal(worker.api.random_u64(),0xfedcba9889abcdefn);
assert.equal(worker.canonical['random-u64'](),0xfedcba9889abcdefn);
assert.equal(calls,2);
crypto.getRandomValues=()=>{throw new Error('entropy unavailable');};
assert.throws(()=>worker.api.random_u64(),/entropy unavailable/);
assert.throws(()=>worker.canonical['random-u64'](),/entropy unavailable/);
console.log('Browser randomness preserves all 64 bits and fails without entropy.');
