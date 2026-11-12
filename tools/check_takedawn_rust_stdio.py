"""Maintainer-only native C/Rust stream ABI test; no game media is needed.

Wine is only a maintainer test runner. Neither Wine nor the conversion tools
are player dependencies. Each Windows test uses its own disposable prefix.
"""
import argparse
import hashlib
import json
import os
import shutil
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--c2rust', type=Path)
    parser.add_argument('--normalizer', type=Path)
    parser.add_argument('--refresh-fixture', action='store_true')
    parser.add_argument('--target', default=('x86_64-pc-windows-msvc' if os.name == 'nt'
                        else 'x86_64-unknown-linux-gnu'), choices=[
        'x86_64-unknown-linux-gnu', 'i686-unknown-linux-gnu',
        'x86_64-pc-windows-gnu', 'i686-pc-windows-gnu',
        'x86_64-pc-windows-msvc', 'i686-pc-windows-msvc'])
    parser.add_argument('--cc')
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    fixture = project / 'engines/takedawn/rust-port/tests'
    work = Path(tempfile.mkdtemp(prefix='gamepkg-native-stdio.'))
    print('Private evidence directory:', work, flush=True)
    source = fixture / 'stdio_fixture.c'
    prepared = fixture / 'stdio-prepared'
    rust = work / 'stdio.rs'
    digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
    identities = {'c_source_sha256': digest(source), 'normalizer_source_sha256':
                  digest(project / 'engines/takedawn/rust-port/src/main.rs')}
    if bool(args.c2rust) != bool(args.normalizer):
        parser.error('pass both --c2rust and --normalizer, or use the checked prepared fixture')
    if args.refresh_fixture and not args.c2rust:
        parser.error('--refresh-fixture requires the conversion tools')
    if args.c2rust:
        database = work / 'compile_commands.json'
        database.write_text(json.dumps([{'directory': str(work), 'file': str(source),
            'arguments': ['clang-18', '-std=gnu11', '-c', str(source)]}]))
        with (work / 'transpile.log').open('w') as log:
            subprocess.run([str(args.c2rust.resolve()), 'transpile', '--fail-on-error',
                            '--disable-rustfmt', '--emit-modules', '--output-dir', str(work / 'raw'),
                            str(database)], stdout=log, stderr=subprocess.STDOUT, check=True, timeout=60)
        subprocess.run([str(args.normalizer.resolve()), str(work / 'raw/src/stdio_fixture.rs'),
                        str(rust), str(work / 'normalized.json')], check=True)
        report = json.loads((work / 'normalized.json').read_text())
        assert report['stdio_streams'] == ['stderr', 'stdin', 'stdout']
        assert report['stdio_functions'] == ['fflush', 'fprintf']
        assert report['fixed_width_constants'] == {'INT64_MIN': 1}
        if args.refresh_fixture:
            prepared.mkdir(exist_ok=True)
            (prepared / 'stdio.rs').write_bytes(rust.read_bytes())
            identities.update(rust_sha256=digest(rust), c2rust_sha256=digest(args.c2rust),
                              normalizer_binary_sha256=digest(args.normalizer))
            (prepared / 'provenance.json').write_text(json.dumps(identities, indent=2) + '\n')
    else:
        provenance = json.loads((prepared / 'provenance.json').read_text())
        assert all(provenance[key] == value for key, value in identities.items()), 'refresh stale prepared fixture'
        assert provenance['rust_sha256'] == digest(prepared / 'stdio.rs'), 'prepared fixture hash differs'
        rust.write_bytes((prepared / 'stdio.rs').read_bytes())
    subprocess.run(['rustc', '--edition=2021', '--crate-type=lib', '--emit=obj',
                    '--target', args.target, '-Awarnings', '-C', 'opt-level=3',
                    '-C', 'panic=abort', str(rust), '-o', str(work / 'stdio.o')], check=True)
    windows = 'windows' in args.target
    msvc = args.target.endswith('-msvc')
    cc = args.cc or ('cl' if msvc else args.target.replace('pc-windows-gnu', 'w64-mingw32') + '-gcc'
                     if windows else 'cc')
    flags = ['-m32'] if args.target.startswith('i686-') and not windows else []
    rename = [f'-D{name}=ref_{name}' for name in ['port_stdin', 'port_stdout', 'port_stderr', 'port_log', 'port_int64_min']]
    executable = work / ('compare.exe' if windows else 'compare')
    if msvc:
        common = [cc, '/nologo', '/std:c11', '/O2', '/UNDEBUG', '/W4', '/WX']
        subprocess.run([*common, *[flag.replace('-D', '/D', 1) for flag in rename],
                        '/c', str(source), '/Fo' + str(work / 'reference.obj')], cwd=work, check=True)
        subprocess.run([*common, str(fixture / 'stdio_compare.c'), str(work / 'reference.obj'),
                        str(work / 'stdio.o'), '/Fe' + str(executable)], cwd=work, check=True)
    else:
        subprocess.run([cc, '-std=c11', '-O3', '-UNDEBUG', *flags, *rename,
                        '-c', str(source), '-o', str(work / 'reference.o')], check=True)
        subprocess.run([cc, '-std=c11', '-O3', '-UNDEBUG', '-Wall', '-Wextra', '-Werror', *flags,
                        str(fixture / 'stdio_compare.c'), str(work / 'reference.o'),
                        str(work / 'stdio.o'), '-o', str(executable)], check=True)
    environment = dict(os.environ)
    wine = windows and os.name != 'nt'
    if wine:
        environment.update(WINEPREFIX=str(work / 'wine-prefix'), WINEDEBUG='-all',
                           WINEDLLOVERRIDES='mscoree,mshtml=')
    command = ['wine', str(executable)] if wine else [str(executable)]
    output = subprocess.run(command, env=environment, capture_output=True, timeout=90)
    (work / 'stdout.log').write_bytes(output.stdout)
    (work / 'stderr.log').write_bytes(output.stderr)
    output.check_returncode()
    lines = [line for line in output.stderr.decode(errors='replace').splitlines()
             if line.startswith('native-stream:')]
    assert len(lines) == 12 and all(lines[i] == lines[i + 1] for i in range(0, 12, 2)), lines
    assert '18446744073709551615' in lines[-1]
    print(output.stdout.decode(errors='replace').strip())
    print('PASS:', args.target, 'native stream and variadic ABI; every C/Rust output pair matches')
    if wine:
        try:
            subprocess.run(['wineserver', '-k'], env=environment, check=False, timeout=20)
            subprocess.run(['wineserver', '-w'], env=environment, check=True, timeout=20)
            shutil.rmtree(work / 'wine-prefix')
        except (OSError, subprocess.SubprocessError) as error:
            print('Native checks passed; temporary test prefix retained:', error)


if __name__ == '__main__':
    main()
