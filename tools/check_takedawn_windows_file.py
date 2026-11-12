"""Maintainer-only Windows disc-file API oracle. Uses synthetic data, no ISO.

Native Windows CI executes directly. A non-Windows maintainer may use Wine;
that runner is never a player dependency. All test files live in a fresh
temporary directory and the native oracle file is self-deleting.
"""
import argparse
import os
import shutil
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', default=('x86_64-pc-windows-msvc' if os.name == 'nt'
                        else 'x86_64-pc-windows-gnu'), choices=[
        'x86_64-pc-windows-msvc', 'i686-pc-windows-msvc',
        'x86_64-pc-windows-gnu', 'i686-pc-windows-gnu'])
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1] / 'engines/takedawn/media-bridge'
    work = Path(tempfile.mkdtemp(prefix='gamepkg-windows-disc-file.'))
    print('Private evidence directory:', work, flush=True)
    executable = work / 'compare.exe'
    sources = [str(root / 'windows_file.c'), str(root / 'tests/windows_file.c')]
    if args.target.endswith('-msvc'):
        # Both inputs are named windows_file.c; cl's default .obj names would
        # overwrite each other before linking and discard the test's main.
        objects = []
        for index, source in enumerate(sources):
            obj = work / f'disc-{index}.obj'
            subprocess.run(['cl', '/nologo', '/std:c11', '/O2', '/UNDEBUG', '/W4', '/WX',
                            '/c', source, '/Fo' + str(obj)], cwd=work, check=True)
            objects.append(str(obj))
        command = ['cl', '/nologo', *objects, '/Fe' + str(executable)]
    else:
        cc = args.target.replace('pc-windows-gnu', 'w64-mingw32') + '-gcc'
        command = [cc, '-std=c11', '-O2', '-UNDEBUG', '-Wall', '-Wextra', '-Werror',
                   *sources, '-o', str(executable)]
    subprocess.run(command, cwd=work, check=True)
    environment = dict(os.environ)
    wine = os.name != 'nt'
    if wine:
        environment.update(WINEPREFIX=str(work / 'wine-prefix'), WINEDEBUG='-all',
                           WINEDLLOVERRIDES='mscoree,mshtml=')
    output = subprocess.run(['wine', str(executable)] if wine else [str(executable)],
                            cwd=work, env=environment, capture_output=True, timeout=90)
    (work / 'stdout.log').write_bytes(output.stdout)
    (work / 'stderr.log').write_bytes(output.stderr)
    if output.returncode:
        print(output.stderr.decode(errors='replace'), flush=True)
    output.check_returncode()
    print(output.stdout.decode(errors='replace').strip())
    if wine:
        # Successful fixtures need logs and binaries, not a large persistent
        # Wine installation. Only this fresh test's own prefix is affected.
        try:
            subprocess.run(['wineserver', '-k'], env=environment, check=False, timeout=20)
            subprocess.run(['wineserver', '-w'], env=environment, check=True, timeout=20)
            shutil.rmtree(work / 'wine-prefix')
        except (OSError, subprocess.SubprocessError) as error:
            print('Native checks passed; temporary test prefix retained:', error)


if __name__ == '__main__':
    main()
