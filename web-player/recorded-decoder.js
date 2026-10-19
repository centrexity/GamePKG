// Trusted decoder adapter. Run in a separately supervised worker, never in a guest.
export const MAX_DECODER_MEMORY=6291456,MAX_ENCODED_BYTES=8388608,MAX_PCM_VALUES=1600,MAX_DECODE_WORK=524288;
const fail=message=>{throw Error(message);};
function layout(bytes,limit){
 if(!(bytes instanceof Uint8Array)||bytes.length<8||bytes.length>4194304||!Number.isInteger(limit)||limit<65536||limit>MAX_DECODER_MEMORY)fail('Invalid decoder module or memory budget');
 if(![0,97,115,109,1,0,0,0].every((value,index)=>bytes[index]===value))fail('Invalid decoder header');
 let at=8,end=bytes.length,memory=0;const types=[],functions=[],exports=new Map();
 const byte=()=>{if(at>=end)fail('Truncated decoder section');return bytes[at++];};
 const uint=()=>{let value=0;for(let shift=0;shift<=28;shift+=7){const next=byte();if(shift===28&&next>15)fail('Invalid decoder integer');value+=(next&127)*2**shift;if(next<128)return value;}fail('Invalid decoder integer');};
 const bounded=maximum=>{if(uint()!==1)fail('Decoder resources must have a fixed maximum');const minimum=uint(),max=uint();if(minimum>max||max>maximum)fail('Decoder resource budget exceeded');return max;};
 while(at<bytes.length){end=bytes.length;const section=byte(),length=uint(),stop=at+length;if(stop>bytes.length)fail('Truncated decoder module');end=stop;
  if(section===1){const count=uint();if(count>4096)fail('Too many decoder types');for(let i=0;i<count;i++){if(byte()!==96)fail('Invalid decoder function type');const params=uint();if(params>16)fail('Too many decoder parameters');const parameters=[];for(let n=0;n<params;n++)parameters.push(byte());const count=uint();if(count>16)fail('Too many decoder results');const results=[];for(let n=0;n<count;n++)results.push(byte());types.push({parameters,results});}if(at!==end)fail('Invalid decoder types');}
  if(section===2){if(uint()!==0||at!==end)fail('Decoder cannot import capabilities');}
  if(section===3){const count=uint();if(count>65536)fail('Too many decoder functions');for(let n=0;n<count;n++)functions.push(uint());if(at!==end)fail('Invalid decoder functions');}
  if(section===7){const count=uint();if(count>4096)fail('Too many decoder exports');for(let n=0;n<count;n++){const size=uint();if(size>1024||at+size>end)fail('Invalid decoder export name');const name=new TextDecoder('utf-8',{fatal:true}).decode(bytes.subarray(at,at+size));at+=size;if(exports.has(name))fail('Duplicate decoder export');exports.set(name,{kind:byte(),index:uint()});}if(at!==end)fail('Invalid decoder exports');}
  if(section===5){if(memory||uint()!==1)fail('Decoder must have one memory');memory=bounded(Math.floor(limit/65536))*65536;if(!memory||at!==end)fail('Invalid decoder memory');}
  if(section===4){if(uint()!==1||byte()!==112)fail('Invalid decoder table');bounded(4096);if(at!==end)fail('Invalid decoder table');}
  if(section===8)fail('Decoder start functions are forbidden');
  at=stop;
 }
 if(!memory)fail('Decoder memory missing');return {memory,types,functions,exports};
}
export function inspectCompanion(bytes,limit=MAX_DECODER_MEMORY){return layout(bytes,limit).memory;}
const ABI={decoder_reserve:1,decoder_open:0,decoder_read:1,decoder_output:0,decoder_rate:0,decoder_channels:0,decoder_work:0,decoder_rewind:0,decoder_set_output_rate:1,decoder_set_mixer_rate:1};
export class RecordedDecoder {
 #api;#maximum;#opened=false;#failed=false;#closed=false;
 constructor(bytes,limit=MAX_DECODER_MEMORY){
  const size=bytes?.byteLength;if(!(bytes instanceof Uint8Array)||!Number.isInteger(size)||size<8||size>4194304)fail('Invalid decoder module');
  const owned=new Uint8Array(size);owned.set(new Uint8Array(bytes.buffer,bytes.byteOffset,size));
  const policy=layout(owned,limit);this.#maximum=policy.memory;
  for(const [name,count] of Object.entries(ABI)){
   const exported=policy.exports.get(name),type=exported?.kind===0?policy.types[policy.functions[exported.index]]:null;
   if(!type||type.parameters.length!==count||type.parameters.some(value=>value!==127)||type.results.length!==1||type.results[0]!==127)fail('Invalid decoder ABI');
  }
  const memory=policy.exports.get('memory');if(memory?.kind!==2||memory.index!==0)fail('Invalid decoder memory export');
  const module=new WebAssembly.Module(owned);
  if(WebAssembly.Module.imports(module).length)fail('Decoder cannot import capabilities');
  const exports=WebAssembly.Module.exports(module);
  if(exports.filter(value=>value.kind==='memory').length!==1||!exports.some(value=>value.name==='memory'&&value.kind==='memory'))fail('Decoder memory export missing');
  for(const name of Object.keys(ABI))if(!exports.some(value=>value.name===name&&value.kind==='function'))fail('Decoder function missing');
  const api=new WebAssembly.Instance(module,{}).exports;
  for(const [name,parameters] of Object.entries(ABI))if(api[name].length!==parameters)fail('Invalid decoder signature');
  this.#api=api;this.#memory();
 }
 get maximumMemoryBytes(){return this.#maximum;}
 get failed(){return this.#failed;}
 #memory(){const memory=this.#api?.memory;if(!(memory instanceof WebAssembly.Memory)||memory.buffer.byteLength>this.#maximum)fail('Decoder memory budget exceeded');return memory.buffer;}
 #call(operation){try{return operation();}catch(error){this.#failed=true;throw error;}}
 #work(){const work=this.#api.decoder_work();if(!Number.isInteger(work)||work<0||work>MAX_DECODE_WORK)fail('Decoder work limit exceeded');}
 #ready(){if(this.#closed||this.#failed||!this.#opened)fail('Decoder is not active');}
 open(encoded){
  if(this.#closed||this.#failed||this.#opened)fail('Decoder cannot be reopened');
  const length=encoded?.byteLength;if(!(encoded instanceof Uint8Array)||!Number.isInteger(length)||length<1||length>MAX_ENCODED_BYTES)fail('Invalid encoded music size');
  this.#opened=true;
  this.#call(()=>{const source=new Uint8Array(encoded.buffer,encoded.byteOffset,length),pointer=this.#api.decoder_reserve(length),memory=this.#memory();
   if(!Number.isInteger(pointer)||pointer<=0||pointer+length>memory.byteLength)fail('Invalid decoder input range');
   new Uint8Array(memory,pointer,length).set(source);
   if(this.#api.decoder_open()!==0)fail('Invalid or over-budget MPEG audio');this.#work();
   if(this.#api.decoder_set_mixer_rate(44100)!==0||this.#api.decoder_rate()!==44100||this.#api.decoder_channels()!==2)fail('Invalid decoder mixer format');this.#memory();
  });
 }
 read(output){
  this.#ready();
  if(!(output instanceof Int16Array)||output.length<2||output.length>MAX_PCM_VALUES||output.length%2)fail('Invalid PCM output buffer');
  return this.#call(()=>{const count=this.#api.decoder_read(output.length);this.#work();const memory=this.#memory();
   if(!Number.isInteger(count)||count<0||count>output.length||count%2)fail('Invalid decoded sample count');
   if(count===0)return 0;
   const pointer=this.#api.decoder_output();if(!Number.isInteger(pointer)||pointer<0||pointer+count*2>memory.byteLength)fail('Invalid decoder output range');
   const view=new DataView(memory,pointer,count*2);for(let index=0;index<count;index++)output[index]=view.getInt16(index*2,true);return count;
  });
 }
 rewind(){this.#ready();this.#call(()=>{if(this.#api.decoder_rewind()!==0)fail('Decoder rewind failed');this.#work();this.#memory();});}
 close(){this.#closed=true;this.#api=null;}
}
