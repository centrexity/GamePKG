"""Bounded replies and thread-safe framing in the production ENet adaptation."""
from pathlib import Path
import os,socket,struct,subprocess
PROJECT=Path(__file__).resolve().parents[1]
binary=PROJECT/'target/cube2-network-bridge-test'
subprocess.run(['g++','-O2','-Wall','-Wextra','-Werror','-pthread','-I'+str(PROJECT/'engines/cube2/upstream/enet/include'),str(PROJECT/'tests/cube2_network_bridge.cpp'),'-o',str(binary)],check=True)
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
  if mode=='oversized':assert parent.recv(1)==b''
  else:
   for index in range(count):
    header=read(16);assert header[:4]==b'GPN1';op,n,reserved=struct.unpack_from('<3I',header,4);assert reserved==0 and n<=8192
    payload=read(n)
    if mode in ['create','threads']:assert op==1 and payload==struct.pack('<I',2)
    else:assert op==4 and payload==struct.pack('<2I',32,4096 if mode=='large-receive' else 32)
    parent.sendall(reply(data=struct.pack('<I',index+32)) if response is None else response)
    if response is not None:parent.shutdown(socket.SHUT_WR)
  out,err=app.communicate(timeout=4);assert app.returncode==0,(mode,app.returncode,err)
  if expected is not None:assert out.strip()==str(expected).encode(),out
 finally:
  parent.close()
  if app.poll() is None:app.kill();app.wait()
run('create',expected=32)
for data in [reply(data=struct.pack('<I',0)),reply(data=struct.pack('<I',1024)),reply(magic=b'BAD!'),reply(reserved=1),reply(length=0xffffffff),reply(status=1),reply(status=2),b'GPR2',reply(length=4,data=b'\0')]:
 run('create',data,expected=-1)
address=bytes([127,0,0,1])+struct.pack('<I',1234)
run('receive',reply(data=address+b'hello'),expected=5)
run('large-receive',reply(data=address+b'z'*4096),expected=4096)
for data in [reply(data=address+b'x'*33),reply(length=4105),reply(status=1),reply(data=b'bad')]:run('receive',data,expected=-1)
run('receive',reply(status=2),expected=0)
run('oversized')
run('threads',count=64)
print('Cube 2 ENet shim passed: raw sockets denied, bounded/truncated/malformed replies, oversized sends and concurrent RPC framing.')
