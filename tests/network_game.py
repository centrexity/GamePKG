"""Verify controlled networking across core and component Wasm guest bindings."""
from pathlib import Path
import os, shutil, socket, subprocess, tempfile, threading
PROJECT=Path(__file__).resolve().parents[1]
CLI=PROJECT/'target/release/gamepkg'
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
     stream.settimeout(15);assert stream.recv(4)==b'ping';stream.sendall(b'pong')
   except BaseException as error:errors.append(error)
  worker=threading.Thread(target=echo);worker.start()
  source=root/profile;shutil.copytree(PROJECT/'examples/minimal',source)
  manifest=source/'manifest.toml';text=manifest.read_text().replace('com.gamepkg.minimal','com.example.'+profile)
  text=text.replace('network = "none"',f'network = "multiplayer"\nnetwork_hosts = ["{endpoint}"]\nnetwork_allow_local = true')
  manifest.write_text(f'runtime_profile = "{profile}"\n'+text)
  (source/'source/src/lib.rs').write_text(f'''pub fn game_tick(_:u32,state:u64)->u64{{
   let handle=state as u32;let mut bytes=[0u8;16];
   if state==0{{match gamepkg::tcp_connect("{endpoint}"){{Ok(h)=>u64::from(h),_=>0}}}}
   else if state>>32==0{{match gamepkg::network_poll(handle,&mut[]){{Ok(_)=>{{match gamepkg::network_send(handle,b"ping"){{Ok(4)=>state|(1u64<<32),_=>state}}}},_=>state}}}}
   else if state>>32==1{{match gamepkg::network_receive(handle,&mut bytes){{Ok(4)=>{{if bytes[..4]==*b"pong"{{(42u64<<32)|u64::from(handle)}}else{{state}}}},_=>state}}}}
   else{{state}}
  }}''')
  package=root/(profile+'.gamepkg');run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator')
  run('build','com.example.'+profile,'--target','wasm')
  output=run('run','com.example.'+profile,'--frames','1000');assert 'score: 42' in output,output
  worker.join(17);server.close();assert not worker.is_alive();assert not errors,errors
 print('Core and component networking passed: asynchronous TCP echo through bounded host bindings.')
