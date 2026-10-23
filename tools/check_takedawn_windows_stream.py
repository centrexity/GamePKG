"""Maintainer-only native Windows startup/SDL stream fixture, without an ISO.

An explicit SDL development root can be reused. MSVC otherwise fetches the
fixed official SDL development archive and verifies its published SHA-256.
Python and this dependency fetch are never part of the player launcher.
"""
import argparse
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import urllib.request
import zipfile

SDL_VERSION = '2.32.10'
SDL_VC_SHA256 = 'af347939395a58b365846aaea27391e69f9ec9d4dd650d6ac40802159b418a6e'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', choices=['x86_64-pc-windows-msvc', 'i686-pc-windows-msvc',
                        'x86_64-pc-windows-gnu', 'i686-pc-windows-gnu'],
                        default='x86_64-pc-windows-msvc' if os.name == 'nt' else 'x86_64-pc-windows-gnu')
    parser.add_argument('--sdl', type=Path)
    args = parser.parse_args()
    work = Path(tempfile.mkdtemp(prefix='gamepkg-windows-stream.'))
    print('Private evidence directory:', work, flush=True)
    root = Path(__file__).resolve().parents[1] / 'engines/takedawn/media-bridge'
    msvc = args.target.endswith('-msvc')
    if args.sdl:
        sdl = args.sdl.resolve()
    elif msvc:
        archive = work / 'SDL2-devel.zip'
        url = f'https://github.com/libsdl-org/SDL/releases/download/release-{SDL_VERSION}/SDL2-devel-{SDL_VERSION}-VC.zip'
        with urllib.request.urlopen(url, timeout=60) as response:
            archive.write_bytes(response.read(8 * 1024 * 1024))
        if hashlib.sha256(archive.read_bytes()).hexdigest() != SDL_VC_SHA256:
            raise RuntimeError('Official SDL development archive digest mismatch')
        with zipfile.ZipFile(archive) as package:
            for member in package.infolist():
                name = Path(member.filename)
                if name.is_absolute() or '..' in name.parts:
                    raise RuntimeError('Unsafe SDL archive member')
            package.extractall(work / 'sdl')
        sdl = work / 'sdl' / f'SDL2-{SDL_VERSION}'
    else:
        parser.error('GNU cross tests require --sdl pointing at the official mingw development package')
    sources = [root / 'windows_file.c', root / 'windows_stream.c', root / 'tests/windows_stream.c']
    executable = work / 'streams.exe'
    if msvc:
        arch = 'x86' if args.target.startswith('i686') else 'x64'
        objects = []
        for index, source in enumerate(sources):
            obj = work / f'stream-{index}.obj'
            subprocess.run(['cl', '/nologo', '/std:c11', '/O2', '/UNDEBUG', '/DSDL_MAIN_HANDLED',
                            '/W4', '/WX', '/I' + str(sdl / 'include'), '/c', str(source),
                            '/Fo' + str(obj)], cwd=work, check=True)
            objects.append(str(obj))
        command = ['cl', '/nologo', *objects, str(sdl / 'lib' / arch / 'SDL2.lib'), '/Fe' + str(executable)]
        dll = sdl / 'lib' / arch / 'SDL2.dll'
    else:
        arch = args.target.replace('pc-windows-gnu', 'w64-mingw32')
        base = sdl / arch
        command = [arch + '-gcc', '-std=c11', '-O2', '-UNDEBUG', '-DSDL_MAIN_HANDLED',
                   '-Wall', '-Wextra', '-Werror', '-I', str(base / 'include/SDL2'),
                   *map(str, sources), '-L', str(base / 'lib'), '-lSDL2', '-o', str(executable)]
        dll = base / 'bin/SDL2.dll'
    subprocess.run(command, cwd=work, check=True)
    shutil.copyfile(dll, work / 'SDL2.dll')
    if os.name != 'nt':
        raise RuntimeError('The fixture includes native Win32 oracles that differ in Wine; execute on actual Windows')
    result = subprocess.run([str(executable)], cwd=work, capture_output=True, timeout=90)
    (work / 'stdout.log').write_bytes(result.stdout)
    (work / 'stderr.log').write_bytes(result.stderr)
    print(result.stdout.decode(errors='replace').strip(), flush=True)
    if result.returncode:
        print(result.stderr.decode(errors='replace'), flush=True)
    result.check_returncode()


if __name__ == '__main__':
    main()
