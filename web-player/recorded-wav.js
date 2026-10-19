// Same canonical, original-rate effect format accepted by the trusted desktop host.
export function originalEffectPCM(bytes){
 const length=bytes?.byteLength;
 if(!(bytes instanceof Uint8Array)||!(bytes.buffer instanceof ArrayBuffer)||!Number.isInteger(length)||length<48||length>2097152+44)throw Error('Invalid effect WAV length');
 const view=new DataView(bytes.buffer,bytes.byteOffset,length),word=at=>view.getUint32(at,true);
 const text=(at,value)=>[...value].every((c,i)=>bytes[at+i]===c.charCodeAt(0));
 if(!text(0,'RIFF')||!text(8,'WAVEfmt ')||word(4)!==length-8||word(16)!==16||view.getUint16(20,true)!==1||view.getUint16(22,true)!==2||word(24)!==44100||word(28)!==176400||view.getUint16(32,true)!==4||view.getUint16(34,true)!==16||!text(36,'data')||word(40)!==length-44||(length-44)%4)throw Error('Effects require canonical 44100 Hz stereo S16 WAV');
 return bytes.subarray(44);
}
