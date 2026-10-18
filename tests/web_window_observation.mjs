import assert from 'node:assert/strict';import {readFileSync} from 'node:fs';import vm from 'node:vm';
const context=vm.createContext({TextEncoder,TextDecoder});vm.runInContext(readFileSync(new URL('../web-player/worker.js',import.meta.url),'utf8'),context);
const api=vm.runInContext('manifest={permissions:{input:true}};({read:api.window_state,component:canonical["window-state"],observe:windowObserve,begin:windowBegin,request:windowRequest,finish:windowFinish,deny:()=>manifest.permissions.input=false,allow:()=>manifest.permissions.input=true})',context);
assert.equal(api.read(),0);
for(const [value,expected] of [[true,3],[false,1],[null,0],[undefined,0],[1,0],['true',0],[{},0],[true,3]]){api.observe(value);for(let n=0;n<64;n++){assert.equal(api.read(),expected);assert.equal(api.component(),expected);}}
api.begin(true);assert.equal(api.request(2,0),1);api.finish(true);assert.equal(api.read(),3,'accepted toggle predicted an unobserved physical change');
api.observe(false);assert.equal(api.read(),1);api.deny();assert.equal(api.read(),0);api.allow();assert.equal(api.read(),1);
api.begin(true);assert.equal(api.request(3,0),1);api.finish(false);assert.equal(api.read(),1,'failed close erased a real observation');api.begin(true);api.request(3,0);api.finish(true);assert.equal(api.read(),0,'closed game retained observation access');
console.log('Window observation uses only actual boolean feedback, stays stable across queries/toggles, retains rollback state and respects input/closed permissions.');
