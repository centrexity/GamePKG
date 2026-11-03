import {PackageAssets} from './asset-store.js';
import {RecordedSession} from './recorded-session.js';
async function companion(name,expected,signal){
 if(typeof expected!=='string'||!/^[a-f0-9]{64}$/.test(expected))throw Error('Missing audio companion integrity hash');
 const response=await fetch(name,{cache:'no-store',signal});if(!response.ok||!response.body)throw Error('Missing audio companion');
 const reader=response.body.getReader(),chunks=[];let length=0;
 try{for(;;){const {done,value}=await reader.read();if(done)break;length+=value.length;if(length>4194304)throw Error('Audio companion exceeds size limit');chunks.push(value);}}finally{await reader.cancel();reader.releaseLock();}
 if(length<8)throw Error('Invalid audio companion');const bytes=new Uint8Array(length);let at=0;for(const chunk of chunks){bytes.set(chunk,at);at+=chunk.length;}
 const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),v=>v.toString(16).padStart(2,'0')).join('');if(hash!==expected)throw Error('Audio companion integrity check failed');return bytes;
}
export async function startRecordedPlayer(data,parent,signal){
 const info=data.recorded_audio;if(!info)throw Error('Missing recorded audio companions');
 const controller=new AbortController(),abort=()=>controller.abort();if(signal?.aborted)abort();signal?.addEventListener('abort',abort,{once:true});const timer=setTimeout(abort,5000);
 try{
  const [converter,decoder]=await Promise.all([companion('audio-converter.wasm',info.converter_sha256,controller.signal),data.manifest.limits.music_memory_bytes?companion('audio-decoder.wasm',info.decoder_sha256,controller.signal):Promise.resolve(null)]);
  const assets=new PackageAssets(data.assets,data.manifest);
  if(assets.scratchBytes!==(data.asset_scratch_bytes??0))throw Error('Invalid package asset scratch allowance');
  return await RecordedSession.open(data.manifest,assets,converter,decoder,parent,controller.signal);
 }catch(error){controller.abort();throw error;}finally{clearTimeout(timer);signal?.removeEventListener('abort',abort);}
}
