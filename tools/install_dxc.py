"""Install the checksum-pinned official DXC Linux x86-64 distribution."""
import argparse, hashlib, platform, tarfile, tempfile, urllib.request
from pathlib import Path
VERSION='1.9.2609'
URL=f'https://github.com/microsoft/DirectXShaderCompiler/releases/download/v{VERSION}/linux_dxc_2026_09_28.x86_x64.tar.gz'
SHA256='96faadc7f5c282d2ffda49804beb4c3ee38127bc252b723234e3c5cdf7aa39a1'
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('destination',type=Path)
args=parser.parse_args()
if platform.system()!='Linux' or platform.machine() not in ('x86_64','amd64'):
    parser.error('this installer supports Linux x86-64')
destination=args.destination.expanduser().absolute()
if destination.exists():parser.error('destination must not exist')
with tempfile.TemporaryDirectory(prefix='gamepkg-dxc-download-') as folder:
    archive=Path(folder)/'dxc.tar.gz'
    digest=hashlib.sha256();total=0
    with urllib.request.urlopen(URL,timeout=120) as response,archive.open('wb') as output:
        while chunk:=response.read(1024*1024):
            total+=len(chunk)
            if total>128*1024*1024:raise RuntimeError('DXC download exceeds 128 MiB')
            digest.update(chunk);output.write(chunk)
    if digest.hexdigest()!=SHA256:raise RuntimeError('DXC checksum mismatch')
    with tarfile.open(archive) as source:
        if sum(member.size for member in source.getmembers())>256*1024*1024:
            raise RuntimeError('DXC archive exceeds 256 MiB')
        source.extractall(destination,filter='data')
print(f'Installed DXC {VERSION} in {destination}')
print(f'Set GAMEPKG_DXC_ROOT={destination} for DXIL shader builds.')
