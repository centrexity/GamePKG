"""Maintainer-only native Windows CLI/GUI SDK setup gate; no game media.

Python orchestrates CI. The player installer itself is Rust and uses Windows'
standard curl for the pinned archive downloads, with no Python or WSL command.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--gamepkg',required=True,type=Path)
args=parser.parse_args()
if os.name!='nt':parser.error('Run on actual Windows')
exe=args.gamepkg.resolve(strict=True)
work=Path(tempfile.mkdtemp(prefix='gamepkg-managed-sdk.'))
root=work/'Windows library with spaces'
env=dict(os.environ,GAMEPKG_HOME=str(root),PATH=str(Path(os.environ['SystemRoot'])/'System32'))
print('Private managed SDK evidence:',work,flush=True)
for phase in ['install','reuse']:
 result=subprocess.run([str(exe),'setup-native-tools'],env=env,capture_output=True,timeout=1800)
 (work/(phase+'.stdout.log')).write_bytes(result.stdout)
 (work/(phase+'.stderr.log')).write_bytes(result.stderr)
 if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
 if b'Verified native Windows build tools ready:' not in result.stdout:raise RuntimeError('SDK setup did not confirm success')
 if phase=='reuse' and b'preparing native Windows build tools' in result.stdout:raise RuntimeError('Verified SDK reuse unexpectedly reinstalls components')
installed=[p for p in (root/'native-sdk').iterdir() if p.is_dir() and not p.name.startswith('.')]
assert len(installed)==1
sdk=installed[0]
pins=json.loads((Path(__file__).resolve().parents[1]/'engines/takedawn/windows-dependencies.json').read_text())
receipt=json.loads((sdk/'gamepkg-sdk.json').read_text())
files=json.dumps(receipt['files'],sort_keys=True,ensure_ascii=False,separators=(',',':')).encode()
assert hashlib.sha256(files).hexdigest()==pins['payload_inventory_sha256']
for relative,record in receipt['files'].items():
 path=sdk/relative
 assert path.stat().st_size==record['bytes'] and hashlib.sha256(path.read_bytes()).hexdigest()==record['sha256'],relative
for relative in ['rust-1.89.0-windows-gnu/bin/rustc.exe','rust-1.89.0-windows-gnu/bin/cargo.exe','llvm-mingw-windows/bin/x86_64-w64-mingw32-clang.exe','cmake-windows/bin/cmake.exe','ninja-windows/ninja.exe']:
 result=subprocess.run([str(sdk/relative),'--version'],env=env,capture_output=True,timeout=30)
 assert result.returncode==0,(relative,result.stderr)
print('PASS: native app CLI installs and reuses the official pinned SDK, matches every reviewed payload file, and runs native tools without Python/WSL setup.',flush=True)
