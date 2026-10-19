// Trusted worker: no guest imports, network requests, files, or device handles.
import {RecordedDecoder,MAX_PCM_VALUES} from './recorded-decoder.js';
let decoder=null,compiled=false,opened=false;
self.onmessage=({data})=>{
 const id=data?.id;
 try{
  if(!Number.isSafeInteger(id)||id<1)throw Error('Invalid decoder request');
  switch(data.kind){
   case 'compile':
    if(compiled)throw Error('Decoder already compiled');
    decoder=new RecordedDecoder(data.bytes,data.limit);compiled=true;
    self.postMessage({id,ok:true,maximumMemoryBytes:decoder.maximumMemoryBytes});return;
   case 'reserve':if(!compiled||opened)throw Error('Decoder cannot reserve');decoder.beginInput(data.length);break;
   case 'input':
    if(!compiled||opened||!(data.bytes instanceof Uint8Array)||!(data.bytes.buffer instanceof ArrayBuffer)||data.bytes.byteOffset!==0||data.bytes.buffer.byteLength!==data.bytes.byteLength)throw Error('Decoder cannot accept input');
    decoder.writeInput(data.offset,data.bytes);data.bytes=null;break;
   case 'open':
    if(!compiled||opened||data.bytes!==undefined)throw Error('Decoder cannot open');
    decoder.finishInput();opened=true;break;
   case 'read':{
    if(!opened||!Number.isInteger(data.length)||data.length<2||data.length>MAX_PCM_VALUES||data.length%2)throw Error('Invalid decoder read');
    const output=new Int16Array(data.length),count=decoder.read(output),values=output.slice(0,count);
    self.postMessage({id,ok:true,values},[values.buffer]);return;
   }
   case 'rewind':if(!opened)throw Error('Decoder is not open');decoder.rewind();break;
   default:throw Error('Unknown decoder operation');
  }
  self.postMessage({id,ok:true});
 }catch(error){decoder?.close();decoder=null;self.postMessage({id,ok:false,error:String(error.message||error).slice(0,512)});self.close();}
};
self.postMessage({ready:true});
