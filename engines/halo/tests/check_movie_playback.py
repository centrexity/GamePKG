#!/usr/bin/env python3
"""Maintainer-only generated movie fixtures; no game media or player Python."""
import argparse
import json
import os
from pathlib import Path
import resource
import struct
import subprocess
import tempfile
import time
from synthetic_movie import image, movie

HERE = Path(__file__).resolve().parent


def fixtures():
    for audio in [None, 'rdft', 'dct']:
        yield 'valid-'+str(audio), movie(audio, (2997, 100)), 'valid'
    original = movie('rdft', (2997, 100))
    for name, offset, value in [
        ('size-short', 4, len(original)-9), ('size-long', 4, len(original)-7),
        ('zero-frames', 8, 0), ('huge-frames', 8, 1000001),
        ('zero-width', 20, 0), ('huge-width', 20, 2049),
        ('zero-height', 24, 0), ('huge-height', 24, 2049),
        ('zero-fps', 28, 0), ('high-fps', 28, 24100), ('zero-fps-den', 32, 0),
        ('huge-streams', 40, 18), ('zero-sample-rate', 52, 0),
        ('bad-first-offset', 60, 0xfffffff1), ('bad-second-offset', 64, 0xfffffff1),
    ]:
        data = bytearray(original)
        struct.pack_into('<I', data, offset, value)
        yield name, data, 'invalid-media'
    yield 'bad-magic', b'BAD!'+original[4:], 'invalid-media'
    for cut in [1, 8, 47, 70, 100, len(original)-13]:
        data = bytearray(original[:cut])
        if len(data) >= 8:
            struct.pack_into('<I', data, 4, len(data)-8)
        yield f'truncated-{cut}', data, 'invalid-media'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--ffmpeg', type=Path, required=True, help='configured and built pinned FFmpeg source')
    parser.add_argument('--library-dir', type=Path, help='flat archive directory for prepared Ninja builds')
    parser.add_argument('--sdl-lib', type=Path, required=True)
    parser.add_argument('--sdl-include', type=Path, required=True)
    parser.add_argument('--sysroot', type=Path)
    parser.add_argument('--cc', default='clang-18')
    parser.add_argument('--profile', choices=['native32', 'sanitized64'], required=True)
    parser.add_argument('--cflag', action='append', default=[])
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    source, ffmpeg, sdl = args.source.resolve(), args.ffmpeg.resolve(), args.sdl_lib.resolve()
    options = ['--target=i686-linux-gnu', '-m32', '-msse2', '-mstackrealign'] if args.profile == 'native32' else [
        '-fsanitize=address,undefined', '-fno-omit-frame-pointer']
    if args.sysroot:
        options.append('--sysroot='+str(args.sysroot.resolve()))
    options += args.cflag
    libraries = [args.library_dir.resolve()/f'lib{x}.a' if args.library_dir else ffmpeg/f'lib{x}/lib{x}.a'
                 for x in ['avformat', 'avcodec', 'swscale', 'swresample', 'avutil']]
    binary = output/'movie-playback-test'
    command = [args.cc, *options, '-D_FILE_OFFSET_BITS=64', '-fms-extensions', '-std=gnu17', '-O1', '-g',
        '-ffp-contract=off', '-Wall', '-Wextra', '-Werror', '-I'+str(args.sdl_include.resolve()),
        '-I'+str(source/'port/linux/src'), '-I'+str(ffmpeg), str(HERE/'movie_playback.c'),
        str(source/'port/linux/src/posix_files.c'), '-Wl,--start-group', *map(str, libraries),
        '-Wl,--end-group', '-L'+str(sdl), '-l:libSDL3.so.0', '-lm', '-pthread', '-ldl', '-o', str(binary)]

    def run(command, log, environment=None, expected_success=True):
        def limits():
            resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
        with log.open('w') as stream:
            result = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT, env=environment,
                                    preexec_fn=limits, timeout=180)
        if expected_success and result.returncode:
            raise RuntimeError(f'{log.name}: exit {result.returncode}\n{log.read_text()[-4000:]}')
        return result.returncode

    run(command, output/'build.log')
    if args.profile == 'native32':
        failed = run(command[:1]+['-malign-double']+command[1:], output/'wrong-abi.log', expected_success=False)
        assert failed and 'codec bridge must use the native i686 library ABI' in (output/'wrong-abi.log').read_text()
    environment = dict(os.environ, LD_LIBRARY_PATH=str(sdl), ASAN_OPTIONS='detect_leaks=1:halt_on_error=1',
                       UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1')
    results = []
    for name, data, scenario in fixtures():
        with tempfile.TemporaryDirectory(prefix='gamepkg-generated-codec-') as temporary:
            iso = Path(temporary)/'fixture.iso'
            image(iso, data)
            began = time.monotonic()
            run([str(binary), str(iso), scenario], output/(name+'.log'), environment)
            results.append(dict(profile=args.profile, fixture=name, result='pass', seconds=round(time.monotonic()-began, 3)))
            print(f'PASS {args.profile}: {name}', flush=True)
    (output/'result.json').write_text(json.dumps(results, indent=2)+'\n')
    print(f'PASS {len(results)} generated scenarios; no game media used')


if __name__ == '__main__':
    main()
