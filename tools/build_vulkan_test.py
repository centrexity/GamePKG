"""Build a pinned, per-application Vulkan loader for Windows CI only.

No system install or driver registration. These are maintainer test tools.
"""
import argparse
import hashlib
import json
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request
from pathlib import Path
from package_release import executable_architecture

VERSION = '1.4.328.1'
SOURCES = {
    'Vulkan-Headers': 'c465aa56757e7746ac707f582b6e2d51546569a4a2488c1172fb543aa5fdfc2c',
    'Vulkan-Loader': '65a2a063a8a33ff298848bb1e70deff8499f1ea8aa486feaa8b2f507d8e9989d',
}


def source(root, name):
    archive = root / (name + '.tar.gz')
    url = f'https://github.com/KhronosGroup/{name}/archive/refs/tags/vulkan-sdk-{VERSION}.tar.gz'
    digest = hashlib.sha256()
    total = 0
    with urllib.request.urlopen(url, timeout=60) as response, archive.open('xb') as output:
        while block := response.read(1024 * 1024):
            total += len(block)
            assert total <= 16 * 1024 * 1024, 'Vulkan source archive exceeds bound'
            digest.update(block)
            output.write(block)
    assert digest.hexdigest() == SOURCES[name], 'Vulkan source checksum mismatch'
    with tarfile.open(archive) as bundle:
        members = bundle.getmembers()
        assert len(members) <= 4096 and sum(m.size for m in members) <= 128 * 1024 * 1024
        assert all(m.isfile() or m.isdir() for m in members), 'Unexpected Vulkan source member'
        bundle.extractall(root, members=members, filter='data')
    return root / f'{name}-vulkan-sdk-{VERSION}'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--architecture', choices=['x86', 'x64'], required=True)
    args = parser.parse_args()
    destination = args.directory.resolve() / 'vulkan-1.dll'
    assert not destination.exists(), 'Refusing to replace an existing Vulkan loader'
    generator_arch = {'x86': 'Win32', 'x64': 'x64'}[args.architecture]
    with tempfile.TemporaryDirectory(prefix='gamepkg-vulkan-test-') as temporary:
        root = Path(temporary)
        headers, loader = (source(root, name) for name in SOURCES)
        prefix = root / 'install'
        commands = [
            ['cmake', '-S', str(headers), '-B', str(root/'headers-build'), '-A', generator_arch, f'-DCMAKE_INSTALL_PREFIX={prefix}'],
            ['cmake', '--install', str(root/'headers-build'), '--config', 'Release'],
            ['cmake', '-S', str(loader), '-B', str(root/'loader-build'), '-A', generator_arch,
             f'-DCMAKE_PREFIX_PATH={prefix}', f'-DCMAKE_INSTALL_PREFIX={prefix}',
             '-DBUILD_TESTS=OFF', '-DUPDATE_DEPS=OFF', '-DLOADER_CODEGEN=OFF'],
            ['cmake', '--build', str(root/'loader-build'), '--config', 'Release', '--parallel', '2'],
            ['cmake', '--install', str(root/'loader-build'), '--config', 'Release'],
        ]
        for command in commands:
            subprocess.run(command, check=True, timeout=600)
        dll = prefix / 'bin/vulkan-1.dll'
        expected = {'x86': 'i686', 'x64': 'x86_64'}[args.architecture]
        assert executable_architecture(dll, True) == expected, 'Vulkan loader architecture mismatch'
        destination.parent.mkdir(parents=True, exist_ok=True)
        with destination.open('xb') as output, dll.open('rb') as input_file:
            shutil.copyfileobj(input_file, output)
        record = {'version': VERSION, 'architecture': expected, 'sources': SOURCES,
                  'dll_sha256': hashlib.sha256(destination.read_bytes()).hexdigest()}
        (destination.parent/'vulkan-test-loader.json').write_text(json.dumps(record, indent=2)+'\n')
        print('Pinned test-only Vulkan loader built:', json.dumps(record), flush=True)


if __name__ == '__main__':
    main()
