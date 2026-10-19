"""Preserve old dependency files in deterministic Windows ZIP distributions."""
import os,sys,tempfile,zipfile
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'tools'))
from package_release import write_zip
with tempfile.TemporaryDirectory(prefix='gamepkg-zip-test-') as directory:
 root=Path(directory);distribution=root/'gamepkg-test';distribution.mkdir()
 old=distribution/'old-notice';old.write_bytes(b'original license notice');os.utime(old,(0,0))
 current=distribution/'program.exe';current.write_bytes(b'test archive content')
 first=root/'first.zip';write_zip(distribution,first)
 os.utime(old,(2000000000,2000000000));os.utime(current,(1000000000,1000000000))
 second=root/'second.zip';write_zip(distribution,second)
 assert first.read_bytes()==second.read_bytes()
 with zipfile.ZipFile(first) as archive:
  assert archive.read('gamepkg-test/old-notice')==b'original license notice'
  assert all(info.date_time==(1980,1,1,0,0,0) for info in archive.infolist())
 print('Windows ZIP preserves original bytes, accepts pre-1980 files and ignores source timestamps.')
