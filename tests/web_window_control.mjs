import assert from 'node:assert/strict';import {readFileSync} from 'node:fs';import vm from 'node:vm';
const worker=vm.createContext({TextEncoder,TextDecoder});vm.runInContext(readFileSync(new URL('../web-player/worker.js',import.meta.url),'utf8'),worker);vm.runInContext('manifest={permissions:{input:true}};',worker);
const api=vm.runInContext('({begin:windowBegin,request:windowRequest,finish:windowFinish,state:()=>windowCurrent,canonical})',worker);
api.begin(false);assert.equal(api.request(0,0),1);assert.equal(api.request(1,1),1);assert.equal(api.request(2,0),0);assert.equal(api.request(3,0),0);assert.equal(api.state().visible,true);api.finish(false);assert.equal(api.state().visible,true);
api.begin(true);api.request(0,0);api.request(1,1);api.request(2,0);api.finish(true);assert.equal(api.state().fullscreen_toggle,true);api.finish(false);assert.equal(api.state().visible,true);
api.begin(true);api.request(2,0);api.request(2,0);api.finish(true);assert.equal(api.state().fullscreen_toggle,false);api.begin(true);assert.equal(api.canonical['window-control'](3,0),1);api.finish(true);assert.equal(api.state().closed,true);assert.equal(api.request(0,1),0);
vm.runInContext('manifest.permissions.input=false;windowStaged=defaultWindow();',worker);assert.equal(api.request(0,0),0);
async function settle(){for(let i=0;i<8;i++)await Promise.resolve();}
const handlers=new Map(),documentHandlers=new Map(),windowHandlers=new Map(),timers=[];let now=1000,locks=0,exits=0,fullscreen=0,fullExits=0,stopped='';
function listen(map,name,fn){const values=map.get(name)||[];values.push(fn);map.set(name,values);}
const buttons=new Map(['#capture','#fullscreen'].map(name=>[name,{hidden:true}]));let focused=true;
const document={activeElement:null,pointerLockElement:null,fullscreenElement:null,hasFocus:()=>focused,querySelector:name=>buttons.get(name),addEventListener:(name,fn)=>listen(documentHandlers,name,fn),exitPointerLock(){exits++;this.pointerLockElement=null;},async exitFullscreen(){fullExits++;this.fullscreenElement=null;}};
const canvas={style:{},focus(){document.activeElement=this;},getBoundingClientRect:()=>({left:100,top:50,width:1280,height:960}),addEventListener:(name,fn)=>listen(handlers,name,fn),async requestPointerLock(){locks++;document.pointerLockElement=canvas;},async requestFullscreen(){fullscreen++;document.fullscreenElement=canvas;},setPointerCapture(){},hasPointerCapture:()=>false};document.activeElement=canvas;
const ui=vm.createContext({canvas,document,running:true,filePending:null,performance:{now:()=>now},setTimeout:fn=>timers.push(fn),resetKeyboard(){},halt:message=>stopped=message,window:{addEventListener:(name,fn)=>listen(windowHandlers,name,fn)}});
const source=readFileSync(new URL('../web-player/player.js',import.meta.url),'utf8');vm.runInContext(source.slice(source.indexOf('// Host-owned window controls'),source.indexOf('// Native browser file selection')),ui);vm.runInContext(source.slice(source.indexOf('// Ordered input')),ui);
const control=vm.runInContext('({apply:applyWindowControls,release:releaseWindowForFile,reset:resetWindowControls,position:location,events:()=>surfaceEvents,clear:()=>{surfaceEvents=[];}})',ui);
const desired={visible:false,confined:true,fullscreen_toggle:false,closed:false};assert.equal(control.apply(desired),true);await settle();assert.equal(locks,1);assert.equal(canvas.style.cursor,'none');
assert.deepEqual(Array.from(control.position({movementX:200,movementY:-100},true)),[420,190]);control.position({movementX:100000,movementY:-100000},true);assert.deepEqual(Array.from(control.position({movementX:0,movementY:0},true)),[640,0]);
assert.deepEqual(Array.from(control.position({movementX:-100,movementY:100})),[640,0],'button event integrated movement a second time');
control.apply({...desired,visible:true,confined:false});assert.equal(document.pointerLockElement,null);assert.equal(canvas.style.cursor,'auto');assert.equal(exits,1);
control.apply({...desired,visible:true,confined:false,fullscreen_toggle:true});await settle();assert.equal(fullscreen,1);document.fullscreenElement=null;control.apply({...desired,visible:true,confined:false,fullscreen_toggle:false});await settle();assert.equal(fullscreen,2,'toggle used stale desired fullscreen state');
control.release();assert.equal(document.fullscreenElement,null);assert.equal(fullExits,1);
control.apply(desired);await buttons.get('#capture').onclick();await settle();control.clear();document.pointerLockElement=null;for(const fn of documentHandlers.get('pointerlockchange'))fn();for(const fn of timers.splice(0))fn();assert.deepEqual(Array.from(control.events(),e=>e[0]),[5,6,9,10]);
for(const fn of handlers.get('keydown'))fn({code:'Escape',key:'Escape',repeat:false});assert.equal(control.events().length,4,'browser Escape was delivered twice');
control.clear();now+=200;for(const fn of handlers.get('keydown'))fn({code:'Escape',key:'Escape',ctrlKey:true,altKey:true,preventDefault(){}});assert.equal(control.events().length,0,'emergency release leaked Escape into the game');
control.reset();vm.runInContext('running=true;filePending=null;',ui);let resolveLock;
canvas.requestPointerLock=()=>new Promise(resolve=>{resolveLock=()=>{document.pointerLockElement=canvas;resolve();};});
control.apply(desired);assert.ok(resolveLock);control.reset();resolveLock();await settle();assert.equal(document.pointerLockElement,null,'stale capture survived game reset');
control.apply({...desired,closed:true});assert.equal(stopped,'Game closed');
console.log('Browser window controls preserve tick rollback, scoped capture, relative-to-absolute motion, fullscreen parity and Escape release.');

// Homepage requests carry only a serial; the player selects the declared URL.
vm.runInContext('globalThis.URL=globalThis.testURL;manifest={permissions:{input:true},homepage:"https://example.org/project"};windowCurrent=defaultWindow();windowPrevious=defaultWindow();windowStaged=defaultWindow();',Object.assign(worker,{testURL:URL}));
api.begin(false);assert.equal(api.request(4,0),0);
api.begin(true);assert.equal(api.request(4,1),0);assert.equal(api.request(4,0),1);assert.equal(api.request(4,0),0);api.finish(false);assert.equal(api.state().homepage_request,0);
api.begin(true);assert.equal(api.request(4,0),1);api.finish(true);assert.equal(api.state().homepage_request,1);
for(const url of ['javascript:alert(1)','file:///tmp/game','http://example.org/','https://user:password@example.org/']){
 worker.badURL=url;vm.runInContext('manifest.homepage=badURL;',worker);api.begin(true);assert.equal(api.request(4,0),0);
}
const opened=[];ui.URL=URL;ui.TextEncoder=TextEncoder;ui.data={manifest:{homepage:'https://example.org/project'}};ui.navigator={userActivation:{isActive:true}};ui.window.open=(...args)=>opened.push(args);
focused=true;document.activeElement=canvas;vm.runInContext('running=true;filePending=null;',ui);control.reset();
const homepage={visible:true,confined:false,fullscreen_toggle:false,closed:false,homepage_request:1};
control.apply(homepage);control.apply(homepage);assert.deepEqual(opened,[['https://example.org/project','_blank','noopener,noreferrer']]);
ui.navigator.userActivation.isActive=false;control.apply({...homepage,homepage_request:2});assert.equal(opened.length,1);
ui.navigator.userActivation.isActive=true;ui.data.manifest.homepage='javascript:alert(1)';control.apply({...homepage,homepage_request:3});assert.equal(opened.length,1);
ui.data.manifest.homepage='https://example.org/project';focused=false;control.apply({...homepage,homepage_request:4});assert.equal(opened.length,1);
focused=true;control.apply({...homepage,homepage_request:4});assert.equal(opened.length,1,'A suppressed background request resurfaced after focus');
console.log('Homepage requests require a fresh gesture, valid declared URL and successful tick; browser navigation is deduplicated and isolated.');
vm.runInContext('homepageEpoch=1;lastHomepageRequest=0;',ui);
control.apply({...homepage,homepage_request:5,homepage_epoch:0});assert.equal(opened.length,1,'Old pre-restart request navigated');
control.apply({...homepage,homepage_request:1,homepage_epoch:1});assert.equal(opened.length,2,'New post-restart click was suppressed');
console.log('Restart generations discard old homepage requests and accept fresh post-restart clicks.');
control.apply({...homepage,homepage_request:2,homepage_epoch:1},false);assert.equal(opened.length,2,'Failed persistence navigated');
control.apply({...homepage,homepage_request:2,homepage_epoch:1},true);assert.equal(opened.length,2,'Failed request was delayed until persistence recovered');
control.apply({...homepage,homepage_request:3,homepage_epoch:1},true);assert.equal(opened.length,3,'Fresh request after persistence recovery was suppressed');
console.log('Failed persistence suppresses and consumes homepage navigation; fresh recovery clicks remain available.');
