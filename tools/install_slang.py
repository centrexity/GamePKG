"""Install the pinned official Linux x86-64 Slang distribution into a new folder."""
import argparse
from pathlib import Path
import platform
import tarfile
import tempfile
from ci_download import download
VERSION = '2026.19'
URL = f'https://github.com/shader-slang/slang/releases/download/v{VERSION}/slang-{VERSION}-linux-x86_64-glibc-2.27.tar.gz'
SHA256 = '5899bd40c3d1ee60eadd1d1b37acc4e88ae65cc25ac61651534d55b71a85fe75'
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('destination', type=Path)
args = parser.parse_args()
if platform.system() != 'Linux' or platform.machine() not in ('x86_64', 'amd64'):
    parser.error('this installer supports Linux x86-64')
destination = args.destination.expanduser().absolute()
if destination.exists():
    parser.error('destination must not exist; choose a new installation folder')
with tempfile.TemporaryDirectory(prefix='gamepkg-slang-download-') as folder:
    archive = Path(folder) / 'slang.tar.gz'
    download(URL, archive, SHA256, timeout=120, max_bytes=256 * 1024 * 1024)
    with tarfile.open(archive) as source:
        members = source.getmembers()
        if sum(member.size for member in members) > 1024 * 1024 * 1024:
            raise RuntimeError('compiler archive exceeds 1 GiB')
        source.extractall(destination, filter='data')
print(f'Installed Slang {VERSION} in {destination}')
print(f'Set GAMEPKG_SLANG_ROOT={destination} or enter this folder in Settings.')
