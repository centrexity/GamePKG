"""Compare recovered original native C bodies with their prepared Rust output.

Maintainer only. Requires an earlier --recover-gameplay-paths conversion, never
reads or extracts game media, and does not change the prepared source tree.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['source', 'toolkit', 'prepared']:
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    prepared = args.prepared.resolve()
    manifest = json.loads((prepared / 'conversion.json').read_text())
    recovery = manifest['recovered_paths']
    spec = project / 'engines/takedawn/rust-port/recovered-paths.json'
    if recovery['specification_sha256'] != hashlib.sha256(spec.read_bytes()).hexdigest():
        raise RuntimeError('prepared recovery differs from the reviewed specification')
    rust = prepared / 'src/recomp_recovered.rs'
    if hashlib.sha256(rust.read_bytes()).hexdigest() != manifest['rust_sources'][rust.name]:
        raise RuntimeError('prepared recovered Rust differs from its conversion receipt')
    work = Path(tempfile.mkdtemp(prefix='gamepkg-recovered-native.'))
    print('Private evidence directory:', work, flush=True)
    gen = args.source.resolve() / 'src/game/recomp/gen'
    includes = ['-I' + str(gen), '-I' + str(args.toolkit.resolve() / 'templates/runtime')]
    subprocess.run(['rustc', '--edition=2021', '--crate-type=staticlib', '-Awarnings',
                    '-C', 'opt-level=3', '-C', 'panic=abort', '-C', 'overflow-checks=off',
                    str(rust), '-o', str(work / 'librecovered.a')], check=True)
    subprocess.run(['clang-18', '-std=gnu11', '-O3', '-fwrapv', '-fno-strict-aliasing',
                    '-Wno-parentheses-equality',
                    *includes, '-Dsub_000966F0=c_sub_000966F0',
                    '-Dsub_00249AB6=c_sub_00249AB6', '-c',
                    str(prepared / 'c/recomp_recovered.c'), '-o', str(work / 'reference.o')], check=True)
    subprocess.run(['clang-18', '-std=gnu11', '-O3', '-UNDEBUG', *includes,
                    str(project / 'engines/takedawn/rust-port/tests/recovered_compare.c'),
                    str(work / 'reference.o'), str(work / 'librecovered.a'),
                    '-lpthread', '-ldl', '-lm', '-o', str(work / 'compare')], check=True)
    result = subprocess.check_output([str(work / 'compare')], text=True, timeout=30)
    (work / 'result.log').write_text(result)
    print(result, end='')


if __name__ == '__main__':
    main()
