// Maintainer-only, offline numeric-table generation from pinned LGPL sources.
import fs from 'node:fs';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const dir=path.join(root,'engines/takedawn/native-gen/runtime/status');
const lock=JSON.parse(fs.readFileSync(path.join(dir,'upstream-lock.json'),'utf8'));
assert.equal(lock.revision,'fd1153552d779e1ac14b0b1c48fbd1cde894eb0a');
for(const record of lock.files){assert.equal(crypto.createHash('sha256').update(fs.readFileSync(path.join(dir,record.path))).digest('hex'),record.sha256);}
const map=fs.readFileSync(path.join(dir,'upstream/error.h'),'utf8');
const constants=fs.readFileSync(path.join(dir,'upstream/winerror.h'),'utf8')+'\n'+fs.readFileSync(path.join(dir,'upstream/ntstatus.h'),'utf8');
const definitions=new Map([...constants.matchAll(/^#define\s+(\w+)\s+([^\n]+)$/gm)].map(m=>[m[1],m[2].trim()]));
function resolve(name,seen=new Set()){
 assert(!seen.has(name),'cyclic constant');seen.add(name);
 const value=definitions.get(name);assert(value,`missing ${name}`);
 let m=value.match(/^(0x[0-9a-fA-F]+|\d+)[uUlL]*$/);
 if(!m)m=value.match(/^_(?:HRESULT|NDIS_ERROR)_TYPEDEF_\((0x[0-9a-fA-F]+|\d+)\)$/);
 if(!m)m=value.match(/^\(\(NTSTATUS\)\s*(0x[0-9a-fA-F]+|\d+)\)$/);
 if(m){const n=BigInt(m[1]);assert(n>=0n&&n<=0xffffffffn);return Number(n);}
 assert(/^\w+$/.test(value),`unsupported constant ${name}: ${value}`);return resolve(value,seen);
}
const begin=map.indexOf('static const DWORD error_map[1586]');const end=map.indexOf('static const struct',begin);assert(begin>0&&end>begin);
const rows=[...map.slice(begin,end).matchAll(/^\s+(\w+),\s*\/\* ([0-9a-f]{8})(?: \((\w+)\))? \*\/$/gm)].map(m=>({name:m[1],status:parseInt(m[2],16),ntname:m[3],value:resolve(m[1])}));assert.equal(rows.length,1586);
const ranges=[...map.slice(end).matchAll(/\{ (0x[0-9a-f]+), (0x[0-9a-f]+), (\d+) \}/g)].map(m=>({start:parseInt(m[1],16),end:parseInt(m[2],16),offset:Number(m[3])}));assert.equal(ranges.length,43);
let offset=0;for(let i=0;i<ranges.length;++i){const r=ranges[i];assert(r.start<=r.end);assert(!i||r.start>ranges[i-1].end);assert.equal(r.offset,offset);for(let s=r.start;s<=r.end;++s)assert.equal(rows[offset++].status,s);}assert.equal(offset,1586);
const hex=n=>'0x'+n.toString(16).padStart(8,'0')+'u';
const notice=map.slice(0,map.indexOf('/* conversion table'));
let output=notice+'/* Numeric adaptation generated offline by tools/generate_takedawn_status.mjs.\n * Pinned Wine9.0; LGPL-2.1-or-later. Keep upstream notices/source/relink inputs. */\n';
output+='static const uint32_t gamepkg_status_errors[1586] = {\n'+rows.map(r=>`  ${hex(r.value)}, /* ${hex(r.status)} ${r.ntname??''} -> ${r.name} */`).join('\n')+'\n};\n';
output+='static const struct { uint32_t start, end, offset; } gamepkg_status_ranges[43] = {\n'+ranges.map(r=>`  {${hex(r.start)}, ${hex(r.end)}, ${r.offset}u},`).join('\n')+'\n};\n';
const errorSource=fs.readFileSync(path.join(dir,'upstream/error.c'),'utf8');const signature='ULONG WINAPI RtlNtStatusToDosErrorNoTeb( NTSTATUS status )';const from=errorSource.indexOf(signature),to=errorSource.indexOf('\n}\n',from);assert(from>0&&to>from);const reference=errorSource.slice(0,errorSource.indexOf('#include'))+errorSource.slice(from,to+3);
const inputs=notice+'/* Maintainer input enumeration: source-table entries plus boundary probes. */\nstatic const uint32_t gamepkg_status_inputs[1586] = {\n'+rows.map(r=>'  '+hex(r.status)+',').join('\n')+'\n};\nstatic const uint32_t gamepkg_status_boundaries[172] = {\n'+ranges.flatMap(r=>[r.start-1,r.start,r.end,r.end+1]).map(n=>'  '+hex(n)+',').join('\n')+'\n};\n';
const products=[['status_map.h',output],['reference/no_teb.inc',reference],['reference/inputs.h',inputs]];
const check=process.argv.slice(2);assert(check.length===0||(check.length===1&&check[0]==='--check'));
for(const [name,text]of products){const target=path.join(dir,name);if(check.length)assert.equal(fs.readFileSync(target,'utf8'),text,`${name} differs`);else fs.writeFileSync(target,text);}
console.log('Pinned NTSTATUS: 1586 mappings, 43 ranges; numeric/reference products '+(check.length?'verified':'generated'));
