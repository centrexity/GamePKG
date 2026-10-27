"""Exercise the real trusted native SDK against a bounded deterministic host."""
from pathlib import Path
import os,select,shutil,struct,subprocess,tempfile,time

SOURCE='''pub fn game_tick(input:u32,_:u64)->u64 {
 let mut sum=0u64;
 for _ in 0..(input&65535) {sum=sum.wrapping_add(gamepkg::random_u64());}
 if input&2147483648!=0 {gamepkg::draw_clear(0x123456ff);}
 for _ in 0..((input>>16)&32767) {sum=sum.wrapping_add(gamepkg::random_u64());}
 sum
}'''
MASK=(1<<64)-1

def _check_native_randomness(binary,env):
 def read_exact(child,n,deadline):
  data=bytearray()
  while len(data)<n:
   left=deadline-time.monotonic()
   assert left>0 and select.select([child.stdout],[],[],left)[0], 'Native random fixture stalled'
   part=os.read(child.stdout.fileno(),n-len(data))
   if not part:raise EOFError('Native random worker closed')
   data.extend(part)
  return bytes(data)
 def receive(child,deadline):
  n=struct.unpack('<I',read_exact(child,4,deadline))[0]
  assert 1<=n<=65536,('Unexpected worker packet length',n)
  return read_exact(child,n,deadline)
 def send(child,data):
  child.stdin.write(struct.pack('<I',len(data))+data)
 def close_input(child):
  if child.stdin is not None:
   child.stdin.close();child.stdin=None
 def one(inputs,expected_error=None,bad_block=None):
  child=subprocess.Popen([str(binary)],env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,bufsize=0)
  emitted=consumed=blocks=draws=0;completed=[]
  try:
   assert receive(child,time.monotonic()+5)==bytes([255])
   ended=False
   for value in inputs:
    send(child,struct.pack('<IQ',value,0));deadline=time.monotonic()+5
    try:
     while True:
      packet=receive(child,deadline)
      if packet[0]==0:
       assert len(packet)==9
       count=(value&65535)+((value>>16)&32767)
       expected=sum(0xa5c3000000000000|i for i in range(consumed+1,consumed+count+1))&MASK
       actual=struct.unpack('<Q',packet[1:])[0];assert actual==expected,(actual,expected)
       consumed+=count;completed.append(value);break
      if packet==bytes([43]):
       blocks+=1
       data=b''.join(struct.pack('<Q',0xa5c3000000000000|i) for i in range(emitted+1,emitted+65));emitted+=64
       if bad_block=='oversized':child.stdin.write(struct.pack('<I',8*1024*1024+65537))
       elif bad_block=='truncated':child.stdin.write(struct.pack('<I',512)+data[:-1]);close_input(child)
       elif bad_block is not None:send(child,data[:bad_block] if bad_block<=512 else data+bytes(bad_block-512))
       else:send(child,data)
      elif packet==bytes([8,0xff,0x56,0x34,0x12]):draws+=1;send(child,bytes([0]))
      else:raise AssertionError(('Unexpected worker request',packet[:32]))
    except EOFError:
     ended=True;break
   close_input(child);stdout,stderr=child.communicate(timeout=5);assert stdout==b'',stdout
   if expected_error:
    assert ended and child.returncode!=0,(child.returncode,completed)
    assert expected_error in stderr.decode(errors='replace'),stderr.decode(errors='replace')
   else:
    assert not ended and child.returncode==0 and completed==inputs,stderr.decode(errors='replace')
    assert blocks==(consumed+63)//64,(blocks,consumed)
    assert draws==sum(bool(value&2147483648) for value in inputs)
  finally:
   if child.poll() is None:child.kill()
   child.wait(timeout=5)
 # Carry unused bytes over ticks, refill exactly at word64 and keep both halves.
 one([0,1,63,64,65,8192,2147483648|8191,0,8192])
 # A cache refill must not spend 64 calls or omit charges on cached draws.
 one([2147483648|63|(8128<<16)])
 for value in [8193,2147483648|8192,2147483648|63|(8129<<16)]:
  one([value],expected_error='native host call budget exceeded')
 for size in [0,1,511,513]:one([1],expected_error='invalid random-block response',bad_block=size)
 one([1],expected_error='assertion failed',bad_block='oversized')
 one([1],expected_error='failed to fill whole buffer',bad_block='truncated')
 print('Native entropy cache passed: exact LE/high bits,64-word boundaries,tick reset,shared8192-call ceiling,malformed/truncated replies and bounded refill counts.',flush=True)

def check_native_randomness(binary,env):
 # Cached artifacts are deliberately non-executable. Like the normal owner,
 # execute a private read-only snapshot without modifying its receipt/mode.
 with tempfile.TemporaryDirectory(prefix='gamepkg-random-protocol-') as temporary:
  worker=Path(temporary)/'worker';shutil.copyfile(binary,worker);worker.chmod(0o500)
  _check_native_randomness(worker,env)
