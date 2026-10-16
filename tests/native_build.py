"""Direct machine code, no implicit Wasm, native deadlines and controlled TCP/UDP."""
from pathlib import Path
import hashlib,os,shutil,socket,subprocess,tempfile,threading
PROJECT=Path(__file__).resolve().parents[1];CLI=PROJECT/'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-native-') as temporary:
 root=Path(temporary);config=root/'settings.toml';config.write_text('automatic_update_checks=false\n')
 env=dict(os.environ,GAMEPKG_CONFIG=str(config),GAMEPKG_HOME=str(root/'library'))
 for name in ['GAMEPKG_DOOM_SYSROOT','GAMEPKG_ZELDA3_SYSROOT','GAMEPKG_SLANG_ROOT']:env.pop(name,None)
 key=root/'key';key.write_text('91'*32);artifacts={}
 def run(*args,good=True):
  r=subprocess.run([str(CLI),*map(str,args)],env=env,capture_output=True,text=True,timeout=120)
  assert (r.returncode==0)==good,r.stdout+r.stderr
  return r.stdout+r.stderr
 def install(name,code,permissions=''):
  source=root/name;shutil.copytree(PROJECT/'examples/minimal',source)
  manifest=source/'manifest.toml';text=manifest.read_text().replace('com.gamepkg.minimal','com.example.'+name)
  if permissions:text=text.replace('network = "none"',permissions)
  manifest.write_text(text);(source/'source/src/lib.rs').write_text(code)
  package=root/(name+'.gamepkg');run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator');run('build','com.example.'+name)
  assert not list((root/'library/builds').rglob('*.wasm')),'Native Build also produced Wasm'
  binary=next((root/'library/builds'/hashlib.sha256(package.read_bytes()).hexdigest()).rglob('game'));artifacts[name]=binary;assert binary.read_bytes().startswith(b'\x7fELF')
  return 'com.example.'+name
 simple=install('simple','pub fn game_tick(_:u32,state:u64)->u64{state+(1u64<<32)}')
 assert 'score: 4' in run('run',simple,'--frames','4')
 loop=install('loop','pub fn game_tick(_:u32,_:u64)->u64{loop{}}')
 assert 'time limit' in run('run',loop,'--frames','1',good=False)
 memory=install('memory','pub fn game_tick(_:u32,_:u64)->u64{let mut memory=Vec::new();memory.resize(32*1024*1024,0u8);gamepkg::rom_read(0,&mut memory[..16]);memory.len() as u64}')
 memory_error=run('run',memory,'--frames','1',good=False)
 assert 'native game stopped' in memory_error or 'time limit' in memory_error,memory_error
 for udp in [False,True]:
  server=socket.socket(socket.AF_INET,socket.SOCK_DGRAM if udp else socket.SOCK_STREAM);server.bind(('127.0.0.1',0));server.settimeout(10)
  if not udp:server.listen(1)
  endpoint='127.0.0.1:'+str(server.getsockname()[1]);errors=[]
  def echo():
   try:
    if udp:
     data,peer=server.recvfrom(16);assert data==b'ping';server.sendto(b'pong',peer)
    else:
     stream,_=server.accept()
     with stream:
      stream.settimeout(10);assert stream.recv(4)==b'ping';stream.sendall(b'pong')
   except BaseException as e:errors.append(e)
  worker=threading.Thread(target=echo);worker.start()
  connect='udp_connect' if udp else 'tcp_connect'
  code=f'''pub fn game_tick(_:u32,state:u64)->u64{{let mut bytes=[0u8;16];let handle=state as u32;
   if state==0{{match gamepkg::{connect}("{endpoint}"){{Ok(h)=>u64::from(h),Err(_)=>0}}}}
   else if state>>32==0{{match gamepkg::network_poll(handle,&mut[]){{Ok(_)=>{{match gamepkg::network_send(handle,b"ping"){{Ok(4)=>state|(1u64<<32),_=>state}}}},_=>state}}}}
   else if state>>32==1{{match gamepkg::network_receive(handle,&mut bytes){{Ok(4)=>{{if bytes[..4]==*b"pong"{{(42u64<<32)|u64::from(handle)}}else{{state}}}},_=>state}}}}
   else{{state}}
  }}'''
  name=install('udp' if udp else 'tcp',code,f'network = "multiplayer"\nnetwork_hosts = ["{endpoint}"]\nnetwork_allow_local = true')
  output=run('run',name,'--frames','1000');assert 'score: 42' in output,output
  worker.join(timeout=12);server.close();assert not errors,errors;assert not worker.is_alive()
 denied=install('denied','pub fn game_tick(_:u32,_:u64)->u64{match gamepkg::tcp_connect("127.0.0.1:1"){Err(gamepkg::NetworkError::PermissionDenied)=>99u64<<32,_=>0}}')
 assert 'score: 99' in run('run',denied,'--frames','1')
 # A native artifact cannot run after its verified checksum changes.
 binary=artifacts['simple'];binary.write_bytes(binary.read_bytes()+b'corrupt')
 assert 'integrity' in run('run',simple,'--frames','1',good=False)
 print('Native builds passed: ELF machine code only, native gameplay, deadline/memory termination, TCP/UDP echo, denied undeclared networking and corrupt artifact rejection.')
