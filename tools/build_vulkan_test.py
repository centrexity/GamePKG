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
             '-DBUILD_TESTS=OFF', '-DUPDATE_DEPS=OFF', '-DLOADER_CODEGEN=OFF',
             # Hosted Windows runners are elevated. This private test loader
             # must honor the explicitly selected private ICD instead of
             # requiring registry/system driver changes. Never ship this DLL.
             '-DLOADER_USE_UNSAFE_FILE_SEARCH=ON'],
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
        import_libraries = [p for p in [prefix/'lib/vulkan-1.lib',prefix/'lib/libvulkan-1.dll.a'] if p.is_file()]
        assert len(import_libraries) == 1
        probe = root / 'probe'
        probe.mkdir()
        probe_source = (Path(__file__).resolve().parent/'windows_vulkan_test_probe.c').as_posix()
        (probe/'CMakeLists.txt').write_text(
            'cmake_minimum_required(VERSION 3.22.1)\n'
            'project(GamePkgVulkanProbe C)\n'
            'set(CMAKE_C_STANDARD 11)\n'
            'set(CMAKE_MSVC_RUNTIME_LIBRARY MultiThreaded)\n'
            f'add_executable(gamepkg-vulkan-test-probe "{probe_source}")\n'
            f'target_include_directories(gamepkg-vulkan-test-probe PRIVATE "{(prefix/"include").as_posix()}")\n'
            f'target_link_libraries(gamepkg-vulkan-test-probe PRIVATE "{import_libraries[0].as_posix()}" advapi32)\n')
        subprocess.run(['cmake','-S',str(probe),'-B',str(root/'probe-build'),'-A',generator_arch,
                        f'-DCMAKE_PREFIX_PATH={prefix}'],check=True,timeout=120)
        subprocess.run(['cmake','--build',str(root/'probe-build'),'--config','Release'],check=True,timeout=120)
        candidates = list((root/'probe-build').rglob('gamepkg-vulkan-test-probe.exe'))
        assert len(candidates) == 1 and executable_architecture(candidates[0],True) == expected
        with (destination.parent/'gamepkg-vulkan-test-probe.exe').open('xb') as output, candidates[0].open('rb') as input_file:
            shutil.copyfileobj(input_file,output)
        record = {'version': VERSION, 'architecture': expected, 'sources': SOURCES,
                  'test_only_env_driver_selection': True,
                  'dll_sha256': hashlib.sha256(destination.read_bytes()).hexdigest()}
        (destination.parent/'vulkan-test-loader.json').write_text(json.dumps(record, indent=2)+'\n')
        print('Pinned test-only Vulkan loader built:', json.dumps(record), flush=True)


if __name__ == '__main__':
    main()
