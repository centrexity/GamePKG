import assert from 'node:assert/strict';
import fs from 'node:fs';
const {RecordedBudget}=await import('data:text/javascript;base64,'+fs.readFileSync(new URL('../web-player/recorded-budget.js',import.meta.url)).toString('base64'));
const parent=new RecordedBudget(16000),guest=parent.reserve(6000),music=parent.partition(10000),decoder=music.reserve(9000),transport=music.reserve(1000);
assert.throws(()=>parent.reserve(1));assert.throws(()=>music.reserve(1));music.close();assert.equal(parent.used,16000);decoder.release();decoder.release();assert.equal(music.used,1000);assert.equal(parent.used,16000);assert.throws(()=>music.reserve(1));transport.release();assert.equal(parent.used,6000);guest.release();assert.equal(parent.used,0);
for(const invalid of [0,-1,NaN,1.5,Number.MAX_SAFE_INTEGER+1])assert.throws(()=>new RecordedBudget(invalid));
parent.close();assert.throws(()=>parent.reserve(1));
console.log('Guest, decoder and transport partitions share one allowance; retained leases prevent early capacity reuse');
