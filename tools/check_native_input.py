"""Maintainer check of the shared native input service and its actual C ABI."""
import argparse
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / 'engines/native-input/Cargo.toml'


def run(command, log, **kwargs):
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, **kwargs)
    log.write_text(result.stdout + result.stderr)
    if result.returncode:
        raise RuntimeError(f'{command[0]} failed; see {log}')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--cc')
    args = parser.parse_args()
    targets = {
        'x86_64-unknown-linux-gnu': ('cc', ['-m64'], False),
        'i686-unknown-linux-gnu': ('cc', ['-m32', '-isystem', '/usr/include/x86_64-linux-gnu'], False),
        'x86_64-pc-windows-msvc': ('cl', [], True),
        'i686-pc-windows-msvc': ('cl', [], True),
    }
    if args.target not in targets:
        parser.error('unsupported native check target')
    compiler, flags, windows = targets[args.target]
    if windows != (sys.platform == 'win32'):
        parser.error('execute on the target operating system; cross-builds alone do not qualify')
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    if args.cc and not windows:
        env['CARGO_TARGET_' + args.target.upper().replace('-', '_') + '_LINKER'] = args.cc
    run(['cargo', 'test', '--locked', '--manifest-path', str(MANIFEST), '--target', args.target], out/'rust-tests.log', timeout=180, env=env)
    result = run(['cargo', 'rustc', '--locked', '--manifest-path', str(MANIFEST), '--target', args.target,
                  '--release', '--lib', '--message-format=json', '--', '--print', 'native-static-libs'], out/'rust-build.log', timeout=180, env=env)
    artifacts = [json.loads(line) for line in result.stdout.splitlines() if line.startswith('{')]
    libraries = [Path(name) for item in artifacts if item.get('reason') == 'compiler-artifact'
                 and item.get('target', {}).get('name') == 'gamepkg_native_input'
                 for name in item.get('filenames', []) if name.endswith(('.a', '.lib'))]
    assert len(libraries) == 1, libraries
    library = libraries[0]
    # The Rust compiler supplies the exact system libraries required by this
    # static library; never add a guessed list of Windows SDK dependencies.
    text = result.stderr
    for item in artifacts:
        text += item.get('message', {}).get('rendered', '')
    match = re.search(r'native-static-libs: (.*)', text)
    assert match, 'Rust did not report native link requirements'
    link = shlex.split(match[1])
    binary = out/('input-abi.exe' if windows else 'input-abi')
    source = ROOT/'engines/native-input/tests/abi.c'
    if windows:
        command = [args.cc or compiler, '/nologo', '/std:c11', '/O2', '/W4', '/WX',
                   '/MT' if any('libcmt' in item.casefold() for item in link) else '/MD',
                   '/I'+str(ROOT/'include'), str(source), '/Fo'+str(out/'abi.obj'),
                   '/link', str(library), *link, '/OUT:'+str(binary)]
    else:
        command = [args.cc or compiler, *flags, '-std=c11', '-O2', '-Wall', '-Wextra', '-Werror',
                   '-I'+str(ROOT/'include'), str(source), str(library), *link, '-o', str(binary)]
    run(command, out/'c-build.log', timeout=120)
    result = run([str(binary)], out/'c-run.log', timeout=60)
    assert 'PASS actual C/Rust native input ABI' in result.stdout
    (out/'result.json').write_text(json.dumps({
        'target': args.target, 'result': 'pass',
        'scope': 'Native Rust state/FFI tests and actual C/Rust ABI execution. No OS input backend, physical controller delivery, game integration or frame-time claim.',
        'library': str(library), 'binary': str(binary), 'c_compile': command,
    }, indent=2)+'\n')
    print(result.stdout, end='')


if __name__ == '__main__':
    main()
