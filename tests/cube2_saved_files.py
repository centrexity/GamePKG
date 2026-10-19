"""Exercise the production C++ save adapter in an isolated filesystem."""
from pathlib import Path
import os, struct, subprocess, tempfile
PROJECT=Path(__file__).resolve().parents[1]
binary=PROJECT/'target/cube2-save-bridge-test'
subprocess.run(['g++','-std=c++17','-O2','-Wall','-Wextra','-Werror',str(PROJECT/'tests/cube2_save_bridge.cpp'),'-o',str(binary)],check=True)
def archive(files):
 payload=b''
 for name,data in files:
  name=name.encode();payload+=struct.pack('<4I',len(name),len(data),0,0)+name+data
 return b'GPB2'+struct.pack('<3I',len(files),len(payload),0)+payload
def call(folder,mode,data=b'',good=True):
 args=['bwrap','--unshare-all','--die-with-parent','--new-session','--clearenv','--ro-bind','/usr','/usr']
 for name in ['/lib','/lib64']:
  if Path(name).exists():args+=['--ro-bind',name,name]
 args+=['--bind',str(folder),'/save','--ro-bind',str(binary),'/test','--proc','/proc','--dev','/dev','--remount-ro','/','--chdir','/save','--','/test',mode]
 result=subprocess.run(args,input=data,capture_output=True,timeout=10)
 assert (result.returncode==0)==good,(mode,result.returncode,result.stderr)
 return result.stdout
def read_tree(data):
 assert data[:4]==b'GPS2'
 count,size,reserved=struct.unpack_from('<3I',data,4);assert size==len(data)-16 and reserved==0
 offset=16;files={}
 for _ in range(count):
  n,size,a,b=struct.unpack_from('<4I',data,offset);assert a==b==0;offset+=16
  name=data[offset:offset+n].decode();offset+=n
  files[name]=data[offset:offset+size];offset+=size
 assert offset==len(data)
 return files
with tempfile.TemporaryDirectory(prefix='cube2-saved-files-') as temporary:
 root=Path(temporary)
 valid=[('auth.cfg',b'authkey public-test 123 ""'),('config.cfg',b'name test'),('demos/match.dmo',b'demo'),('packages/base/map.ogz',b'edited map'),('packages/custom/map.cfg',b'map settings')]
 folder=root/'roundtrip';folder.mkdir()
 assert read_tree(call(folder,'restore',archive(valid)))==dict(valid)
 assert read_tree(call(folder,'collect'))==dict(valid)
 for index,(offset,value) in enumerate([(4,257),(8,32*1048576),(12,1),(16,241),(20,16*1048576+1),(24,1),(28,1)]):
  folder=root/('header-'+str(index));folder.mkdir()
  bad=bytearray(archive([('config.cfg',b'x')]));struct.pack_into('<I',bad,offset,value)
  call(folder,'restore',bytes(bad),good=False)
  assert not list(folder.rglob('*'))
 for index,path in enumerate(['../outside.cfg','/config.cfg','packages/base/../map.ogz','packages//base/map.ogz','packages/base/.hidden.ogz','packages/base/%2e%2e.ogz','packages\\base\\map.ogz','C:/packages/base/map.ogz','packages/base/map.ogz\0','packages/base/map.cpp','data/autoexec.cfg','packages/a/b/c/d/e/f.ogz']):
  folder=root/('bad-'+str(index));folder.mkdir()
  call(folder,'restore',archive([(path,b'bad')]),good=False)
  assert not list(folder.rglob('*')),('Malformed archive wrote a file',path)
 for index,files in enumerate([[('config.cfg',b'a'),('config.cfg',b'b')],[('init.cfg',b'a'),('config.cfg',b'b')],[('packages/base/a.cfg',b'a'),('packages/base/a.cfg/map.ogz',b'b')]]):
  folder=root/('collision-'+str(index));folder.mkdir();call(folder,'restore',archive(files),good=False)
  assert not list(folder.rglob('*'))
 outside=root/'private';outside.write_bytes(b'private')
 folder=root/'linked-parent';(folder/'private/base').mkdir(parents=True)
 (folder/'private/base/map.ogz').write_bytes(b'private')
 (folder/'packages').symlink_to('private',target_is_directory=True)
 call(folder,'restore',archive([('packages/base/map.ogz',b'overwrite')]),good=False)
 assert (folder/'private/base/map.ogz').read_bytes()==b'private'
 for kind in ['symlink','hardlink','fifo','oversize','count','total']:
  folder=root/kind;(folder/'packages/base').mkdir(parents=True)
  path=folder/'packages/base/map.ogz'
  if kind=='symlink':path.symlink_to(outside)
  elif kind=='hardlink':os.link(outside,path)
  elif kind=='fifo':os.mkfifo(path)
  elif kind=='oversize':
   with path.open('wb') as f:f.truncate(16*1048576+1)
  elif kind=='count':
   for i in range(257):(path.parent/f'{i}.ogz').write_bytes(b'')
  else:
   for i in range(3):
    with (path.parent/f'{i}.ogz').open('wb') as f:f.truncate(11*1048576)
  call(folder,'collect',good=False)
  assert outside.read_bytes()==b'private'
print('Cube 2 saved-file adapter passed: round trip, traversal/encoding/type/collision attacks, links, FIFO, file/count/total quotas.')
