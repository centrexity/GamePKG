import assert from 'node:assert/strict';import {readFileSync} from 'node:fs';import vm from 'node:vm';
const messages=[];const context=vm.createContext({TextEncoder,TextDecoder,Uint8Array,postMessage:value=>messages.push(value)});vm.runInContext(readFileSync(new URL('../web-player/worker.js',import.meta.url),'utf8'),context);
vm.runInContext(`manifest={permissions:{input:true,files:[{id:'patterns',title:'Pattern',extensions:['txt'],max_bytes:1024,read:true,write:true}]}};instance={exports:{memory:{buffer:new ArrayBuffer(65536)},cabi_realloc:()=>2048}};`,context);
const service=vm.runInContext(`({api,canonical,name:fileNameAllowed,policy:()=>filePolicy(0),begin:()=>{eventBytes=encodeEvents([[9,0,0,105]]);fileTickBegin();},finish:fileTickFinish,result:fileResult,entry:()=>fileEntry,view:()=>new DataView(instance.exports.memory.buffer),call:fileCall})`,context);
const zero=new Uint8Array(4);assert.equal(service.api.file_call(0,0,4,64,4),-1);service.begin();assert.equal(service.api.file_call(0,0,4,65535,4),-1);assert.equal(service.api.file_call(0,0,4,64,4),4);assert.equal(messages.length,0);service.finish(false);assert.equal(service.entry(),null);
service.begin();service.canonical['file-call'](0,0,4,4,128);assert.equal(service.view().getInt32(128,true),4);assert.equal(service.view().getUint32(2048,true),2);service.finish(true);assert.equal(messages.length,1);
const job=messages[0];service.result({handle:99,generation:job.generation,status:2,name:'safe.txt',bytes:new Uint8Array([1])});assert.equal(service.entry().status,1);
service.result({handle:job.handle,generation:job.generation,status:2,name:'café.TXT',bytes:new Uint8Array([1,2,3])});assert.equal(service.entry().status,2);
const request=new Uint8Array(8);new DataView(request.buffer).setUint32(0,job.handle,true);new DataView(request.buffer).setUint32(4,1,true);assert.deepEqual(Array.from(service.call(4,request,10)),[2,3]);
assert.equal(service.call(3,request.subarray(0,4),2),null);assert.equal(new TextDecoder().decode(service.call(3,request.subarray(0,4),256)),'café.TXT');assert.equal(service.call(5,request.subarray(0,4),0).length,0);
assert.equal(service.call(0,zero,4),null);service.begin();assert.equal(service.call(0,zero,3),null);assert.equal(service.call(0,zero,4).length,4);service.finish(true);service.result({handle:service.entry().handle,generation:service.entry().generation,status:2,name:'bad.exe',bytes:new Uint8Array([1])});assert.equal(service.entry().status,5);
for(const name of ['../x.txt','a/b.txt','a\\b.txt','CON.txt','con .txt','CONIN$.txt','LPT¹.txt','NUL.txt','x.txt:stream','x?.txt','x\u0085.txt'])assert.equal(service.name(name,service.policy()),false,name);
for(const name of ['.hidden.txt','café😀.TXT','COM10.txt','safe%2Fname.txt'])assert.equal(service.name(name,service.policy()),true,name);
console.log('Browser file requests preserve gestures, permissions, transaction boundaries, opaque handles, names and chunk limits.');

const playerSource=readFileSync(new URL('../web-player/player.js',import.meta.url),'utf8');const ui=vm.createContext({TextEncoder});vm.runInContext(playerSource.slice(playerSource.indexOf('function chosenFileName'),playerSource.indexOf('function showFileRequest')),ui);const check=vm.runInContext('chosenFileName',ui);
for(const name of ['../x.txt','CON.txt','con .txt','CONIN$.txt','LPT¹.txt','café😀.TXT','.hidden.txt','safe%2Fname.txt'])assert.equal(check(name,['txt']),service.name(name,service.policy()),name);

vm.runInContext('fileEntry=null;fileGesture=true;manifest.permissions.files[0].read="yes";',context);assert.equal(service.call(0,zero,4),null);
vm.runInContext('manifest.permissions.files[0].read=true;manifest.permissions.files.push({...manifest.permissions.files[0]});',context);assert.equal(service.call(0,zero,4),null);
vm.runInContext('manifest.permissions.files.pop();manifest.permissions.files[0].max_bytes=1048577;',context);assert.equal(service.call(0,zero,4),null);
vm.runInContext('manifest.permissions.files[0].max_bytes=1024;manifest.permissions.input=false;',context);assert.equal(service.call(0,zero,4),null);

// Exercise the real UI's optional browser Save-picker path, including a stale
// picker resolving after Stop/Restart. These stubs grant no filesystem access.
const posted=[];let writes=[],selectedName='selected.txt',savePicker=async()=>({name:selectedName,createWritable:async()=>({write:async bytes=>writes.push(Array.from(bytes)),close:async()=>{}})});
const box={hidden:true,children:[],replaceChildren(){this.children=[];},append(node){this.children.push(node);}};
const dom={querySelector:()=>box,createElement:tag=>({tag,textContent:'',setAttribute(){}})};const active={postMessage:value=>posted.push(value)};
const uiHost=vm.createContext({TextEncoder,Uint8Array,document:dom,window:{showSaveFilePicker:(...args)=>savePicker(...args)},canvas:{focus(){}},worker:active,running:true,data:{manifest:{id:'test',permissions:{files:[{title:'Pattern',extensions:['txt'],max_bytes:1024,read:true,write:true}]}}}});
vm.runInContext('let filePending=null;'+playerSource.slice(playerSource.indexOf('// Native browser file selection'),playerSource.indexOf('// Ordered input')),uiHost);
const display=vm.runInContext('showFileRequest',uiHost);const saveRequest={handle:1,generation:1,index:0,operation:1,name:'suggested.txt',bytes:new Uint8Array([1,2,3])};
display(saveRequest,active);await box.children.find(n=>n.textContent.startsWith('Save ')).onclick();assert.deepEqual(writes,[[1,2,3]]);assert.equal(posted.at(-1).status,3);assert.equal(box.hidden,true);
writes=[];posted.length=0;selectedName='forbidden.exe';display({...saveRequest,handle:2},active);await box.children.find(n=>n.textContent.startsWith('Save ')).onclick();assert.deepEqual(writes,[]);assert.equal(posted[0].status,5);
posted.length=0;savePicker=async()=>{const error=new Error('cancel');error.name='AbortError';throw error;};display({...saveRequest,handle:3},active);await box.children.find(n=>n.textContent.startsWith('Save ')).onclick();assert.equal(posted[0].status,4);
let resolvePicker;savePicker=()=>new Promise(resolve=>resolvePicker=resolve);posted.length=0;display({...saveRequest,handle:4},active);const pending=box.children.find(n=>n.textContent.startsWith('Save ')).onclick();vm.runInContext('clearFileUI();running=false;',uiHost);resolvePicker({name:'selected.txt',createWritable:async()=>{throw new Error('stale picker wrote a file');}});await pending;assert.equal(posted.length,0);
console.log('Browser picker writes only the chosen permitted file and ignores cancelled/stale results.');
