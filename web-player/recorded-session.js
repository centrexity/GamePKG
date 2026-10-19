import {PackageAssets} from './asset-store.js';
// Trusted lifetime owner for one recorded-audio game session.
import {RecordedBudget} from './recorded-budget.js';
import {RecordedOutputClient} from './recorded-output-client.js';
import {RecordedMusicSender} from './recorded-transport.js';
import {RecordedMusicLoader} from './recorded-music-loader.js';
import {RecordedFrameDispatcher} from './recorded-frame-dispatcher.js';
const CREATE=Symbol();
export class RecordedSession {
 #parent;#guest;#guestBytes;#assetLease=null;#assets;#worker=null;#effects=null;#music=null;#output=null;#dispatcher=null;#closed=false;#closing=null;
 constructor(token,parent,guestBytes,scratch,assets){if(token!==CREATE)throw Error('Use RecordedSession.open');this.#parent=parent;this.#guestBytes=guestBytes;this.#assets=assets;try{this.#guest=parent.reserve(guestBytes+scratch);if(scratch)this.#assetLease=parent.reserve(scratch);}catch(error){this.#guest?.release();this.#assetLease?.release();throw error;}}
 static async open(manifest,assets,converter,decoder,parent,signal){
  if(!(parent instanceof RecordedBudget)||!(assets instanceof Map)||manifest?.permissions?.audio!==true)throw Error('Recorded audio requires its declared permission and memory allowance');
  if(signal!==undefined&&!(signal instanceof AbortSignal))throw Error('Invalid recorded session cancellation');if(signal?.aborted)throw Error('Recorded session startup cancelled');
  const limits=manifest.limits,effects=limits?.effects_memory_bytes??0,music=limits?.music_memory_bytes??0;
  for(const [share,max] of [[effects,4194304],[music,16777216]])if(!Number.isSafeInteger(share)||share<0||share>max||(share>0&&share<65536))throw Error('Invalid declared audio share');
  if(!effects&&!music)throw Error('No recorded audio share');
  if(!Number.isSafeInteger(limits.hard_memory_bytes)||parent.limit>limits.hard_memory_bytes||parent.limit-effects-music<65536)throw Error('Audio shares exceed the selected memory ceiling');
  if(!(converter instanceof Uint8Array)||converter.length<8||converter.length>4194304)throw Error('Invalid audio converter');
  if(assets instanceof PackageAssets&&assets.closed)throw Error('Package assets are closed');
  const scratch=assets instanceof PackageAssets?assets.scratchBytes:0,guestBytes=parent.limit-effects-music-2*scratch;
  if(guestBytes<65536)throw Error('Compressed assets leave insufficient guest memory');
  const session=new RecordedSession(CREATE,parent,guestBytes,scratch,assets);
  const abort=()=>{void session.close().catch(()=>{});};signal?.addEventListener('abort',abort,{once:true});
  try{
   if(effects)session.#effects=parent.partition(effects);if(music)session.#music=parent.partition(music);
   const outputParent=session.#effects||session.#music,outputBytes=effects?effects-converter.length*3:327680;
   const output=await RecordedOutputClient.open(converter,outputBytes,outputParent,signal);if(session.#closed||signal?.aborted){await output.close();throw Error('Recorded session startup cancelled');}session.#output=output;
   let loader=null;
   if(music){const channel=new MessageChannel(),sender=new RecordedMusicSender(channel.port1,session.#music);try{if(!await session.#output.connectMusic(sender,channel.port2))throw Error('Music output connection failed');}catch(error){sender.close();channel.port2.close();throw error;}loader=new RecordedMusicLoader(sender,decoder,session.#music);}
   if(session.#closed||signal?.aborted)throw Error('Recorded session startup cancelled');
   session.#dispatcher=new RecordedFrameDispatcher(session.#output,assets,{effects:effects>0,music:music>0,musicLoader:loader});return session;
  }catch(error){await session.close();throw error;}finally{signal?.removeEventListener('abort',abort);}
 }
 get guestBytes(){return this.#guestBytes;}
 get closed(){return this.#closed;}
 attachGuest(worker){if(this.#closed||this.#worker||typeof worker?.terminate!=='function')return false;this.#worker=worker;return true;}
 volume(value){return !this.#closed&&this.#output.volume(value);}
 resume(){return this.#closed?Promise.reject(Error('Recorded session closed')):this.#output.resume();}
 async snapshot(){if(this.#closed)throw Error('Recorded session closed');const state=await this.#output.request('snapshot');if(!state.ok)throw Error('Recorded output unavailable');return {...state,ready:true};}
 prepare(frame){return this.#closed?Promise.resolve(false):this.#dispatcher.prepare(frame);}
 finish(epoch,serial,success){return this.#closed?Promise.resolve(false):this.#dispatcher.finish(epoch,serial,success);}
 close(){
  if(this.#closing)return this.#closing;this.#closed=true;this.#worker?.terminate();this.#worker=null;this.#guest?.release();this.#guest=null;
  this.#closing=(async()=>{try{if(this.#dispatcher)await this.#dispatcher.close();else if(this.#output)await this.#output.close();}finally{try{if(this.#assets instanceof PackageAssets)await this.#assets.close();}finally{this.#assetLease?.release();this.#assetLease=null;this.#effects?.close();this.#music?.close();}}})();return this.#closing;
 }
}
