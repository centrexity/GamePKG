import assert from 'node:assert/strict';
import fs from 'node:fs';
const {originalEffectPCM}=await import('data:text/javascript;base64,'+fs.readFileSync(new URL('../web-player/recorded-wav.js',import.meta.url)).toString('base64'));
function wav(size){const bytes=new Uint8Array(size+44),view=new DataView(bytes.buffer);for(const [at,text] of [[0,'RIFF'],[8,'WAVEfmt '],[36,'data']])bytes.set(new TextEncoder().encode(text),at);for(const [at,n] of [[4,size+36],[16,16],[24,44100],[28,176400],[40,size]])view.setUint32(at,n,true);for(const [at,n] of [[20,1],[22,2],[32,4],[34,16]])view.setUint16(at,n,true);for(let i=0;i<size;i++)bytes[i+44]=i%256;return bytes;}
const source=wav(4096),pcm=originalEffectPCM(source);assert.deepEqual(pcm,source.subarray(44));assert.equal(pcm.buffer,source.buffer,'parser unexpectedly allocated a decoded duplicate');
for(let at=0;at<44;at++){const bad=source.slice();bad[at]^=1;assert.throws(()=>originalEffectPCM(bad),`header byte ${at}`);}
for(const size of [0,1,2,3,5,2097156])assert.throws(()=>originalEffectPCM(wav(size)));
assert.equal(originalEffectPCM(wav(4)).length,4);assert.equal(originalEffectPCM(wav(2097152)).length,2097152);
const offset=new Uint8Array(source.length+5);offset.set(source,5);assert.deepEqual(originalEffectPCM(offset.subarray(5)),pcm);
assert.throws(()=>originalEffectPCM(source.subarray(0,-1)));assert.throws(()=>originalEffectPCM(new Int16Array(24)));assert.throws(()=>originalEffectPCM(new Uint8Array(new SharedArrayBuffer(48))));
console.log('Canonical effect WAV: exact LE stereo PCM view, all 44 header mutations, size/alignment limits, offset views and invalid backing stores passed');
