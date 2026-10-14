"""Exercise the extracted Windows host distribution on a Windows runner."""
import argparse,hashlib,json,os,platform,subprocess,tempfile,zipfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('archive',type=Path);args=p.parse_args()
assert platform.system()=='Windows','Run this test on Windows'
archive=args.archive.resolve();checksum=archive.with_suffix(archive.suffix+'.sha256').read_text().split()[0]
assert hashlib.sha256(archive.read_bytes()).hexdigest()==checksum
with tempfile.TemporaryDirectory(prefix='gamepkg-windows-package-') as directory:
 root=Path(directory)
 with zipfile.ZipFile(archive) as bundle:bundle.extractall(root)
 distribution=root/archive.stem;metadata=json.loads((distribution/'BUILD.json').read_text());assert metadata['platform'] in ['windows-x86_64','windows-i686']
 for port in ['2048','arkanoo','zelda3']:assert (distribution/'ports'/port/'manifest.toml').is_file()
 assert not list((distribution/'ports').rglob('.git'))
 assert (distribution/'third-party-sources').is_dir();assert json.loads((distribution/'DEPENDENCIES.json').read_text())
 env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'))
 def run(exe,*args):
  r=subprocess.run([str(distribution/'bin'/exe),*map(str,args)],env=env,capture_output=True,text=True,timeout=30);assert r.returncode==0,r.stderr;return r.stdout
 for exe,digest in metadata['executables_sha256'].items():
  assert hashlib.sha256((distribution/'bin'/exe).read_bytes()).hexdigest()==digest
  assert metadata['version'] in run(exe,'--version')
 key=root/'creator.key';package=root/'example.gamepkg'
 run('gamepkg.exe','keygen','-o',key);run('gamepkg.exe','pack',distribution/'examples/good/collect-stars','-o',package,'--key',key)
 run('gamepkg.exe','verify',package);run('gamepkg.exe','install',package,'--trust-creator')
 assert 'com.gamepkg.collect-stars' in run('gamepkg.exe','list')
print('Extracted Windows distribution passed: checksum, executable versions, dependency sources, signed package verification and installation.')
