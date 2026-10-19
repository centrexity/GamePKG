// Fractional fixed-step time must survive integer browser timer quantization.
const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
const source=fs.readFileSync('web-player/player.js','utf8');
const line=source.split('\n').find(v=>v.startsWith('function scheduleTick('));assert(line);
vm.runInNewContext(`let nextTickAt=0,frameTimer,clock=0,delay=0;const performance={now:()=>clock};function tick(){}function clearTimeout(value){assert(value===undefined||value===1);}function setTimeout(fn,ms){assert.equal(fn,tick);delay=ms;return 1;}
${line}
for(let frame=0;frame<600;frame++){clock+=.2;scheduleTick();assert(delay>=0);clock+=Math.floor(delay);}
assert(Math.abs(clock-10000)<1,clock);
clock+=500;scheduleTick();assert.equal(delay,0);
clock+=.2;scheduleTick();assert(delay>15&&delay<17,delay);
`,{assert,Math});
console.log('Browser fixed-step clock: 600 quantized ticks retain fractional time; late frames do not create a catch-up burst.');
