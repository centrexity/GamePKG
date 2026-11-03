"""Maintainer-only C/Rust control-flow comparison. No game or disc is needed.

Exercises large typed graphs, repeated C local names, initialization effects,
host callbacks, live TLS and both explicit and implicit function returns.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--c2rust', type=Path, required=True)
parser.add_argument('--normalizer', type=Path, required=True)
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
work = Path(tempfile.mkdtemp(prefix='gamepkg-takedawn-rust-cfg.'))
print(f'Private evidence directory: {work}', flush=True)
source = '''#include <stdint.h>
extern __thread uint32_t shared;
extern void native_touch(void);
uint32_t cfg_case(uint32_t seed,uint32_t steps,uint32_t *out) {
uint32_t state=seed, tick=0;shared=seed;goto L0;
'''
for i in range(300):
    source += f'L{i}: {{ uint32_t scratch=state ^ {i*0x12345+17}u; if(tick++==steps)goto done; state=scratch*1664525u+1013904223u; shared+=state; '
    if i % 7 == 0:
        source += 'native_touch(); '
    source += f'if(state&1u)goto L{(i*17+13)%300};goto L{(i+1)%300}; }}\n'
source += 'done: out[0]=state;out[1]=tick;out[2]=shared;return state^shared;}\n'
source += 'void cfg_void(uint32_t *out) {uint32_t n=0;goto V0;\n'
for i in range(300):
    source += f'V{i}: *out+=n;if(++n==4)goto finish;goto V{(i+1)%300};\n'
source += 'finish: *out+=0;}\n'
c = work / 'fixture.c'
c.write_text(source)
database = work / 'compile_commands.json'
database.write_text(json.dumps([{'directory': str(work), 'file': str(c), 'arguments': [
    '/usr/bin/clang-18', '-std=gnu11', '-c', str(c)]}]))
environment = dict(os.environ, GAMEPKG_C2RUST_FLAT_CFG='1')
with (work / 'transpile.log').open('w') as log:
    subprocess.run([str(args.c2rust.resolve()), 'transpile', '--fail-on-error',
                    '--no-incremental-relooper', '--no-simplify-structures',
                    '--disable-rustfmt', '--emit-modules', '--output-dir', str(work / 'raw'),
                    str(database)], env=environment, stdout=log, stderr=subprocess.STDOUT,
                   check=True, timeout=60)
rust = work / 'fixture.rs'
subprocess.run([str(args.normalizer.resolve()), str(work / 'raw/src/fixture.rs'), str(rust),
                str(work / 'normalized.json')], check=True)
with (work / 'compile.log').open('w') as log:
    subprocess.run(['rustc', '--edition=2021', '--crate-type=staticlib', '-Awarnings',
                    '-C', 'opt-level=2', '-C', 'panic=abort', '-C', 'overflow-checks=off',
                    str(rust), '-o', str(work / 'libfixture.a')],
                   stdout=log, stderr=subprocess.STDOUT, check=True, timeout=60)
subprocess.run(['clang-18', '-std=gnu11', '-O2', '-fwrapv', '-Dcfg_case=c_cfg_case',
                '-Dcfg_void=c_cfg_void', '-c', str(c), '-o', str(work / 'fixture-c.o')], check=True)
subprocess.run(['clang-18', '-std=gnu11', '-O2',
                str(project / 'engines/takedawn/rust-port/tests/cfg_compare.c'),
                str(work / 'fixture-c.o'), str(work / 'libfixture.a'), '-ldl', '-lpthread', '-lm',
                '-o', str(work / 'compare')], check=True)
result = subprocess.check_output([str(work / 'compare')], text=True, timeout=30)
(work / 'result.log').write_text(result)
print(result, end='')
