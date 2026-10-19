// Trusted bounded zlib/DEFLATE decoder. Output capacity is fixed before decoding.
export const MAX_INFLATED_BYTES=1049088;
export const INFLATE_SCRATCH_BYTES=263168;
export class AssetInflater {
 #workspace;#maximum;
 constructor(maximum){
  if(!Number.isInteger(maximum)||maximum<1||maximum>MAX_INFLATED_BYTES)throw Error('Invalid inflation capacity');
  this.#maximum=maximum;this.#workspace={output:new Uint8Array(maximum),literalTable:new Uint32Array(32768),distanceTable:new Uint32Array(32768),counts:new Uint16Array(16),next:new Uint16Array(16),lengths:new Uint8Array(320),codeLengths:new Uint8Array(19),lengthBase:new Uint16Array([3,4,5,6,7,8,9,10,11,13,15,17,19,23,27,31,35,43,51,59,67,83,99,115,131,163,195,227,258]),lengthExtra:new Uint8Array([0,0,0,0,0,0,0,0,1,1,1,1,2,2,2,2,3,3,3,3,4,4,4,4,5,5,5,5,0]),distanceBase:new Uint16Array([1,2,3,4,5,7,9,13,17,25,33,49,65,97,129,193,257,385,513,769,1025,1537,2049,3073,4097,6145,8193,12289,16385,24577]),distanceExtra:new Uint8Array([0,0,0,0,1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,10,10,11,11,12,12,13,13]),permutation:new Uint8Array([16,17,18,0,8,7,9,6,10,5,11,4,12,3,13,2,14,1,15])};
 }
 get reservedBytes(){return this.#maximum+INFLATE_SCRATCH_BYTES;}
 get backingBytes(){return this.#workspace?Object.values(this.#workspace).reduce((sum,bytes)=>sum+bytes.byteLength,0):0;}
 // Borrowed output: the trusted owner must finish using it before decoding again.
 decode(input,expected){
  if(!this.#workspace||!(input instanceof Uint8Array)||!(input.buffer instanceof ArrayBuffer)||input.length<6||input.length>2097152||!Number.isInteger(expected)||expected<1||expected>this.#maximum)throw Error('Invalid compressed asset input');
  return inflate(input,expected,this.#workspace);
 }
 close(){this.#workspace=null;}
}
export function inflatePNG(parts,expected){
 if(!Array.isArray(parts)||parts.length>2048||!Number.isInteger(expected)||expected<1||expected>MAX_INFLATED_BYTES)throw Error('Invalid PNG inflation capacity');
 let length=0;for(const part of parts){if(!(part instanceof Uint8Array)||!(part.buffer instanceof ArrayBuffer)||(length+=part.length)>2097152)throw Error('Invalid PNG compressed input');}
 if(length<6)throw Error('Truncated PNG zlib input');
 const input=new Uint8Array(length);let copied=0;for(const part of parts){input.set(part,copied);copied+=part.length;}
 return new AssetInflater(expected).decode(input,expected);
}
function inflate(input,expected,workspace){
 if((input[0]&15)!==8||(input[0]>>>4)>7||((input[0]<<8)|input[1])%31||input[1]&32)throw Error('Unsupported compressed asset zlib header');
 const window=1<<(8+(input[0]>>>4));
 const length=input.length,end=length-4,output=workspace.output.subarray(0,expected);let at=2,word=0,bits=0,written=0;
 const {literalTable,distanceTable,counts,next,lengths,codeLengths,lengthBase,lengthExtra,distanceBase,distanceExtra,permutation}=workspace;
 function take(count){while(bits<count){if(at>=end)throw Error('Truncated compressed asset DEFLATE');word|=input[at++]<<bits;bits+=8;}const value=word&((1<<count)-1);word>>>=count;bits-=count;return value;}
 function tree(sizes,table){
  counts.fill(0);let maximum=0;for(const size of sizes){if(size>15)throw Error('Invalid compressed asset Huffman length');if(size){counts[size]++;maximum=Math.max(maximum,size);}}
  let available=1,code=0;for(let size=1;size<=15;size++){available=(available<<1)-counts[size];if(available<0)throw Error('Oversubscribed compressed asset Huffman tree');code=(code+counts[size-1])<<1;next[size]=code;}
  const size=1<<maximum;table.fill(0,0,size);
  for(let symbol=0;symbol<sizes.length;symbol++){const count=sizes[symbol];if(!count)continue;let value=next[count]++,reverse=0;for(let i=0;i<count;i++){reverse=(reverse<<1)|(value&1);value>>>=1;}for(let i=reverse;i<size;i+=1<<count)table[i]=(count<<16)|(symbol+1);}
  return maximum;
 }
 function symbol(table,maximum){
  while(bits<maximum&&at<end){word|=input[at++]<<bits;bits+=8;}
  const entry=table[word&((1<<maximum)-1)],count=entry>>>16;if(!entry||count>bits)throw Error('Invalid compressed asset Huffman symbol');word>>>=count;bits-=count;return (entry&65535)-1;
 }
 let final=false;
 while(!final){
  final=take(1)!==0;const kind=take(2);
  if(kind===0){
   const skip=bits%8;word>>>=skip;bits-=skip;const count=take(16),inverse=take(16);if((count^inverse)!==65535)throw Error('Invalid stored compressed asset block');if(written+count>expected)throw Error('Exact compressed asset inflated size exceeded');for(let i=0;i<count;i++)output[written++]=take(8);continue;
  }
  let literalMaximum,distanceMaximum;
  if(kind===1){for(let i=0;i<288;i++)lengths[i]=i<144?8:i<256?9:i<280?7:8;literalMaximum=tree(lengths.subarray(0,288),literalTable);lengths.fill(5,0,32);distanceMaximum=tree(lengths.subarray(0,32),distanceTable);}
  else if(kind===2){
   const literalCount=take(5)+257,distanceCount=take(5)+1,codeCount=take(4)+4;if(literalCount>286)throw Error('Invalid compressed asset literal count');
   // RFC order is deliberately explicit; no callback or unbounded decoder state.
   codeLengths.fill(0);for(let i=0;i<codeCount;i++)codeLengths[permutation[i]]=take(3);
   const codeMaximum=tree(codeLengths,literalTable);if(!codeMaximum)throw Error('Empty compressed asset code tree');let count=0;const total=literalCount+distanceCount;
   while(count<total){const value=symbol(literalTable,codeMaximum);if(value<16){lengths[count++]=value;continue;}let repeat,fill;if(value===16){if(!count)throw Error('Invalid compressed asset repeat');repeat=take(2)+3;fill=lengths[count-1];}else if(value===17){repeat=take(3)+3;fill=0;}else if(value===18){repeat=take(7)+11;fill=0;}else throw Error('Invalid compressed asset code repeat');if(count+repeat>total)throw Error('compressed asset code repeat exceeded');lengths.fill(fill,count,count+repeat);count+=repeat;}
   if(!lengths[256])throw Error('Missing compressed asset end code');literalMaximum=tree(lengths.subarray(0,literalCount),literalTable);distanceMaximum=tree(lengths.subarray(literalCount,total),distanceTable);
  }else throw Error('Reserved compressed asset DEFLATE block');
  for(;;){
   const value=symbol(literalTable,literalMaximum);if(value===256)break;
   if(value<256){if(written===expected)throw Error('Exact compressed asset inflated size exceeded');output[written++]=value;continue;}
   if(value<257||value>285)throw Error('Invalid compressed asset length symbol');const index=value-257,count=lengthBase[index]+take(lengthExtra[index]),distanceCode=symbol(distanceTable,distanceMaximum);
   if(distanceCode>29)throw Error('Invalid compressed asset distance symbol');const distance=distanceBase[distanceCode]+take(distanceExtra[distanceCode]);
   if(distance>written||distance>window)throw Error('Invalid compressed asset back reference');if(written+count>expected)throw Error('Exact compressed asset inflated size exceeded');
   for(let i=0;i<count;i++){output[written]=output[written-distance];written++;}
  }
 }
 if(written!==expected)throw Error('Truncated exact compressed asset scanlines');
 if(Math.ceil((at*8-bits)/8)!==end)throw Error('Trailing compressed asset DEFLATE data');
 let a=1,b=0;for(const byte of output){a=(a+byte)%65521;b=(b+a)%65521;}
 const checksum=new DataView(input.buffer,input.byteOffset,input.byteLength).getUint32(end);if((((b<<16)|a)>>>0)!==checksum)throw Error('Invalid compressed asset Adler checksum');
 return output;
}
