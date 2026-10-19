"""Maintainer-only isolated Ethernet LAN for original Cube 2 client/server tests."""
from pathlib import Path
import argparse, errno, json, os, socket, struct, subprocess, sys, time
p=argparse.ArgumentParser()
for name in ['engine','binary','state','proof','test']:p.add_argument('--'+name,required=True)
p.add_argument('--inside',action='store_true');p.add_argument('--worker',choices=['server','client']);p.add_argument('--capture',action='store_true')
a=p.parse_args();state=Path(a.state);proof=Path(a.proof)
def wait_for(path,seconds=15):
 until=time.monotonic()+seconds
 while not path.exists():
  assert time.monotonic()<until,str(path)
  time.sleep(.01)
def ip(*args):subprocess.run(['ip',*map(str,args)],check=True)
def common():return [sys.executable,str(Path(__file__).resolve()),'--engine',a.engine,'--binary',a.binary,'--state',a.state,'--proof',a.proof,'--test',a.test]
if a.inside or a.worker or a.capture:
 assert a.inside,'worker/capture requires a private namespace'
 assert os.readlink('/proc/self/ns/net')!=os.environ['GAMEPKG_LAN_PARENT_NETNS'],'refusing to use the real network'
if a.worker:
 (state/(a.worker+'.pid')).write_text(str(os.getpid()));wait_for(state/'network-ready')
 env=dict(os.environ,GAMEPKG_LAN_TEST_ROLE=a.worker,GAMEPKG_LAN_TEST_STATE=a.state,GAMEPKG_LAN_TEST_PROOF=a.proof,GAMEPKG_LAN_TEST_ENGINE=a.engine)
 os.execve(a.binary,[a.binary,'--exact',a.test,'--ignored','--nocapture'],env)
if not a.inside:
 env=dict(os.environ,GAMEPKG_LAN_PARENT_NETNS=os.readlink('/proc/self/ns/net'))
 subprocess.run(['unshare','--user','--map-root-user','--net',*common(),'--inside'],env=env,check=True);raise SystemExit
assert os.readlink('/proc/self/ns/net')!=os.environ['GAMEPKG_LAN_PARENT_NETNS'],'refusing to configure the real network'
if a.capture:
 frames=[]
 with socket.socket(socket.AF_PACKET,socket.SOCK_RAW,socket.htons(3)) as capture:
  capture.bind(('gp-client0',3));capture.settimeout(.1);(state/'capture-ready').write_text('ready')
  while not (state/'capture-stop').exists():
   try:data=capture.recv(65536)
   except TimeoutError:continue
   except OSError as error:
    if error.errno==errno.ENETDOWN and (state/'client-finished').exists():break
    (proof/'packets.json').write_text(json.dumps(frames,indent=2)+'\n')
    raise
   if len(data)<42 or data[12:14]!=b'\x08\x00' or data[14]>>4!=4 or data[23]!=17:continue
   offset=14+(data[14]&15)*4
   if len(data)<offset+8:continue
   src,dst,length,_=struct.unpack_from('!4H',data,offset)
   if dst!=28784 and src!=32124:continue
   assert len(frames)<256,'unexpected packet flood'
   frames.append(dict(phase=(state/'phase').read_text() if (state/'phase').exists() else 'setup',source=socket.inet_ntoa(data[26:30]),destination=socket.inet_ntoa(data[30:34]),source_port=src,destination_port=dst,payload=data[offset+8:offset+length].hex()))
 (proof/'packets.json').write_text(json.dumps(frames,indent=2)+'\n');raise SystemExit
processes=[];logs=[];capture=None;namespaces={}
try:
 ip('link','set','lo','up');ip('link','add','gp-bridge','type','bridge');ip('link','set','gp-bridge','up')
 for role in ['server','client']:
  outer='gp-'+role+'0';inner='gp-'+role+'1';ip('link','add',outer,'type','veth','peer','name',inner);ip('link','set',outer,'master','gp-bridge');ip('link','set',outer,'up')
  log=(proof/(role+'.log')).open('wb');logs.append(log)
  child=subprocess.Popen(['unshare','--net',*common(),'--inside','--worker',role],stdout=log,stderr=subprocess.STDOUT);processes.append(child)
  wait_for(state/(role+'.pid'));pid=int((state/(role+'.pid')).read_text())
  assert pid==child.pid,'worker PID changed unexpectedly'
  namespace=os.readlink('/proc/'+str(pid)+'/ns/net')
  assert namespace not in [os.environ['GAMEPKG_LAN_PARENT_NETNS'],os.readlink('/proc/self/ns/net'),*namespaces.values()],'hosts must have separate private network namespaces'
  namespaces[role]=namespace
  ip('link','set',inner,'netns',pid)
  prefix=['nsenter','--net=/proc/'+str(pid)+'/ns/net','--','ip']
  for arguments in [['link','set','lo','up'],['addr','add','10.77.0.'+('1' if role=='server' else '2')+'/24','brd','10.77.0.255','dev',inner],['link','set',inner,'up'],['route','add','default','dev',inner]]:subprocess.run(prefix+arguments,check=True)
 (proof/'topology.json').write_text(json.dumps(dict(parent=os.environ['GAMEPKG_LAN_PARENT_NETNS'],bridge=os.readlink('/proc/self/ns/net'),hosts=namespaces),indent=2)+'\n')
 capture=subprocess.Popen([*common(),'--inside','--capture'],stdout=subprocess.PIPE,stderr=subprocess.PIPE);processes.append(capture);wait_for(state/'capture-ready')
 (state/'network-ready').write_text('ready')
 deadline=time.monotonic()+120
 while processes[1].poll() is None:
  assert time.monotonic()<deadline,'client timed out'
  assert processes[0].poll() is None,'server stopped early'
  time.sleep(.05)
 assert processes[1].returncode==0,'client failed'
 (state/'stop').write_text('stop');assert processes[0].wait(timeout=8)==0,'server failed'
 (state/'capture-stop').write_text('stop');out,err=capture.communicate(timeout=3);assert capture.returncode==0,(out,err)
 frames=json.loads((proof/'packets.json').read_text())
 for phase in ['offline','search-off']:assert not any(f['phase']==phase for f in frames),phase
 broadcasts=[f for f in frames if f['phase']=='search-on' and f['destination']=='255.255.255.255' and f['destination_port']==28784]
 replies=[f for f in frames if f['source']=='10.77.0.1' and f['source_port']==32124]
 assert broadcasts and replies,frames
 assert all(b['source']=='10.77.0.2' and b['payload'] for b in broadcasts),broadcasts
 assert any(r['destination']=='10.77.0.2' and r['destination_port']==b['source_port'] and len(r['payload'])>len(b['payload']) and r['payload'].startswith(b['payload']) for r in replies for b in broadcasts),'original ping token not echoed'
 print('Private original-engine LAN: disabled cases silent; broadcast, token-echo reply, browser discovery, row-click connection and remote identity passed.')
finally:
 (state/'stop').write_text('stop');(state/'capture-stop').write_text('stop')
 if capture is not None and capture.poll() is None:
  try:capture.wait(timeout=2)
  except subprocess.TimeoutExpired:pass
 for child in processes:
  if child.poll() is None:
   child.terminate()
   try:child.wait(timeout=5)
   except subprocess.TimeoutExpired:child.kill();child.wait()
 for log in logs:log.close()
 for role in ['server','client']:
  path=proof/(role+'.log')
  if path.exists():print(path.read_text()[-8000:])
