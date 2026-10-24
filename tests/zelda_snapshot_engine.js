// Maintainer-only actual engine capture/restore. Resources arrive in RAM on stdin.
const fs = require('node:fs');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const moduleObject = new WebAssembly.Module(fs.readFileSync(process.argv[2]));
assert.equal(WebAssembly.Module.imports(moduleObject).length, 0);
const resources = fs.readFileSync(0);
const size = 275497;
const crcTable = Uint32Array.from({ length: 256 }, (_, value) => {
  for (let bit = 0; bit < 8; bit++) value = value & 1 ? 0xedb88320 ^ value >>> 1 : value >>> 1;
  return value >>> 0;
});
function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) crc = crcTable[(crc ^ byte) & 255] ^ crc >>> 8;
  return (crc ^ 0xffffffff) >>> 0;
}
assert.equal(crc32(Buffer.from('123456789')), 0xcbf43926);
function create() {
  const e = new WebAssembly.Instance(moduleObject, {}).exports;
  assert.equal(e.snapshot_size(), 0);
  assert.equal(e.snapshot_data(), 0);
  assert.equal(e.snapshot_capture(), -1);
  assert.equal(e.snapshot_restore(size), -1);
  const ptr = e.allocate(resources.length);
  assert(ptr);
  new Uint8Array(e.memory.buffer, ptr, resources.length).set(resources);
  // Every bank is checked before ZeldaInitialize or any original upload runs.
  let bankOffset = 748 + resources.readUInt32LE(84);
  for (let bank = 0; bank < 3; bank++) {
    bankOffset = (bankOffset + 3) & ~3;
    const length = resources.readUInt32LE(88 + bank * 4);
    assert(length > 2);
    const memory = new Uint8Array(e.memory.buffer, ptr + bankOffset, length);
    for (const offset of [0, length - 2]) {
      const original = memory.slice(offset, offset + 2);
      memory[offset] = offset === 0 ? 0 : 1;
      memory[offset + 1] = 0;
      assert.equal(e.initialize(ptr, resources.length), -1);
      assert.equal(e.snapshot_size(), 0);
      memory.set(original, offset);
    }
    bankOffset += length;
  }
  assert.equal(e.initialize(ptr, resources.length), 0);
  assert.equal(e.configure_video(0, 0), 0);
  assert.equal(e.snapshot_size(), size);
  return e;
}
function capture(e) {
  assert.equal(e.snapshot_capture(), 0);
  const bytes = Buffer.from(new Uint8Array(e.memory.buffer, e.snapshot_data(), size));
  assert.equal(bytes.subarray(0, 8).toString('hex'), '8947505a53540d0a');
  assert.equal(bytes.readUInt32LE(8), 1);
  assert.equal(bytes.readUInt32LE(12), size - 32);
  assert.equal(bytes.subarray(16, 24).toString(), '45a149d8');
  assert.equal(bytes.readUInt32LE(24), crc32(bytes.subarray(32)));
  assert.equal(bytes.readUInt32LE(28), 0);
  assert(e.memory.buffer.byteLength <= 16 * 1024 * 1024);
  return bytes;
}
function put(e, bytes) {
  assert.equal(bytes.length, size);
  new Uint8Array(e.memory.buffer, e.snapshot_data(), size).set(bytes);
}
function restore(e, bytes) {
  put(e, bytes);
  assert.equal(e.snapshot_restore(size), 0);
  assert.equal(e.save_dirty(), 1, 'Restored SRAM was not marked for the host save path');
}
function run(e, frames, input = 0) {
  for (let i = 0; i < frames; i++) assert.equal(e.step(input), 0);
}
function press(e, input) { run(e, 1, input); run(e, 1); }
function signature(e) {
  const hash = crypto.createHash('sha256');
  hash.update(new Uint8Array(e.memory.buffer, e.pixels(), 256 * 224 * 4));
  hash.update(new Uint8Array(e.memory.buffer, e.audio(), 1600 * 2));
  return [e.status() >>> 0, e.position() >>> 0, e.controller() >>> 0, hash.digest('hex')];
}
function invalidSnapshots(e) {
  const baseline = capture(e);
  e.save_dirty();
  function reject(bytes, length = size) {
    put(e, bytes);
    assert.equal(e.snapshot_restore(length), -1);
    assert.equal(e.save_dirty(), 0);
    assert.deepEqual(capture(e), baseline);
  }
  for (const length of [0, 1, 31, 32, size - 1, size + 1, 0xffffffff]) reject(baseline, length);
  for (const offset of [0, 7, 8, 12, 16, 23, 24, 28, 31, size - 1, 32 + 144389]) {
    const bytes = Buffer.from(baseline); bytes[offset] ^= 0x80; reject(bytes);
  }
  for (const offset of [65603 + 128 + 4, 68642 + 11, 68834 + 67138]) {
    const bytes = Buffer.from(baseline); bytes[32 + offset] = 2;
    bytes.writeUInt32LE(crc32(bytes.subarray(32)), 24);
    reject(bytes);
  }
}
function scenario(e, name, inputs, disturb) {
  const saved = capture(e);
  const status = e.status(), position = e.position();
  disturb(e); // Restore across a real module/menu transition, not just a short pause.
  restore(e, saved);
  assert.equal(e.status(), status);
  assert.equal(e.position(), position);
  const expected = [];
  for (const input of inputs) { run(e, 1, input); expected.push(signature(e)); }
  const fresh = create();
  restore(fresh, saved);
  assert.equal(fresh.status(), status);
  assert.equal(fresh.position(), position);
  for (let i = 0; i < inputs.length; i++) {
    run(fresh, 1, inputs[i]);
    assert.deepEqual(signature(fresh), expected[i], `${name}: restored frame/audio/state differs at ${i}`);
  }
  assert(expected.some((entry, i) => i && entry[3] !== expected[0][3]), 'Restored playback never advanced');
  invalidSnapshots(fresh);
  console.log(`${name}: ${inputs.length} exact post-restore frames/audio/controller/state observations and invalid-snapshot preservation passed.`);
}
const title = create(); run(title, 1000);
function enterGameplay(e) {
  press(e, 8); run(e, 60); press(e, 8); run(e, 60);
  press(e, 1); run(e, 10); press(e, 8); run(e, 60);
  press(e, 8); run(e, 600); run(e, 180, 256);
  for (let i = 0; i < 12; i++) { press(e, 1); run(e, 180); }
  assert.equal(e.status() & 255, 7, 'Opening dialogue did not reach gameplay');
}
scenario(title, 'Title after entering gameplay', Array(120).fill(0), enterGameplay);
const game = create(); run(game, 1000); enterGameplay(game);
scenario(game, 'Gameplay after inventory', [...Array(60).fill(128), ...Array(30).fill(16), ...Array(60).fill(64), ...Array(30).fill(0)], e => {
  press(e, 8); run(e, 45);
  assert.notEqual(e.status() & 255, 7, 'Inventory transition did not leave gameplay');
});
console.log('Actual import-free Wasm snapshot capture/restore passed; resources and snapshots stayed in RAM.');
