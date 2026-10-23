"""Maintainer-only pinned Windows kernel file-call staging/compile gate.

Uses upstream source and synthetic staging checks, never game assets. The
original source is read only; altered input and in-place output must reject.
"""
import argparse
import hashlib
import os
from pathlib import Path
import re
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True, help='Pinned xboxrecomp checkout')
    parser.add_argument('--target', choices=['x86_64-pc-windows-msvc', 'i686-pc-windows-msvc',
                        'x86_64-pc-windows-gnu', 'i686-pc-windows-gnu'],
                        default='x86_64-pc-windows-msvc' if os.name == 'nt' else 'x86_64-pc-windows-gnu')
    args = parser.parse_args()
    source = args.source.resolve()
    original = source / 'src/kernel/kernel_file.c'
    before = original.read_bytes()
    work = Path(tempfile.mkdtemp(prefix='gamepkg-windows-kernel.'))
    root = Path(__file__).resolve().parents[1]
    adapter = root / 'engines/takedawn/media-bridge'
    staged = work / 'kernel_file.c'

    def stage(input_path, output_path, success):
        output = subprocess.run(['cmake', '-DINPUT=' + str(input_path), '-DOUTPUT=' + str(output_path),
                                 '-P', str(adapter / 'adapt_windows_file.cmake')], capture_output=True)
        if (output.returncode == 0) != success:
            raise RuntimeError(output.stdout.decode(errors='replace') + output.stderr.decode(errors='replace'))

    stage(original, staged, True)
    text = staged.read_text()
    apis = re.findall(r'gpw_([A-Za-z0-9_]+)\(', (adapter / 'windows_file.h').read_text())
    expected = 0
    for api in apis:
        count = len(re.findall(r'\b' + api + r'\(', before.decode()))
        expected += count
        assert count and len(re.findall(r'\bgpw_' + api + r'\(', text)) == count, api
        assert not re.search(r'\b' + api + r'\(', text), api
    restored = re.sub(r'\bgpw_(' + '|'.join(apis) + r')\(', r'\1(', text)
    include = '#if defined(_WIN32)\n#include "' + (adapter / 'windows_file.h').as_posix() + '"\n#endif\n'
    assert restored.count(include) == 1
    restored = restored.replace(include, '')
    assert restored == before.decode(), 'Staging changed code beyond the explicit host API routing'
    stage(original, original, False)
    altered = work / 'altered.c'
    altered.write_bytes(before + b'\n/* source revision changed */\n')
    rejected = work / 'must-not-exist.c'
    stage(altered, rejected, False)
    assert not rejected.exists() and original.read_bytes() == before
    obj = work / 'kernel_file.obj'
    if args.target.endswith('-msvc'):
        command = ['cl', '/nologo', '/std:c11', '/O2', '/W3', '/c',
                   '/I' + str(source / 'src'), '/I' + str(source / 'src/kernel'),
                   str(staged), '/Fo' + str(obj)]
    else:
        cc = args.target.replace('pc-windows-gnu', 'w64-mingw32') + '-gcc'
        command = [cc, '-std=c11', '-O2', '-Wall', '-Wextra', '-Werror', '-Wno-unused-parameter',
                   '-isystem', str(source / 'src'), '-isystem', str(source / 'src/kernel'),
                   '-c', str(staged), '-o', str(obj)]
    subprocess.run(command, cwd=work, check=True)
    assert obj.stat().st_size > 0 and original.read_bytes() == before
    print(f'PASS: {expected} original Win32 calls routed; source restored byte-for-byte; changed/in-place input rejected; {args.target} compiled')
    print('Original SHA-256:', hashlib.sha256(before).hexdigest())
    print('Private evidence directory:', work)


if __name__ == '__main__':
    main()
