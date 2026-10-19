import {inflatePNG} from './png-inflate.js';
// Trusted bounded PNG path: raw 8-bit RGB/RGBA and 1/2/4/8-bit indexed, with no browser color/premultiply conversion.
const SIGNATURE=[137,80,78,71,13,10,26,10],MAX_BYTES=2097152;
const CRC=new Uint32Array(256);for(let n=0;n<256;n++){let value=n;for(let i=0;i<8;i++)value=value&1?0xedb88320^(value>>>1):value>>>1;CRC[n]=value>>>0;}
function crc(bytes,start,end){let value=0xffffffff;for(let i=start;i<end;i++)value=CRC[(value^bytes[i])&255]^(value>>>8);return (value^0xffffffff)>>>0;}
function paeth(a,b,c){const p=a+b-c,pa=Math.abs(p-a),pb=Math.abs(p-b),pc=Math.abs(p-c);return pa<=pb&&pa<=pc?a:pb<=pc?b:c;}
export async function decodeExactPNG(bytes){
 if(!(bytes instanceof Uint8Array)||!(bytes.buffer instanceof ArrayBuffer)||bytes.length<33||bytes.length>MAX_BYTES||!SIGNATURE.every((value,index)=>bytes[index]===value))throw Error('Invalid exact PNG');
 const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),width=view.getUint32(16),height=view.getUint32(20),type=bytes[25],depth=bytes[24];
 // Other valid PNG representations retain the existing exporter-provided precise pixels.
 if(![2,3,6].includes(type)||(type===3?![1,2,4,8].includes(depth):depth!==8)||bytes[28]!==0)return null;
 if(width<1||height<1||width>512||height>512||bytes[26]!==0||bytes[27]!==0)throw Error('Invalid exact PNG dimensions or encoding');
 let at=8,chunks=0,header=false,ended=false,closedIDAT=false,idat=false;const compressed=[];let palette=null,alpha=null;
 while(at<bytes.length){
  if(++chunks>2048||at+12>bytes.length)throw Error('Invalid exact PNG chunks');
  const size=view.getUint32(at),end=at+12+size;if(end>bytes.length)throw Error('Truncated exact PNG');
  const kind=String.fromCharCode(...bytes.subarray(at+4,at+8));
  if(!/^[A-Za-z]{4}$/.test(kind)||crc(bytes,at+4,end-4)!==view.getUint32(end-4))throw Error('Invalid exact PNG CRC or chunk type');
  if(!header){if(kind!=='IHDR'||size!==13)throw Error('Missing exact PNG header');header=true;}
  else if(kind==='IHDR')throw Error('Repeated exact PNG header');
  if(kind==='acTL'||kind==='tRNS'&&type!==3)return null;
  if(kind==='PLTE'){if(idat||palette||size<3||size>768||size%3)throw Error('Invalid exact PNG palette');palette=bytes.slice(at+8,end-4);}
  if(kind==='tRNS'){if(idat||alpha||!palette||size>palette.length/3)throw Error('Invalid exact PNG transparency');alpha=bytes.slice(at+8,end-4);}
  if(kind==='IDAT'){if(closedIDAT)throw Error('Nonconsecutive exact PNG input');idat=true;compressed.push(bytes.subarray(at+8,end-4));}
  else if(idat)closedIDAT=true;
  if(kind==='IEND'){if(size||!idat||end!==bytes.length)throw Error('Invalid exact PNG end');ended=true;break;}
  if((bytes[at+4]&32)===0&&!['IHDR','IDAT','IEND','PLTE'].includes(kind))throw Error('Unsupported exact PNG critical chunk');
  at=end;
 }
 if(!ended)throw Error('Incomplete exact PNG');
 if(type===3&&!palette)throw Error('Missing exact PNG palette');
 const channels=type===6?4:type===3?1:3,stride=type===3?Math.ceil(width*depth/8):width*channels,expected=(stride+1)*height;
 const raw=inflatePNG(compressed,expected);
 const pixels=new Uint8Array(width*height*4);let previous=new Uint8Array(stride),current=new Uint8Array(stride);
 for(let y=0;y<height;y++){
  const offset=y*(stride+1),filter=raw[offset];if(filter>4)throw Error('Invalid exact PNG filter');
  for(let x=0;x<stride;x++){
   const left=x>=channels?current[x-channels]:0,above=previous[x],upperLeft=x>=channels?previous[x-channels]:0;
   const prediction=filter===0?0:filter===1?left:filter===2?above:filter===3?Math.floor((left+above)/2):paeth(left,above,upperLeft);
   current[x]=(raw[offset+1+x]+prediction)&255;
  }
  for(let x=0;x<width;x++){const to=(y*width+x)*4,from=x*channels;if(type===3){const index=(current[Math.floor(x*depth/8)]>>>(8-depth-x*depth%8))&((1<<depth)-1);if(index*3+2>=palette.length)throw Error('Invalid exact PNG palette index');pixels[to]=palette[index*3];pixels[to+1]=palette[index*3+1];pixels[to+2]=palette[index*3+2];pixels[to+3]=alpha&&index<alpha.length?alpha[index]:255;}else{pixels[to]=current[from];pixels[to+1]=current[from+1];pixels[to+2]=current[from+2];pixels[to+3]=channels===4?current[from+3]:255;}}
  const swap=previous;previous=current;current=swap;
 }
 return {width,height,pixels,precise:true};
}
