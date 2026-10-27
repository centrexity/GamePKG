"""Verify controlled networking across core and component Wasm guest bindings."""
from pathlib import Path
import json, os, shutil, socket, subprocess, tempfile, threading
PROJECT=Path(__file__).resolve().parents[1]
CLI=Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',PROJECT/'target/release'))/'gamepkg'
compiled=subprocess.run(['cargo','test','--locked','--release','--test','network_game_runtime','--no-run','--message-format=json'],cwd=PROJECT,capture_output=True,text=True,timeout=300)
assert compiled.returncode==0,compiled.stderr
executables=[value['executable'] for line in compiled.stdout.splitlines() if line.startswith('{') and (value:=json.loads(line)).get('reason')=='compiler-artifact' and value.get('target',{}).get('name')=='network_game_runtime' and value.get('executable')]
assert len(executables)==1,executables
with tempfile.TemporaryDirectory(prefix='gamepkg-network-') as directory:
 root=Path(directory);key=root/'key';key.write_text('73'*32)
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'))
 def run(*args):
  result=subprocess.run([str(CLI),*map(str,args)],env=env,capture_output=True,text=True,timeout=120)
  assert result.returncode==0,result.stdout+result.stderr
  return result.stdout
 for profile in ['core-v1','component-v1']:
  server=socket.socket();server.bind(('127.0.0.1',0));server.listen(1);server.settimeout(15)
  endpoint='127.0.0.1:'+str(server.getsockname()[1]);errors=[]
  def echo():
   try:
    stream,_=server.accept()
    with stream:
     stream.settimeout(15)
     def receive(count):
      data=b''
      while len(data)<count:
       part=stream.recv(count-len(data));assert part,'unexpected peer close';data+=part
      return data
     assert receive(4)==b'ping'
     # Handshakes force separate guest reads without relying on sleeps or
     # TCP preserving send boundaries. The guest must retain partial bytes.
     stream.sendall(b'p');assert receive(1)==b'a'
     stream.sendall(b'on');assert receive(1)==b'b'
     stream.sendall(b'g')
   except BaseException as error:errors.append(error)
  source=root/profile;shutil.copytree(PROJECT/'examples/minimal',source)
  manifest=source/'manifest.toml';text=manifest.read_text().replace('com.gamepkg.minimal','com.example.'+profile)
  text=text.replace('network = "none"','network = true')
  manifest.write_text(f'runtime_profile = "{profile}"\n'+text)
  (source/'source/src/lib.rs').write_text(f'''struct Link {{ handle:u32, phase:u32, sent:u32, received:u32, acknowledged:u32, bytes:[u8;4] }}
static LINK:std::sync::Mutex<Link>=std::sync::Mutex::new(Link{{handle:0,phase:0,sent:0,received:0,acknowledged:0,bytes:[0;4]}});
pub fn game_tick(_:u32,_:u64)->u64{{
 let Ok(mut link)=LINK.lock() else {{return 0;}};
 if link.phase==0 {{if let Ok(handle)=gamepkg::tcp_connect("{endpoint}"){{link.handle=handle;link.phase=1;}}}}
 else if link.phase==1 {{
  let handle=link.handle;
  if gamepkg::network_poll(handle,&mut[]).is_ok() {{
   if let Ok(count)=gamepkg::network_send(handle,&b"ping"[link.sent as usize..]) {{
    if count>4-link.sent {{return 999u64<<32;}}
    link.sent+=count;if link.sent==4 {{link.phase=2;}}
   }}
  }}
 }} else if link.phase==2 {{
  let handle=link.handle;
  if link.received==1 && link.acknowledged&1==0 {{
   if let Ok(1)=gamepkg::network_send(handle,b"a"){{link.acknowledged|=1;}}
  }} else if link.received==3 && link.acknowledged&2==0 {{
   if let Ok(1)=gamepkg::network_send(handle,b"b"){{link.acknowledged|=2;}}
  }} else {{
   let offset=link.received as usize;
   if let Ok(count)=gamepkg::network_receive(handle,&mut link.bytes[offset..]) {{
    if count>4-link.received {{return 999u64<<32;}}
    link.received+=count;
    if link.received==4 {{if link.bytes==*b"pong"{{link.phase=3;}}else{{return 999u64<<32;}}}}
   }}
  }}
 }}
 let score=if link.phase==3 {{42}} else if link.received>0 {{100+link.received}} else {{0}};
 (u64::from(score)<<32)|u64::from(link.handle)
}}''')
  package=root/(profile+'.gamepkg');run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator')
  run('build','com.example.'+profile,'--target','wasm')
  worker=threading.Thread(target=echo);worker.start()
  try:
   subprocess.run([executables[0],'fragmented_tcp_round_trip_completes_across_real_async_ticks','--ignored','--nocapture'],cwd=PROJECT,env=dict(env,GAMEPKG_NETWORK_TEST_ROOT=str(root/'library'),GAMEPKG_NETWORK_TEST_ID='com.example.'+profile),check=True,timeout=30)
  finally:
   worker.join(17);server.close()
  assert not worker.is_alive();assert not errors,errors
  # Disabled permission must fail before DNS, connect or bind in both ABIs.
  disabled='com.example.'+profile+'-disabled'
  manifest.write_text(manifest.read_text().replace('com.example.'+profile,disabled).replace('network = true','network = false'))
  (source/'source/src/lib.rs').write_text('''pub fn game_tick(_:u32,_:u64)->u64 {
   let mut denied=0u32;
   if let Err(gamepkg::NetworkError::PermissionDenied)=gamepkg::https_get("https://unused.invalid/"){denied+=1;}
   if let Err(gamepkg::NetworkError::PermissionDenied)=gamepkg::network_request("https://unused.invalid/",&mut [0u8;1]){denied+=1;}
   if let Err(gamepkg::NetworkError::PermissionDenied)=gamepkg::tcp_connect("127.0.0.1:1"){denied+=1;}
   if let Err(gamepkg::NetworkError::PermissionDenied)=gamepkg::udp_connect("127.0.0.1:1"){denied+=1;}
   if let Err(gamepkg::NetworkError::PermissionDenied)=gamepkg::tcp_listen("127.0.0.1:1"){denied+=1;}
   if denied==5 {99u64<<32} else {0}
  }''')
  package=root/(profile+'-disabled.gamepkg');run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator')
  run('build',disabled,'--target','wasm')
  output=run('run',disabled,'--frames','1');assert 'score: 99' in output,output
 print('Core and component networking passed: unlisted local TCP echo and disabled HTTPS/cached-HTTPS/TCP/UDP/listener denial through bounded bindings.')
