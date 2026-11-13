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
    parser.add_argument('--pinned-xinput', type=Path)
    parser.add_argument('--pinned-input-core', type=Path)
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
    checks = [('input-abi', ROOT/'engines/native-input/tests/abi.c', [ROOT/'include'],
               'PASS actual C/Rust native input ABI')]
    if windows:
        checks.append(('native-xinput', ROOT/'engines/takedawn/media-bridge/tests/native_xinput.c',
                       [ROOT/'include', ROOT/'engines/takedawn/media-bridge'],
                       'PASS native Windows XInput adapter'))
    compiled = []
    for name, source, includes, marker in checks:
        binary = out/(name + ('.exe' if windows else ''))
        if windows:
            command = [args.cc or compiler, '/nologo', '/std:c11', '/O2', '/W4', '/WX',
                       '/MT' if any('libcmt' in item.casefold() for item in link) else '/MD',
                       *['/I'+str(path) for path in includes], str(source), '/Fo'+str(out/(name+'.obj')),
                       '/link', str(library), *link, '/OUT:'+str(binary)]
        else:
            command = [args.cc or compiler, *flags, '-std=c11', '-O2', '-Wall', '-Wextra', '-Werror',
                       *['-I'+str(path) for path in includes], str(source), str(library), *link, '-o', str(binary)]
        run(command, out/(name+'-build.log'), timeout=120)
        result = run([str(binary)], out/(name+'-run.log'), timeout=60)
        assert marker in result.stdout
        compiled.append({'fixture': name, 'binary': str(binary), 'compile': command})
        print(result.stdout, end='')
    if args.pinned_xinput:
        original = args.pinned_xinput.resolve()
        adapter = ROOT/'engines/takedawn/media-bridge/adapt_native_input.cmake'
        staged = out/'xinput_device.c'
        def adapt(source, destination, label, expected=None):
            command = ['cmake', '-DINPUT='+str(source), '-DOUTPUT='+str(destination), '-P', str(adapter)]
            result = subprocess.run(command, capture_output=True, text=True, timeout=30)
            (out/(label+'.log')).write_text(result.stdout+result.stderr)
            if expected:
                assert result.returncode != 0 and expected in result.stderr, result.stderr
            else:
                assert result.returncode == 0, result.stderr
        adapt(original, staged, 'source-adapt')
        marker = '#else /* !_WIN32 */'
        assert original.read_text().split(marker, 1)[1] == staged.read_text().split(marker, 1)[1], 'SDL backend changed'
        modified = out/'modified-source.c'
        modified.write_bytes(original.read_bytes()+b'\n/* changed pinned source */\n')
        rejected = out/'rejected-output.c'; rejected.unlink(missing_ok=True)
        adapt(modified, rejected, 'source-reject-modified', 'Unreviewed native controller source')
        assert not rejected.exists()
        copy = out/'pinned-copy.c'; copy.write_bytes(original.read_bytes())
        adapt(copy, copy, 'source-reject-inplace', 'Never overwrite pinned input source')
        assert copy.read_bytes() == original.read_bytes()
        print('PASS pinned adapter staging, unchanged SDL backend, modified-source rejection and in-place protection')
    if args.pinned_input_core:
        assert not windows, 'Probe predicate reference is a native Linux fixture'
        adapter = ROOT/'engines/takedawn/media-bridge/adapt_native_input.cmake'
        core = out/'input_core.c'
        run(['cmake', '-DINPUT='+str(args.pinned_input_core.resolve()), '-DOUTPUT='+str(core),
             '-DKIND=core', '-P', str(adapter)], out/'core-adapt.log', timeout=30)
        text = core.read_text(); start = text.index('static int due(DWORD port)')
        function = text[start:text.index('}', start)+1]
        start = text.index('BOOL xbox_InputIsConnected(DWORD dwPort)')
        connection = text[start:text.index('}', start)+1]
        fixture = out/'probe.c'
        fixture.write_text("""#include <assert.h>
#include <stdint.h>
#include <stdio.h>
typedef uint32_t DWORD;
typedef int BOOL;
#define FALSE 0
typedef struct {int unused;} XBOX_INPUT_STATE;
#define XBOX_MAX_CONTROLLERS 4
#define XBOX_INPUT_PROBE_MS 1000
static uint64_t now;
static uint64_t clock_ms(void){return now;}
struct backend {uint64_t (*now_ms)(void);};
static const struct backend xbox_input_host_backend={clock_ms},other={clock_ms};
static const struct backend *g_be;
static int g_probed[4],g_connected[4];static uint64_t g_last_probe[4];
static unsigned probe_calls;
static void xbox_InputInit(void){}
static DWORD probe(DWORD port,XBOX_INPUT_STATE *out){(void)out;++probe_calls;g_connected[port]=1;g_last_probe[port]=now;return 0;}
#include "due.inc"
int main(void){
 for(DWORD port=0;port<4;port++){
  g_probed[port]=1;g_connected[port]=0;g_last_probe[port]=now=50;
  g_be=&xbox_input_host_backend;assert(due(port));
  g_be=&other;assert(!due(port));now=1050;assert(due(port));
  now=50;g_connected[port]=1;assert(due(port));g_connected[port]=0;
  g_probed[port]=0;assert(due(port));
 }
 g_be=&xbox_input_host_backend;g_connected[0]=0;g_last_probe[0]=now=50;probe_calls=0;
 assert(xbox_InputIsConnected(0)&&probe_calls==1);
 g_be=&other;g_connected[0]=0;g_last_probe[0]=now=50;probe_calls=0;
 assert(!xbox_InputIsConnected(0)&&!probe_calls);now=1050;assert(xbox_InputIsConnected(0)&&probe_calls==1);
 assert(!xbox_InputIsConnected(4));
 puts("PASS actual logical/connection probe policy: host snapshots stay fresh; other backends retain backoff");return 0;
}
""")
        for mutation in ['none', 'logical', 'connection']:
            due_piece, connection_piece = function, connection
            before = 'g_be == &xbox_input_host_backend || '
            if mutation == 'logical':
                assert due_piece.count(before) == 1
                due_piece = due_piece.replace(before, '')
            if mutation == 'connection':
                assert connection_piece.count(before) == 1
                connection_piece = connection_piece.replace(before, '')
            (out/'due.inc').write_text(due_piece+'\n'+connection_piece)
            name = 'probe-'+mutation
            binary = out/name
            run([args.cc or compiler, *flags, '-std=c11', '-O2', '-Wall', '-Wextra', '-Werror',
                 '-I'+str(out), str(fixture), '-o', str(binary)], out/(name+'-build.log'), timeout=60)
            result = subprocess.run([str(binary)], capture_output=True, text=True, timeout=30)
            (out/(name+'-run.log')).write_text(result.stdout+result.stderr)
            if mutation != 'none':
                assertion = 'due(port)' if mutation == 'logical' else 'xbox_InputIsConnected(0)'
                assert result.returncode != 0 and 'Assertion' in result.stderr and assertion in result.stderr
            else:
                assert result.returncode == 0, result.stderr
                print(result.stdout, end='')
        (out/'due.inc').write_text(function+'\n'+connection)
    (out/'result.json').write_text(json.dumps({
        'target': args.target, 'result': 'pass',
        'scope': 'Native Rust state/FFI and actual C ABI execution; Windows adapter uses fake OS device callbacks. No physical controller delivery, full game integration or frame-time claim.',
        'library': str(library), 'compiled': compiled,
        'pinned_source_checks': bool(args.pinned_xinput),
        'host_probe_policy_and_negative_control': bool(args.pinned_input_core),
    }, indent=2)+'\n')



if __name__ == '__main__':
    main()
