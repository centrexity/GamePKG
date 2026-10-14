"""Development-only differential check; the shipped host never uses Python.

The reference decoder holds ROM resources in RAM. Only sizes/checksums are
written to a temporary test metadata file, never asset data or ROM bytes.
"""
import argparse,hashlib,importlib.util,json,os,struct,subprocess,tempfile
from pathlib import Path
PROJECT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser(description=__doc__);p.add_argument('rom',type=Path);args=p.parse_args()
source=PROJECT/'engines/zelda3/adapters/rom_resources.py'
spec=importlib.util.spec_from_file_location('reference_decoder',source)
reference=importlib.util.module_from_spec(spec);spec.loader.exec_module(reference)
original=args.rom.read_bytes();rom_digest=hashlib.sha256(original).hexdigest()
data=reference.decode(original)
base=len(b'Zelda3_v0     \n\0')+64
count,keylen=struct.unpack_from('<II',data,base);assert count==165
sizes=struct.unpack_from('<165I',data,base+8)
names=data[base+8+count*4:base+8+count*4+keylen].split(b'\0')[:-1];assert len(names)==count
pos=base+8+count*4+keylen;hashes={}
for name,size in zip(names,sizes):
 pos=(pos+3)&~3;hashes[name.decode()]=[size,hashlib.sha256(data[pos:pos+size]).hexdigest()];pos+=size
hashes['__blob']=[len(data),hashlib.sha256(data).hexdigest()]
with tempfile.TemporaryDirectory(prefix='zelda3-native-reference-') as directory:
 metadata=Path(directory)/'hashes.json';metadata.write_text(json.dumps(hashes))
 env=dict(os.environ,GAMEPKG_ZELDA3_TEST_ROM=str(args.rom.resolve()),GAMEPKG_ZELDA3_REFERENCE=str(metadata),GAMEPKG_ZELDA3_PYTHON='intentionally-unavailable-decoder')
 subprocess.run(['cargo','test','--locked','--lib','native_resources_match_reference','--','--ignored','--nocapture'],cwd=PROJECT,env=env,check=True)
assert hashlib.sha256(args.rom.read_bytes()).hexdigest()==rom_digest
print('Native Rust decoder: all 165 resources and complete packed blob match; headered ROM matches; source ROM unchanged.')
