#!/usr/bin/env python3
"""Maintainer C/Rust child-local mutex ABI; no end-user C/Python requirement."""
import argparse,hashlib,json,os,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--evidence',type=Path,required=True);p.add_argument('--wine',action='store_true');a=p.parse_args();app=Path(__file__).resolve().parent.parent;out=a.evidence.resolve();out.mkdir(exist_ok=False)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
files=[*sorted((app/'engines/native-services').glob('*.rs')),app/'include/gamepkg_port.h',app/'tests/fixtures/local_mutex_ffi.c',app/'tests/fixtures/local_mutex_ffi.rs',Path(__file__).resolve()];hashes={str(p.relative_to(app)):sha(p)for p in files};results=[]
with tempfile.TemporaryDirectory(prefix='gamepkg-mutex-c-ffi-')as temp:
 root=Path(temp)
 for target,cc,flags in [('x86_64-unknown-linux-gnu','gcc',[]),('i686-unknown-linux-gnu','gcc-11',['-m32']),('x86_64-pc-windows-gnu','x86_64-w64-mingw32-gcc',[]),('i686-pc-windows-gnu','i686-w64-mingw32-gcc',[])]:
  obj=root/(target+'.o');exe=root/(target+('.exe'if'windows'in target else''));log=out/(target+'.log')
  with log.open('w')as f:
   subprocess.run([cc,*flags,'-std=c11','-Wall','-Wextra','-Werror','-O2','-I',str(app/'include'),'-c',str(app/'tests/fixtures/local_mutex_ffi.c'),'-o',str(obj)],check=True,stdout=f,stderr=subprocess.STDOUT)
   command=['rustc','--edition=2021','-D','warnings','-O','--target',target,str(app/'tests/fixtures/local_mutex_ffi.rs'),'-C','link-arg='+str(obj),'-o',str(exe)]
   if target=='i686-unknown-linux-gnu':command+=['-C','linker=gcc-11','-C','link-arg=-lssp_nonshared']
   if'windows'in target:command+=['-C','link-arg=-lmingwex','-C','link-arg=-lmsvcrt']
   subprocess.run(command,check=True,stdout=f,stderr=subprocess.STDOUT)
   execute='windows'not in target or a.wine
   if execute:subprocess.run(['wine',str(exe)]if'windows'in target else[str(exe)],check=True,stdout=f,stderr=subprocess.STDOUT,timeout=30,env=dict(os.environ,WINEDEBUG='-all',WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
  results.append({'target':target,'execution':'Wine'if execute and'windows'in target else'native Linux'if execute else'build-only','C_compiler':cc,'log_sha256':sha(log)});print('PASS C/Rust local ABI '+target,flush=True)
assert hashes=={str(p.relative_to(app)):sha(p)for p in files},'source changed during qualification'
(out/'qualification.json').write_text(json.dumps({'schema':1,'sources':hashes,'results':results,'status':'passed','scope':'actual child-local ABI; no private broker transport, title entry, physical controls, audible audio, GPU fidelity or frame pacing'},indent=2)+'\n')
