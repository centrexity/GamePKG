// Test-only, read-only observation of the real worker's frame acknowledgments.
(() => {
 const commits=[];globalThis.homepageFrameCommits=commits;
 const NativeWorker=globalThis.Worker;let nextId=0;
 globalThis.Worker=class extends NativeWorker {
  constructor(...args){
   super(...args);this.homepageProbeId=++nextId;this.homepageProbeFrames=new Map();
   this.addEventListener('message',({data})=>{
    if(data?.type==='frame'&&data.window)this.homepageProbeFrames.set(data.epoch+':'+data.serial,{request:data.window.homepage_request,epoch:data.epoch,serial:data.serial,worker:this.homepageProbeId});
   });
  }
  postMessage(message,...args){
   if(message?.type==='window-commit'){
    const key=message.epoch+':'+message.serial,frame=this.homepageProbeFrames.get(key);
    this.homepageProbeFrames.delete(key);
    if(frame){commits.push({...frame,success:message.success});if(commits.length>256)commits.shift();}
   }
   return super.postMessage(message,...args);
  }
 };
})();
