"""Bounded replies and thread-safe framing in the production ENet adaptation."""
from pathlib import Path
import importlib.util,os,shutil,socket,struct,subprocess,tempfile
PROJECT=Path(__file__).resolve().parents[1]
binary=PROJECT/'target/cube2-network-bridge-test'
spec=importlib.util.spec_from_file_location('cube2_build',PROJECT/'tools/build_cube2_engine.py')
build=importlib.util.module_from_spec(spec);spec.loader.exec_module(build);build.verify()
with tempfile.TemporaryDirectory(prefix='gamepkg-network-headers-') as temporary:
 stage=Path(temporary);shutil.copytree(build.SOURCE/'enet/include',stage/'enet/include');build.adapt_socket_sets(stage)
 subprocess.run(['g++','-O2','-Wall','-Wextra','-Werror','-pthread','-I'+str(stage/'enet/include'),str(PROJECT/'tests/cube2_network_bridge.cpp'),'-o',str(binary)],check=True)
 abi=stage/'socket-set-abi-test'
 subprocess.run(['cc','-std=c11','-O2','-Wall','-Wextra','-Werror',str(PROJECT/'tests/fixtures/cube2-socket-set-abi.c'),'-o',str(abi)],check=True)
 subprocess.run([str(abi)],check=True,timeout=10)
 sets=stage/'socket-set-test'
 subprocess.run(['g++','-O2','-Wall','-Wextra','-Werror','-fsanitize=address,undefined','-I'+str(stage/'enet/include'),str(PROJECT/'tests/fixtures/cube2-socket-set.cpp'),'-o',str(sets)],check=True)
 subprocess.run([str(sets)],check=True,timeout=10,env=dict(os.environ,ASAN_OPTIONS='detect_leaks=1:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1'))

def reply(status=0,data=b'',magic=b'GPR2',reserved=0,length=None):
 return magic+struct.pack('<3I',status,len(data) if length is None else length,reserved)+data
def run(mode,response=None,count=1,expected=None):
 parent,child=socket.socketpair();parent.settimeout(3)
 def setup():os.dup2(child.fileno(),3)
 # Python closes non-listed fds after preexec_fn. Preserve destination 3 too.
 app=subprocess.Popen([str(binary),mode],preexec_fn=setup,pass_fds=(3,child.fileno()),stdout=subprocess.PIPE,stderr=subprocess.PIPE)
 child.close()
 def read(n):
  b=b''
  while len(b)<n:
   block=parent.recv(n-len(b));assert block,'Incomplete shim request';b+=block
  return b
 try:
  if mode in ['oversized','invalid-local','select-overflow','select-union-overflow']:assert parent.recv(1)==b''
  else:
   for index in range(count):
    header=read(16);assert header[:4]==b'GPN1';op,n,reserved=struct.unpack_from('<3I',header,4);assert reserved==0 and n<=8192
    payload=read(n)
    if mode in ['create','threads']:assert op==1 and payload==struct.pack('<I',2)
    elif mode in ['create-stream','normal-master']:assert op==1 and payload==struct.pack('<I',1)
    elif mode=='snapshot':assert op==16 and payload==b''
    elif mode=='listen':assert op==9 and payload==struct.pack('<2I',32,0xffffffff)
    elif mode.startswith('shutdown-'):assert op==13 and payload==struct.pack('<2I',32,int(mode[-1]))
    elif mode in ['accept','accept-null']:assert op==14 and payload==struct.pack('<I',32)
    elif mode=='select':assert op==8 and payload==struct.pack('<2I',32,3)
    elif mode in ['select-high','select-alias']:assert op==8 and payload==struct.pack('<2I',1000000,3)
    elif mode=='select-sparse':assert op==8 and payload==struct.pack('<2I',*[ (32,2),(1000000,3),(2147483647,1)][index])
    elif mode=='select-atomic':assert op==8 and payload==struct.pack('<2I',*[(32,2),(1000000,2)][index])
    elif mode=='set-option':assert op==11 and payload==struct.pack('<3I',32,10,37)
    elif mode=='get-option':assert op==12 and payload==struct.pack('<2I',32,10)
    else:assert op==4 and payload==struct.pack('<2I',32,4096 if mode=='large-receive' else 32)
    parent.sendall(reply(data=struct.pack('<I',index+32)) if response is None else response[index] if isinstance(response,list) else response)
    if response is not None and index+1==count:parent.shutdown(socket.SHUT_WR)
  out,err=app.communicate(timeout=4);assert app.returncode==0,(mode,app.returncode,err)
  if expected is not None:assert out.strip()==str(expected).encode(),out
 finally:
  parent.close()
  if app.poll() is None:app.kill();app.wait()
run('create',expected=32)
for opaque in [1024,1000000,2147483647]:run('create',reply(data=struct.pack('<I',opaque)),expected=opaque)
for mode in ['create-stream','normal-master','snapshot']:run(mode,expected=32)
for mode in ['listen','shutdown-0','shutdown-1','shutdown-2']:
 run(mode,reply(),expected=0)
 for response in [reply(status=1),reply(data=b'x'),reply(magic=b'BAD!'),b'GPR2']:
  run(mode,response,expected=-1)
run('invalid-local')
run('select-overflow')
run('select-union-overflow')
run('select-alias',reply(data=struct.pack('<I',2)),expected=1)
run('select-high',reply(data=struct.pack('<I',2)),expected=1)
run('select-sparse',[reply(data=struct.pack('<I',value)) for value in [0,2,1]],count=3,expected=2)
run('select-atomic',[reply(data=struct.pack('<I',0)),reply(status=1)],count=2,expected=-1)
run('select',reply(data=struct.pack('<I',3)),expected=1)
run('select',reply(status=1),expected=-1)
for flags in [4,2147483648,4294967295]:run('select',reply(data=struct.pack('<I',flags)),expected=-1)
peer=bytes([127,0,0,1])+struct.pack('<I',1234)
for mode in ['accept','accept-null']:
 run(mode,reply(data=struct.pack('<I',33)+peer),expected=33)
 run(mode,reply(data=struct.pack('<I',2147483647)+peer),expected=2147483647)
 for response in [reply(status=2),reply(status=1),reply(data=b'bad'),reply(data=struct.pack('<I',2147483648)+peer),reply(data=struct.pack('<I',33)+peer[:4]+struct.pack('<I',65536)),reply(length=13,data=b'x'*13)]:
  run(mode,response,expected=-1)
run('set-option',reply(),expected=0)
run('set-option',reply(status=1),expected=-1)
run('get-option',reply(data=struct.pack('<I',37)),expected=37)
for data in [reply(status=1),reply(data=b'bad'),reply(data=struct.pack('<I',0xffffffff)),reply(length=5,data=b'abcde')]:
 run('get-option',data,expected=-1)
for data in [reply(data=struct.pack('<I',0)),reply(data=struct.pack('<I',2147483648)),reply(magic=b'BAD!'),reply(reserved=1),reply(length=0xffffffff),reply(status=1),reply(status=2),b'GPR2',reply(length=4,data=b'\0')]:
 run('create',data,expected=-1)
address=bytes([127,0,0,1])+struct.pack('<I',1234)
run('receive',reply(data=address+b'hello'),expected=5)
run('large-receive',reply(data=address+b'z'*4096),expected=4096)
for data in [reply(data=address+b'x'*33),reply(length=4105),reply(status=1),reply(data=b'bad')]:run('receive',data,expected=-1)
run('receive',reply(status=2),expected=0)
run('oversized')
run('threads',count=64)
print('Cube 2 ENet shim passed: raw sockets denied, bounded/truncated/malformed replies, oversized sends, TCP/snapshot/listen/accept/shutdown/select framing bounded opaque socket sets and concurrent RPC calls.')
