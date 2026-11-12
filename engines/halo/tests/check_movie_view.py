"""Maintainer-only generated movie namespace/adversarial/concurrency tests."""
from pathlib import Path
import os,struct,subprocess,tempfile,argparse
root=Path(__file__).resolve().parent
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--source',type=Path,required=True)
source=parser.parse_args().source.resolve()
c=(root/'movie_disc_view.c').read_text()
def entry(name,sector,size,directory=False,right=0):
 if isinstance(name,str):name=name.encode()
 return struct.pack('<HHIIBB',0,right,sector,size,16 if directory else 0,len(name))+name
def image(path,fault):
 directories=entry('bink',43,64,True,right=8).ljust(32,b'\xff')+entry('maps',42,64,True)
 movies=bytearray(entry('intro.bik',45,32).ljust(64,b'\xff'))
 if fault=='traversal':movies[:21]=entry('../evil',45,32)
 if fault=='colon':movies[:24]=entry('intro:evil',45,32)
 if fault=='nul':movies[:23]=entry(b'intro\x00bik',45,32)
 if fault=='duplicate':struct.pack_into('<H',movies,2,8);movies[32:55]=entry('INTRO.BIK',45,32)
 if fault=='cycle':struct.pack_into('<H',movies,2,8);movies[32:55]=entry('other.bik',45,32,right=8)
 if fault=='extent':struct.pack_into('<I',movies,8,0xffffffff)
 if fault=='link':struct.pack_into('<H',movies,2,65534)
 directories=bytearray(directories.ljust(64,b'\xff'))
 if fault=='directory-extent':struct.pack_into('<I',directories,4,0xffffffff)
 if fault=='directory-size':struct.pack_into('<I',directories,8,0xffffffff)
 descriptor=bytearray(2048);descriptor[:20]=descriptor[0x7ec:0x800]=b'MICROSOFT*XBOX*MEDIA'
 struct.pack_into('<II',descriptor,0x14,40,64)
 with path.open('wb') as f:
  for at,data in [(0x10000,descriptor),(40*2048,directories),(42*2048,entry('ui.map',44,10).ljust(64,b'\xff')),
   (43*2048,movies),(44*2048,b'0123456789'),(45*2048,bytes(range(32)))]:f.seek(at);f.write(data)
with tempfile.TemporaryDirectory(prefix='gamepkg-movie-view-') as tmp:
 tmp=Path(tmp);(tmp/'check.c').write_text(c)
 (tmp/'stub.h').write_text('#define __HALO_LINUX_PLATFORM_H\nvoid platform_log(const char *, ...);\n')
 for profile,flags in [('native64-sanitized',['-fsanitize=address,undefined']),('native32',['-m32','-isystem','/usr/include/x86_64-linux-gnu'])]:
  binary=tmp/profile
  subprocess.run(['/usr/bin/clang-18','-g','-O1','-D_GNU_SOURCE','-D_FILE_OFFSET_BITS=64','-pthread',*flags,
   '-include',str(tmp/'stub.h'),'-I',str(source/'port/linux/src'),str(tmp/'check.c'),str(source/'port/linux/src/xiso.c'),str(source/'port/linux/src/posix_files.c'),'-o',str(binary)],check=True)
  for fault in [None,'traversal','colon','nul','duplicate','cycle','extent','link','directory-extent','directory-size']:
   disc=tmp/'fixture.iso';image(disc,fault)
   subprocess.run([str(binary),str(disc),'bad' if fault else 'good'],check=True,
    env=dict(os.environ,ASAN_OPTIONS='detect_leaks=1:abort_on_error=1',UBSAN_OPTIONS='halt_on_error=1'))
   print('PASS',profile,fault or 'namespace/readonly/8000-concurrent-reads',flush=True)
