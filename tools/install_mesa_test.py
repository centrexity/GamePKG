"""Prepare pinned per-application Mesa DLLs for native Windows CI rendering only."""
import argparse,hashlib,shutil,subprocess,tempfile,urllib.request
from pathlib import Path
VERSION='26.2.4';SHA256='351fc8c8b695878ffb3eaa044b3ead08672a48b1a045e3c3e3975811df0f6695'
URL=f'https://github.com/pal1000/mesa-dist-win/releases/download/{VERSION}/mesa3d-{VERSION}-release-msvc.7z'
p=argparse.ArgumentParser(description=__doc__);p.add_argument('directory',type=Path);args=p.parse_args()
seven=shutil.which('7z') or str(Path('C:/Program Files/7-Zip/7z.exe'))
with tempfile.TemporaryDirectory(prefix='gamepkg-mesa-test-') as temporary:
 root=Path(temporary);archive=root/'mesa.7z';digest=hashlib.sha256();total=0
 with urllib.request.urlopen(URL,timeout=60) as response,archive.open('xb') as output:
  while block:=response.read(1024*1024):
   total+=len(block);assert total<=90*1024*1024,'Mesa test archive exceeds its bound'
   digest.update(block);output.write(block)
 assert digest.hexdigest()==SHA256,'Mesa test archive checksum mismatch'
 extracted=root/'files'
 subprocess.run([seven,'x','-y','-o'+str(extracted),str(archive)],check=True,capture_output=True,timeout=90)
 source=extracted/'x64'
 assert (source/'opengl32.dll').is_file() and (source/'libgallium_wgl.dll').is_file()
 dlls=list(source.glob('*.dll'));assert len(dlls)<=64
 args.directory.mkdir(parents=True,exist_ok=True)
 assert all(not (args.directory/file.name).exists() for file in dlls),'Refusing to replace existing graphics DLLs'
 for file in dlls:shutil.copy2(file,args.directory/file.name)
print('Pinned Mesa test runtime prepared:',VERSION,SHA256)
