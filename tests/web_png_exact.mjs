import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
const inflater=readFileSync(new URL('../web-player/png-inflate.js',import.meta.url));
const inflaterURL='data:text/javascript;base64,'+inflater.toString('base64');
const source=Buffer.from(readFileSync(new URL('../web-player/png-exact.js',import.meta.url),'utf8').replace("'./png-inflate.js'",JSON.stringify(inflaterURL)));
const {decodeExactPNG}=await import('data:text/javascript;base64,'+source.toString('base64'));
const report=JSON.parse(readFileSync(process.argv[2],'utf8'));let checked=0;
for(const image of report.images){
 const bytes=new Uint8Array(readFileSync((process.argv[3]??process.cwd()+'/ports/arkanoo/assets')+'/'+image.asset));
 const decoded=await decodeExactPNG(bytes);assert.ok(decoded,`Unsupported PNG ${image.asset}: color type ${bytes[25]}, depth ${bytes[24]}`);
 assert.equal(createHash('sha256').update(decoded.pixels).digest('hex'),image.rgba_sha256,image.asset);checked++;
 const broken=bytes.slice();broken[broken.length-1]^=1;await assert.rejects(()=>decodeExactPNG(broken),/CRC/);
 await assert.rejects(()=>decodeExactPNG(bytes.subarray(0,bytes.length-1)));
}
assert.equal(checked,76);console.log('All 76 original PNGs match independent decoded RGBA hashes; CRC and truncation reject.');
// Independent small encodings exercise every predictor, channels and malformed bounds.
const {deflateSync}=await import('node:zlib');
function crc(bytes){let value=0xffffffff;for(const byte of bytes){value^=byte;for(let i=0;i<8;i++)value=value&1?0xedb88320^(value>>>1):value>>>1;}return (value^0xffffffff)>>>0;}
function chunk(kind,data){const output=Buffer.alloc(data.length+12);output.writeUInt32BE(data.length);output.write(kind,4);output.set(data,8);output.writeUInt32BE(crc(output.subarray(4,output.length-4)),output.length-4);return output;}
function png(raw,{width=2,height=2,channels=4,depth=8,color,extra=[],split=false}={}){const header=Buffer.alloc(13);header.writeUInt32BE(width);header.writeUInt32BE(height,4);header[8]=depth;header[9]=color??(channels===4?6:channels===1?3:2);const zipped=deflateSync(raw);return new Uint8Array(Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]),chunk('IHDR',header),...extra,...(split?[chunk('IDAT',zipped.subarray(0,3)),chunk('IDAT',zipped.subarray(3))]:[chunk('IDAT',zipped)]),chunk('IEND',Buffer.alloc(0))]));}
function paeth(a,b,c){const p=a+b-c,da=Math.abs(p-a),db=Math.abs(p-b),dc=Math.abs(p-c);return da<=db&&da<=dc?a:db<=dc?b:c;}
for(const channels of [3,4])for(let filter=0;filter<5;filter++){
 const pixels=Uint8Array.from({length:4*channels},(_,i)=>(i*79+17)%256),raw=[];
 for(let y=0;y<2;y++){raw.push(filter);for(let x=0;x<2*channels;x++){const pos=y*2*channels+x,a=x>=channels?pixels[pos-channels]:0,b=y?pixels[pos-2*channels]:0,c=y&&x>=channels?pixels[pos-3*channels]:0;const predict=filter===0?0:filter===1?a:filter===2?b:filter===3?Math.floor((a+b)/2):paeth(a,b,c);raw.push((pixels[pos]-predict)&255);}}
 const decoded=await decodeExactPNG(png(Uint8Array.from(raw),{channels,split:true}));
 const expected=[];for(let i=0;i<pixels.length;i+=channels)expected.push(pixels[i],pixels[i+1],pixels[i+2],channels===4?pixels[i+3]:255);
 assert.deepEqual([...decoded.pixels],expected);
}
for(const [raw,options,pattern] of [
 [new Uint8Array(19),{},/inflated size/],
 [new Uint8Array(17),{},/Truncated/],
 [Uint8Array.of(5,...new Array(17).fill(0)),{},/filter/],
 [new Uint8Array(18),{width:513},/dimensions/],
 [new Uint8Array(18),{extra:[chunk('NOPE',Buffer.alloc(0))]},/critical/],
])await assert.rejects(()=>decodeExactPNG(png(raw,options)),pattern);
assert.deepEqual([...(await decodeExactPNG(png(new Uint8Array(14),{channels:3,extra:[chunk('tRNS',Buffer.alloc(6))]}))).pixels],new Array(16).fill(0));
assert.deepEqual([...(await decodeExactPNG(png(Uint8Array.of(0,10,20,30,10,20,31),{width:2,height:1,channels:3,extra:[chunk('tRNS',Buffer.from([0,10,0,20,0,30]))]}))).pixels],[10,20,30,0,10,20,31,255]);
console.log('All five PNG filters, RGB/RGBA expansion, split IDAT, inflation bounds, invalid filters/dimensions/chunks and exact RGB color-key transparency passed.');

const palette=[chunk('PLTE',Buffer.from([10,20,30,100,110,120])),chunk('tRNS',Buffer.from([0,127]))];
assert.deepEqual([...(await decodeExactPNG(png(Uint8Array.of(0,0,1,0,1,0),{channels:1,extra:palette}))).pixels],[10,20,30,0,100,110,120,127,100,110,120,127,10,20,30,0]);
await assert.rejects(()=>decodeExactPNG(png(Uint8Array.of(0,0,2,0,1,0),{channels:1,extra:palette})),/palette index/);
await assert.rejects(()=>decodeExactPNG(png(new Uint8Array(6),{channels:1})),/palette/);
console.log('Indexed palette, exact transparency, missing palette and out-of-range indices passed.');

for(const depth of [1,2,4]){
 const row0=1<<(8-2*depth),row1=1<<(8-depth);
 const decoded=await decodeExactPNG(png(Uint8Array.of(0,row0,0,row1),{channels:1,depth,extra:palette}));
 assert.deepEqual([...decoded.pixels],[10,20,30,0,100,110,120,127,100,110,120,127,10,20,30,0]);
}
console.log('Packed 1/2/4-bit palette rows preserve exact colors and transparency.');
// W3C PNG: low-bit grayscale samples expand before display, while tRNS
// compares the original unscaled sample. Partial rows keep their padding.
for(const depth of [1,2,4,8])for(const filter of [0,1,2,3,4]){
 const width=5,height=2,mask=(1<<depth)-1,stride=Math.ceil(width*depth/8),rows=[],samples=[];
 for(let y=0;y<height;y++){
  const row=new Uint8Array(stride);
  for(let x=0;x<width;x++){const value=(x+y)&mask;row[Math.floor(x*depth/8)]|=value<<(8-depth-x*depth%8);samples.push(value);}
  rows.push(row);
 }
 const raw=[];
 for(let y=0;y<height;y++){raw.push(filter);for(let x=0;x<stride;x++){const a=x?rows[y][x-1]:0,b=y?rows[y-1][x]:0,c=y&&x?rows[y-1][x-1]:0,p=filter===0?0:filter===1?a:filter===2?b:filter===3?Math.floor((a+b)/2):paeth(a,b,c);raw.push((rows[y][x]-p)&255);}}
 const key=Buffer.from([0,1]);
 const decoded=await decodeExactPNG(png(Uint8Array.from(raw),{width,height,color:0,depth,extra:[chunk('tRNS',key)],split:true}));
 assert.deepEqual([...decoded.pixels],samples.flatMap(sample=>[sample*255/mask,sample*255/mask,sample*255/mask,sample===1?0:255]));
 const opaque=await decodeExactPNG(png(Uint8Array.from(raw),{width,height,color:0,depth}));
 assert.deepEqual([...opaque.pixels],samples.flatMap(sample=>[sample*255/mask,sample*255/mask,sample*255/mask,255]));
}
for(const filter of [0,1,2,3,4]){
 const pixels=Uint8Array.from([0,0,64,127,100,255,230,1]),raw=[];
 for(let y=0;y<2;y++){raw.push(filter);for(let x=0;x<4;x++){const at=y*4+x,a=x>=2?pixels[at-2]:0,b=y?pixels[at-4]:0,c=y&&x>=2?pixels[at-6]:0,p=filter===0?0:filter===1?a:filter===2?b:filter===3?Math.floor((a+b)/2):paeth(a,b,c);raw.push((pixels[at]-p)&255);}}
 assert.deepEqual([...(await decodeExactPNG(png(Uint8Array.from(raw),{color:4}))).pixels],[0,0,0,0,64,64,64,127,100,100,100,255,230,230,230,1]);
}
assert.equal(await decodeExactPNG(png(Uint8Array.of(0,0,1),{width:1,height:1,color:0,depth:16})),null);
for(const options of [
 {color:0,extra:[chunk('tRNS',Buffer.from([0]))]},
 {color:0,depth:1,extra:[chunk('tRNS',Buffer.from([0,2]))]},
 {color:2,extra:[chunk('tRNS',Buffer.from([1,0,0,0,0,0]))]},
 {color:4,extra:[chunk('tRNS',Buffer.alloc(2))]},
 {color:6,extra:[chunk('tRNS',Buffer.alloc(6))]},
 {color:0,extra:[chunk('PLTE',Buffer.alloc(3))]},
 {color:0,extra:[chunk('tRNS',Buffer.alloc(2)),chunk('tRNS',Buffer.alloc(2))]},
])await assert.rejects(()=>decodeExactPNG(png(new Uint8Array(6),options)),/transparency|palette/);
console.log('All grayscale depths, gray/alpha, color-key bounds, row padding and five predictors preserve exact straight RGBA; malformed metadata rejects.');

const {inflatePNG}=await import(inflaterURL);
const {constants,inflateSync}=await import('node:zlib');
let seed=0x12345678;
function random(){seed^=seed<<13;seed^=seed>>>17;seed^=seed<<5;return seed>>>0;}
for(const strategy of [constants.Z_DEFAULT_STRATEGY,constants.Z_FIXED,constants.Z_HUFFMAN_ONLY,constants.Z_RLE])for(const level of [0,1,6,9])for(const size of [1,17,257,65537]){
 const raw=Buffer.alloc(size);for(let i=0;i<size;i++)raw[i]=strategy===constants.Z_RLE?(i%71?19:random()&255):random()&255;
 const compressed=deflateSync(raw,{level,strategy}),parts=[];for(let at=0;at<compressed.length;at+=113)parts.push(compressed.subarray(at,at+113));
 assert.deepEqual(Buffer.from(inflatePNG(parts,size)),raw);
 if(size>1)assert.throws(()=>inflatePNG(parts,size-1),/inflated size/);
 const damaged=compressed.slice();damaged[damaged.length-1]^=1;assert.throws(()=>inflatePNG([damaged],size),/Adler/);
}
const bomb=deflateSync(Buffer.alloc(4*1024*1024));assert.throws(()=>inflatePNG([bomb],16),/inflated size/);
const original=deflateSync(Buffer.from('bounded PNG inflater corruption regression '.repeat(100)));
for(let index=0;index<original.length;index++){
 const mutation=original.slice();mutation[index]^=0x41;
 let actual;try{actual=inflatePNG([mutation],4200);}catch{continue;}
 assert.deepEqual(Buffer.from(actual),inflateSync(mutation));
}
console.log('Bounded inflater: 64 independent stored/fixed/dynamic/RLE encodings match zlib; undersized output, bad Adler, 4 MiB expansion and byte corruption reject without growing output.');

assert.throws(()=>inflatePNG([Uint8Array.of(0x78,0x01,0x07,0,0,0,1)],1),/Reserved/);
assert.throws(()=>inflatePNG([Uint8Array.of(0x78,0x01,0x01,2,0,0,0,0,0,0,1)],1),/stored/);
assert.throws(()=>inflatePNG([new Uint8Array(new SharedArrayBuffer(8))],1),/input/);
assert.throws(()=>inflatePNG([new Uint8Array(2097153)],1),/input/);
