// Run the shipped keyboard implementation, without a browser or guest wrappers.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
const source=readFileSync(new URL('../web-player/player.js',import.meta.url),'utf8');
const context=vm.createContext({document:{querySelector:()=>({getContext:()=>({})})}});
vm.runInContext(source.slice(source.indexOf('const canvas='),source.indexOf('function color(')),context);
const keyboard=vm.runInContext(`({down:keyboardDown,up:keyboardUp,reset:resetKeyboard,
 tick:()=>{const value=pressed|((held&255)<<8)|((held&0x000f0000)<<4);pressed=0;return value;}})`,context);
const event=(key,code,repeat=false)=>({key,code,repeat,preventDefault(){}});
keyboard.down(event('ArrowLeft','ArrowLeft'));assert.equal(keyboard.tick(),1|256);
keyboard.down(event('ArrowLeft','ArrowLeft',true));assert.equal(keyboard.tick(),256);
keyboard.down(event('a','KeyA'));assert.equal(keyboard.tick(),256);
keyboard.up(event('ArrowLeft','ArrowLeft'));assert.equal(keyboard.tick(),256);
keyboard.up(event('A','KeyA'));assert.equal(keyboard.tick(),0);
keyboard.down(event('x','KeyX'));keyboard.up(event('x','KeyX'));assert.equal(keyboard.tick(),32);
assert.equal(keyboard.tick(),0);
keyboard.down(event('c','KeyC'));assert.equal(keyboard.tick(),65536|1048576);
keyboard.reset();assert.equal(keyboard.tick(),0);
keyboard.down(event('c','KeyC',true));assert.equal(keyboard.tick(),0);
console.log('Browser keyboard preserves physical edges, shared-key holds, quick taps and action held bits.');

keyboard.reset();let prevented=false;keyboard.down({key:'v',code:'KeyV',ctrlKey:true,preventDefault(){prevented=true;}});assert.equal(keyboard.tick(),0);assert.equal(prevented,false,'Ctrl+V must remain available to the focused canvas paste handler');
