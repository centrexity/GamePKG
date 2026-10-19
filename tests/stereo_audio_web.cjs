// Exercise the production browser submission and output functions without a device.
const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
const worker=fs.readFileSync('web-player/worker.js','utf8');
const submit=worker.slice(worker.indexOf('function audioSubmit('),worker.indexOf('\nconst api='));
const context=vm.createContext({assert,DataView,Uint8Array});
vm.runInContext(`let manifest={permissions:{audio:true}},audioSamples=[],audioChannels=1;
const memory=new Uint8Array(3200);const view=new DataView(memory.buffer);
view.setInt16(0,32767,true);view.setInt16(2,-32768,true);
function bytes(p,n,max){return Number.isInteger(p)&&p>=0&&n<=max&&p+n<=memory.length?memory.subarray(p,p+n):null;}
${submit}
assert.equal(audioSubmit(0,3,2),-1);
assert.equal(audioSubmit(-1,2,2),-1);
assert.equal(audioSubmit(3199,2,2),-1);
assert.equal(audioSamples.length,0);
assert.equal(audioSubmit(0,0,1),0);
assert.equal(audioSubmit(0,2,2),0);
assert.equal(audioSamples[0],32767);assert.equal(audioSamples[1],-32768);
assert.equal(audioChannels,2);
assert.equal(audioSubmit(0,2,1),-1);
assert.equal(audioSubmit(0,1598,2),0);
assert.equal(audioSamples.length,1600);
assert.equal(audioSubmit(0,2,2),-1);
audioSamples=[];audioChannels=1;manifest.permissions.audio=false;
assert.equal(audioSubmit(0,2,2),-1);assert.equal(audioSamples.length,0);
`,context);
const player=fs.readFileSync('web-player/player.js','utf8');
const play=player.slice(player.indexOf('function playAudio('),player.indexOf('\n',player.indexOf('function playAudio(')));
vm.runInContext(`let audioNext=0,audioNodes=new Set(),audioGain={},canvas={dataset:{}},buffers=[],starts=[];
let audioContext={state:'running',currentTime:0,
createBuffer(channels,frames,rate){assert.equal(rate,48000);const planes=Array.from({length:channels},()=>new Float32Array(frames));const b={planes,getChannelData:c=>planes[c]};buffers.push(b);return b;},
createBufferSource(){return {connect(){},start(t){starts.push(t);}};}};
${play}
playAudio([32767,-32768,16384,-16384],2);
assert.equal(buffers[0].planes.length,2);
assert.equal(buffers[0].planes[0][0],32767/32768);
assert.equal(buffers[0].planes[0][1],.5);
assert.equal(buffers[0].planes[1][0],-1);
assert.equal(buffers[0].planes[1][1],-.5);
assert.equal(audioNext,2/48000);
playAudio([8192,-8192]);assert.equal(buffers[1].planes.length,1);
assert.equal(starts[1],2/48000);assert.equal(audioNext,4/48000);
for(const [samples,channels] of [[[1],2],[[1,2],3],[new Array(1602).fill(0),2]])playAudio(samples,channels);
assert.equal(buffers.length,2);
`,vm.createContext({assert,Float32Array}));
console.log('Browser stereo: independent channels, frame duration, permissions, memory and shared budget passed');
