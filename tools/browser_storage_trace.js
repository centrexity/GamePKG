// Private qualification observer: preserve IndexedDB requests and their errors.
(() => {
 const entries=[];
 window.progressTrace=entries;
 window.noteProgress=entry=>{
  entries.push({...entry,at:performance.now()});
  if(entries.length>32)entries.shift();
 };
 const open=IDBFactory.prototype.open;
 let serial=0;
 IDBFactory.prototype.open=function(...args){
  const requestId=++serial,started=performance.now();
  window.noteProgress({operation:'open',requestId});
  try{
   const request=open.apply(this,args);
   for(const event of ['upgradeneeded','blocked','success','error']){
    request.addEventListener(event,()=>{
     const error=event==='error'?request.error:null;
     window.noteProgress({operation:event,requestId,elapsed:performance.now()-started,
      error:error?String(error).slice(0,512):null});
    });
   }
   return request;
  }catch(error){
   window.noteProgress({operation:'open-throw',requestId,elapsed:performance.now()-started,
    error:String(error).slice(0,512)});
   throw error;
  }
 };
})();
