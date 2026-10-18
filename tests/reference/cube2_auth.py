"""Generate public test credentials with unchanged pinned Cube 2 cryptography."""
from pathlib import Path
import importlib.util, json, shutil, subprocess, tempfile
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('cube2_build',ROOT/'tools/build_cube2_engine.py')
build=importlib.util.module_from_spec(spec);spec.loader.exec_module(build)
lock=build.verify()
with tempfile.TemporaryDirectory(prefix='gamepkg-auth-reference-') as temporary:
 stage=Path(temporary)
 for source in (build.SOURCE/'shared').iterdir():
  if source.is_file():shutil.copy2(source,stage/source.name)
 tools=stage/'tools.h'
 tools.write_text(build.replace_once(tools.read_text(),'inline void *operator new(size_t, void *p) { return p; }\ninline void *operator new[](size_t, void *p) { return p; }\ninline void operator delete(void *, void *) {}\ninline void operator delete[](void *, void *) {}','#include <new>'))
 binary=stage/'reference'
 subprocess.run(['g++','-O2','-DSTANDALONE','-I'+str(stage),'-I'+str(build.SOURCE/'enet/include'),str(Path(__file__).with_suffix('.cpp')),str(stage/'crypto.cpp'),'-o',str(binary)],check=True)
 private,challenge,response=subprocess.check_output([str(binary)],text=True).splitlines()
 output={'revision':lock['revision'],'notice':'Public deterministic test credential; no real account or user key.','private_key':private,'challenge':challenge,'response':response}
 (ROOT/'tests/fixtures/cube2-auth.json').write_text(json.dumps(output,indent=2)+'\n')
 print('Generated pinned-original public authentication fixture.')
