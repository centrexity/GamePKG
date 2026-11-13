#!/usr/bin/env python3
"""Maintainer/CI setup for generated codec fixtures, never a player build step."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request

APP = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True, help='pinned engine with reviewed patches')
    parser.add_argument('--work', type=Path, required=True, help='new, empty fixture directory')
    parser.add_argument('--profile', choices=['native32', 'sanitized64'], required=True)
    parser.add_argument('--cc', default='/usr/bin/clang-18')
    args = parser.parse_args()
    work = args.work.resolve()
    work.mkdir(parents=True, exist_ok=True)
    assert not any(work.iterdir()), 'Use an empty fixture directory'
    plan = json.loads((APP/'engines/halo/prepared/linux-i686/plan.json').read_text())

    def fetch(suffix):
        entry = next(x for x in plan['downloads'] if x['output'].endswith('/'+suffix))
        packed = None
        for url in entry['urls']:
            assert url.startswith('https://')
            try:
                with urllib.request.urlopen(url, timeout=60) as response:
                    data = response.read(128*1024*1024+1)
                if len(data) <= 128*1024*1024 and hashlib.sha256(data).hexdigest() == entry['sha256']:
                    packed = data
                    break
            except OSError as error:
                print(f'Dependency mirror unavailable: {error}', flush=True)
        assert packed, 'Could not fetch verified '+suffix
        archive = work/(suffix+'.tar')
        archive.write_bytes(packed)
        with tempfile.TemporaryDirectory(dir=work) as temporary:
            with tarfile.open(archive) as tar:
                tar.extractall(temporary, filter='data')
            directories = list(Path(temporary).iterdir())
            assert len(directories) == 1 and directories[0].is_dir()
            source = work/suffix
            shutil.move(str(directories[0]), source)
        archive.unlink()
        return source

    sdl, ffmpeg = fetch('SDL3'), fetch('ffmpeg')

    def run(command, directory, name):
        with (work/(name+'.log')).open('w') as log:
            result = subprocess.run(command, cwd=directory, stdout=log, stderr=subprocess.STDOUT, timeout=900)
        if result.returncode:
            raise RuntimeError(f'{name} failed:\n{(work/(name+".log")).read_text()[-5000:]}')

    bits = ['--target=i686-linux-gnu', '-m32'] if args.profile == 'native32' else []
    fixture_flags = []
    if args.profile == 'native32' and Path('/usr/include/x86_64-linux-gnu').is_dir():
        bits += ['-isystem', '/usr/include/x86_64-linux-gnu']
        fixture_flags = ['--cflag=-isystem', '--cflag=/usr/include/x86_64-linux-gnu']
    sdl_build = work/'sdl-build'
    run(['cmake', '-S', str(sdl), '-B', str(sdl_build), '-G', 'Ninja',
        '-DCMAKE_C_COMPILER='+args.cc, '-DCMAKE_C_FLAGS='+' '.join(bits), '-DCMAKE_BUILD_TYPE=Release',
        '-DSDL_SHARED=ON', '-DSDL_STATIC=OFF', '-DSDL_TESTS=OFF', '-DSDL_TEST_LIBRARY=OFF',
        '-DSDL_UNIX_CONSOLE_BUILD=ON', '-DSDL_X11=OFF', '-DSDL_WAYLAND=OFF', '-DSDL_OPENGL=OFF',
        '-DSDL_OPENGLES=OFF', '-DSDL_VULKAN=OFF', '-DSDL_ALSA=OFF', '-DSDL_PULSEAUDIO=OFF',
        '-DSDL_PIPEWIRE=OFF', '-DSDL_JACK=OFF', '-DSDL_SNDIO=OFF', '-DSDL_DUMMYAUDIO=ON'], work, 'sdl-configure')
    run(['cmake', '--build', str(sdl_build), '--target', 'SDL3-shared', '--parallel', '2'], work, 'sdl-build')
    options = ['--arch=x86_32', '--target-os=linux'] if args.profile == 'native32' else [
        '--extra-cflags=-O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer',
        '--extra-ldflags=-fsanitize=address,undefined']
    run(['./configure', '--cc='+' '.join([args.cc, *bits]), *options,
        '--disable-autodetect', '--disable-everything', '--disable-programs', '--disable-doc', '--disable-debug',
        '--disable-network', '--disable-avdevice', '--disable-avfilter', '--disable-x86asm', '--disable-inline-asm',
        '--enable-static', '--disable-shared', '--enable-decoder=bink,binkaudio_rdft,binkaudio_dct',
        '--enable-demuxer=bink', '--disable-protocols'], ffmpeg, 'ffmpeg-configure')
    run(['make', '-j2'], ffmpeg, 'ffmpeg-build')
    run(['python3', str(APP/'engines/halo/tests/check_movie_playback.py'), '--source', str(args.source.resolve()),
        '--ffmpeg', str(ffmpeg), '--sdl-lib', str(sdl_build), '--sdl-include', str(sdl/'include'),
        '--cc', args.cc, '--profile', args.profile, *fixture_flags, '--output', str(work/'results')], work, 'fixtures')
    print(f'PASS {args.profile}: verified dependency builds and 25 generated movie cases', flush=True)


if __name__ == '__main__':
    main()
