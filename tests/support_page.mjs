import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const script = readFileSync(new URL('../html/support/support.js', import.meta.url), 'utf8');
const {copyAddress, renderSupport} = await import(`data:text/javascript;base64,${Buffer.from(script).toString('base64')}`);
const details = JSON.parse(readFileSync(new URL('../html/support/details.json', import.meta.url)));
assert.equal(details.mining_url, 'https://simoleons.net/group/gamepkg');
const copied = [];
const clipboard = {writeText: async value => copied.push(value)};
assert.equal(await copyAddress('', clipboard), false);
assert.deepEqual(copied, []);
assert.equal(await copyAddress('exact test-only address', clipboard), true);
assert.deepEqual(copied, ['exact test-only address']);
await assert.rejects(copyAddress('test-only', {writeText: async () => {throw new Error('Denied');}}));
await assert.rejects(copyAddress('test-only', null));
function documentFixture() {
  const elements = new Map();
  for(const id of ['paypal','paypal-hint','bitcoin','monero','dogecoin','feedback']) {
    const code = {textContent:''};
    const hint = {textContent:'Address coming soon.'};
    elements.set(id, {disabled:true,attributes:{'aria-disabled':'true'},textContent:'',listeners:{},
      querySelector:()=>code,parentElement:{querySelector:()=>hint},
      removeAttribute(name){delete this.attributes[name];},
      addEventListener(name, callback){this.listeners[name]=callback;}});
  }
  return {getElementById:id=>elements.get(id)};
}
const blank = documentFixture();
renderSupport({...details,paypal_url:'',bitcoin:'',monero:'',dogecoin:''},blank);
assert.equal(blank.getElementById('paypal').href,undefined);
for(const coin of ['bitcoin','monero','dogecoin']) {
  assert.equal(blank.getElementById(coin).disabled,true);
  assert.equal(blank.getElementById(coin).querySelector('code').textContent,'');
  assert.equal(blank.getElementById(coin).listeners.click,undefined);
}
const populated = documentFixture();
renderSupport({...details,paypal_url:'https://www.paypal.com/',bitcoin:'test-only <address>',monero:'test-only XMR',dogecoin:'test-only DOGE'},populated);
assert.equal(populated.getElementById('paypal').href,'https://www.paypal.com/');
assert.equal(populated.getElementById('bitcoin').querySelector('code').textContent,'test-only <address>');
const previous = Object.getOwnPropertyDescriptor(globalThis,'navigator');
Object.defineProperty(globalThis,'navigator',{value:{clipboard},configurable:true});
try {
  await populated.getElementById('bitcoin').listeners.click();
  assert.equal(copied.at(-1),'test-only <address>');
  assert.equal(populated.getElementById('feedback').textContent,'Bitcoin address copied.');
  globalThis.navigator.clipboard={writeText:async()=>{throw new Error('Denied');}};
  await populated.getElementById('monero').listeners.click();
  assert.match(populated.getElementById('feedback').textContent,/Could not copy/);
} finally {
  if(previous)Object.defineProperty(globalThis,'navigator',previous);else delete globalThis.navigator;
}
console.log('Support page: empty details stay inactive; exact copy, configured links, and clipboard rejection feedback passed.');
