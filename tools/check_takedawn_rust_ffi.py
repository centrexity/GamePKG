"""Maintainer-only native C/Rust atomic and opaque-pointer comparison."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--c2rust', type=Path, required=True)
parser.add_argument('--normalizer', type=Path, required=True)
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
fixtures = project / 'engines/takedawn/rust-port/tests'
work = Path(tempfile.mkdtemp(prefix='gamepkg-takedawn-rust-ffi.'))
print(f'Private evidence directory: {work}', flush=True)
c = fixtures / 'ffi_fixture.c'
database = work / 'compile_commands.json'
database.write_text(json.dumps([{'directory': str(work), 'file': str(c), 'arguments': [
    '/usr/bin/clang-18', '-std=gnu11', '-c', str(c)]}]))
with (work / 'transpile.log').open('w') as log:
    subprocess.run([str(args.c2rust.resolve()), 'transpile', '--fail-on-error',
                    '--disable-rustfmt', '--emit-modules', '--output-dir', str(work / 'raw'),
                    str(database)], stdout=log, stderr=subprocess.STDOUT, check=True, timeout=60)
rust = work / 'fixture.rs'
subprocess.run([str(args.normalizer.resolve()), str(work / 'raw/src/ffi_fixture.rs'),
                str(rust), str(work / 'normalized.json')], check=True)
with (work / 'compile.log').open('w') as log:
    subprocess.run(['rustc', '--edition=2021', '--crate-type=staticlib', '-Awarnings',
                    '-C', 'opt-level=2', '-C', 'panic=abort', str(rust),
                    '-o', str(work / 'libfixture.a')],
                   stdout=log, stderr=subprocess.STDOUT, check=True, timeout=60)
subprocess.run(['clang-18', '-std=gnu11', '-O2', '-Dport_add=ref_port_add',
                '-Dport_cas=ref_port_cas', '-Dport_cas_bool=ref_port_cas_bool',
                '-Dport_opaque=ref_port_opaque', '-c', str(c),
                '-o', str(work / 'fixture-c.o')], check=True)
subprocess.run(['clang-18', '-std=gnu11', '-O2', str(fixtures / 'ffi_compare.c'),
                str(work / 'fixture-c.o'), str(work / 'libfixture.a'), '-ldl', '-lpthread', '-lm',
                '-o', str(work / 'compare')], check=True)
result = subprocess.check_output([str(work / 'compare')], text=True, timeout=30)
(work / 'result.log').write_text(result)
print(result, end='')
