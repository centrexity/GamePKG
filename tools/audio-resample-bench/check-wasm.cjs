// Maintainer-only; run with the separately compiled comparison module.
const fs = require("node:fs");
async function main() {
    if (process.argv.length !== 3) throw Error("usage: node check-wasm.cjs COMPARE.wasm");
    const module = await WebAssembly.compile(fs.readFileSync(process.argv[2]));
    if (WebAssembly.Module.imports(module).length) throw Error("unexpected host imports");
    const { exports: e } = await WebAssembly.instantiate(module, {});
    const checksum = e.compare_audio_resamplers();
    if (e.memory.buffer.byteLength > 16777216) throw Error("comparison memory limit");
    console.log(JSON.stringify({
        cases: 2592, frames: 5963184,
        fnv64: BigInt.asUintN(64, checksum).toString(16),
        memory_bytes: e.memory.buffer.byteLength,
    }));
}
main().catch(error => { console.error(error); process.exitCode = 1; });
