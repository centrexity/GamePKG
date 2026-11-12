"""Compare the adapter input filter to the unchanged original for all 16-bit inputs."""
from pathlib import Path
import os
import subprocess
import tempfile
root = Path(__file__).resolve().parents[1]
upstream = (root / 'engines/zelda3/upstream/src/zelda_rtl.c').read_text()
original = upstream.split('bool ZeldaRunFrame(int inputs) {', 1)[1].split('  frame_ctr_dbg++;', 1)[0]
adapter = (root / 'engines/zelda3/adapters/engine.c').read_text()
adapted = adapter.split("  // Match ZeldaRunFrame's opposing-direction filter before bypassing its recorder.", 1)[1].split('  // Preserve the upstream scheduling', 1)[0]
source = '#include <assert.h>\n#include <stdint.h>\n'
source += 'static uint32_t original(uint32_t inputs) {' + original + '\nreturn inputs;}\n'
source += 'static uint32_t adapted(uint32_t buttons) {' + adapted + '\nreturn buttons;}\n'
source += '''int main(void) {
for(uint32_t i=0;i<65536;i++) {
 uint32_t expected=original(i), actual=adapted(i);
 assert(actual==expected);
 assert((actual & ~0xf0u)==(i & ~0xf0u));
 assert((actual & 0x30u)!=0x30u && (actual & 0xc0u)!=0xc0u);
}
}\n'''
with tempfile.TemporaryDirectory(prefix='gamepkg-zelda-input-') as temporary:
    directory=Path(temporary)
    path=directory/'input.c';path.write_text(source)
    binary=directory/'input'
    subprocess.run(['cc','-std=c11','-Wall','-Wextra','-Werror','-fsanitize=address,undefined',str(path),'-o',str(binary)],check=True)
    subprocess.run([str(binary)],check=True,env=dict(os.environ,ASAN_OPTIONS='detect_leaks=1',UBSAN_OPTIONS='halt_on_error=1'))
print('All 65,536 adapter input combinations match the unchanged original filter.')
