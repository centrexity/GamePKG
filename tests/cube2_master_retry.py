"""Original master timing/parser fragments; no clocks or rules changed in engine."""
from pathlib import Path
import importlib.util, os, subprocess, tempfile

project = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('cube2_build', project/'tools/build_cube2_engine.py')
build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build)
build.verify()
source = (build.SOURCE/'engine/server.cpp').read_text()

def fragment(start, end):
    assert source.count(start) == 1, start
    begin = source.index(start)
    finish = source.index(end, begin)
    return source[begin:finish]

fragments = [
    fragment('ENetSocket mastersock =', 'VARN(updatemaster,'),
    fragment('void disconnectmaster()', '\nSVARF(mastername,'),
    fragment('bool requestmaster(const char *req)', '\nvoid processmasterinput()'),
    fragment('void processmasterinput()', '\nvoid flushmasteroutput()'),
    fragment('void flushmasteroutput()', '\nvoid flushmasterinput()'),
    fragment('void updatemasterserver()', '\nuint totalsecs ='),
]
heartbeat = fragment('    if(!lastupdatemaster || totalmillis-lastupdatemaster>60*60*1000)', '\n    if(totalmillis-laststatus>')
with tempfile.TemporaryDirectory(prefix='gamepkg-cube-master-timing-') as temporary:
    root = Path(temporary)
    (root/'original-master.h').write_text('\n'.join(fragments)+'\nvoid originalheartbeat() {\n'+heartbeat+'\n}\n')
    binary = root/'master-timing'
    subprocess.run(['g++','-O2','-Wall','-Wextra','-Werror','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(root),str(project/'tests/fixtures/cube2-master-retry.cpp'),'-o',str(binary)],check=True)
    subprocess.run([str(binary)],check=True,timeout=10,env=dict(os.environ,ASAN_OPTIONS='detect_leaks=1:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1'))
print('Pinned original master logic: connect timeout, five-minute backoff edges, hourly registration, refusal, fragmented replies, partial/blocked writes and disconnect cleanup passed under sanitizers.')
