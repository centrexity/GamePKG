"""Asset-free maintainer qualification of the native Rust SSE setup bridge."""
import argparse
from pathlib import Path
import subprocess
import tempfile
from takedawn_native_mxcsr import adapt, SITES


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cc', default='cc')
    parser.add_argument('--target', default='x86_64-unknown-linux-gnu',
                        choices=['x86_64-unknown-linux-gnu', 'i686-unknown-linux-gnu'])
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    original = '\n'.join(f'RECOMP_UNIMPL("{op}", 0x{va}u); /* TODO: {op} */'
                         for va, op, _ in SITES)
    updated, seen = adapt(original)
    assert seen == ['001DD63F', '001DD652'] and 'RECOMP_UNIMPL' not in updated
    assert 'MEM32(esp + 0xC) = gamepkg_takedawn_stmxcsr();' in updated
    assert 'gamepkg_takedawn_ldmxcsr(MEM32(esp + 8));' in updated
    assert adapt('unrelated native code') == ('unrelated native code', [])
    for bad in [original + '\n' + original, original.splitlines()[0], original.splitlines()[1]]:
        try:
            adapt(bad)
        except RuntimeError:
            pass
        else:
            raise AssertionError('duplicate or incomplete setup was accepted')
    work = Path(tempfile.mkdtemp(prefix='gamepkg-native-mxcsr.'))
    print('Private evidence directory:', work, flush=True)
    rust = project / 'engines/takedawn/rust-port/native_mxcsr.rs'
    subprocess.run(['rustc', '--edition=2021', '--target', args.target, '--crate-type=staticlib',
                    '-Dwarnings', '-C', 'opt-level=3', '-C', 'panic=abort',
                    str(rust), '-o', str(work / 'libmxcsr.a')], check=True)
    flags = ['-m32', '-msse2'] if args.target.startswith('i686-') else []
    subprocess.run([args.cc, '-std=gnu11', '-O3', '-UNDEBUG', '-Wall', '-Wextra', '-Werror',
                    *flags, str(project / 'engines/takedawn/rust-port/tests/mxcsr_compare.c'),
                    str(work / 'libmxcsr.a'), '-lpthread', '-ldl', '-lm',
                    '-o', str(work / 'compare')], check=True)
    output = subprocess.check_output([str(work / 'compare')], text=True, timeout=30)
    (work / 'result.log').write_text(output)
    print(output, end='')


if __name__ == '__main__':
    main()
