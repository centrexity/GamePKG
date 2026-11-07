"""Maintainer regression for GPU report ranking and pinned-source adaptation.

No game assets; Python is a maintainer/CI requirement only, never a player one.
GCC/Clang-style compilers are supported, including MinGW for Windows fixtures.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

FIXTURE = r"""
static int check(uint32_t method,uint32_t count,uint32_t param) {
 for(int i=0;i<s_unhandled_count;i++)if(s_unhandled[i].method==method){
  if(s_unhandled[i].count==count && s_unhandled[i].last_param==param)return 1;
  fprintf(stderr,"WRONG method%04x count%u expected%u last%08x expected%08x\n",method,s_unhandled[i].count,count,s_unhandled[i].last_param,param);return 0;
 }return 0;
}
int main(void) {
 note_unhandled(0x100,0xa001);note_unhandled(0x104,0xb001);note_unhandled(0x104,0xb002);
 note_unhandled(0x108,0xc001);note_unhandled(0x108,0xc002);note_unhandled(0x108,0xc003);
 report(0);note_unhandled(0x100,0xa002);
 if(!check(0x100,2,0xa002)||!check(0x104,2,0xb002)||!check(0x108,3,0xc003))return 17;
 for(unsigned n=0;n<12;n++)note_unhandled(0x300+4*n,0xf000+n);
 for(unsigned n=0;n<32;n++){
  report(n&1);note_unhandled(0x100,0xa003+n);note_unhandled(0x108,0xc004+n);
  note_unhandled(0x2004,0xd001+n);note_unhandled(0x101,0xe001+n);
  if(!check(0x100,3+n,0xa003+n)||!check(0x108,4+n,0xc004+n)||!check(0x2004,1+n,0xd001+n)||!check(0x101,1+n,0xe001+n))return 18;
 }
 for(unsigned n=0;n<12;n++)if(!check(0x300+4*n,1,0xf000+n))return 19;
 return 0;
}
"""


def parts(source):
    start = source.index('#define PB_EXEC_MAX_UNHANDLED')
    types = source[start:source.index('/* Every texture-stage register', start)]
    start = source.index('static uint16_t s_unhandled_slot')
    counter = source[start:source.index('/* Read attribute', start)]
    body_start = source.index('        int shown = recomp_env(RENV_PB_UNHANDLED_ALL)')
    start = source.index('for (i = 0;', body_start)
    end = source.index('\n    }    fflush(stderr);', start)
    return types, counter, source[start:end], source[body_start:end]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True, help='Pinned toolkit tree')
    parser.add_argument('--cc', default=os.environ.get('CC', 'cc'))
    parser.add_argument('--compiler-kind', choices=['gnu', 'msvc'], default='gnu')
    parser.add_argument('--sanitize', action='store_true')
    parser.add_argument('--compile-only', action='store_true', help='Run the emitted fixtures on their target separately')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if args.sanitize and args.compiler_kind == 'msvc':
        parser.error('Use the native GCC/Clang sanitizer fixture')
    if args.compile_only and args.sanitize:
        parser.error('Cross fixtures must be executed with a compatible sanitizer runtime separately')
    project = Path(__file__).resolve().parents[1]
    adapter = project / 'engines/takedawn/media-bridge/adapt_gpu_report.cmake'
    upstream = args.source.resolve() / 'src/kernel/nv2a_pb_exec.c'
    before = upstream.read_bytes()
    expected_hash = 'efc81258a81c3a2add67b904e4815b53e2283243e000c768b97c8319e2c1d220'
    assert hashlib.sha256(before).hexdigest() == expected_hash
    work = args.output.resolve() if args.output else Path(tempfile.mkdtemp(prefix='gamepkg-gpu-report-'))
    work.mkdir(parents=True, exist_ok=True)
    original = work / 'original-source.c'
    assert not original.exists(), 'Use an empty output directory'
    original.write_bytes(before)
    staged = work / 'adapted-source.c'

    def stage(src, dest, succeeds):
        result = subprocess.run(['cmake', '-DINPUT=' + str(src), '-DOUTPUT=' + str(dest),
                                 '-P', str(adapter)], capture_output=True, text=True)
        assert (result.returncode == 0) == succeeds, result.stdout + result.stderr

    stage(original, staged, True)
    old = before.decode()
    new = staged.read_text()
    _, _, _, old_report = parts(old)
    _, _, _, new_report = parts(new)
    assert 'memcpy(ranked, s_unhandled,' in new_report
    assert new.count(new_report) == old.count(old_report) == 1
    assert new.replace(new_report, old_report) == old, 'Non-reporting GPU code changed'
    first_mtime = staged.stat().st_mtime_ns
    stage(original, staged, True)
    assert staged.read_text() == new and staged.stat().st_mtime_ns == first_mtime
    stage(original, original, False)
    altered = work / 'altered-source.c'
    altered.write_bytes(before + b'\n/* unreviewed revision */\n')
    rejected = work / 'rejected.c'
    stage(altered, rejected, False)
    assert not rejected.exists()
    stage(staged, rejected, False)
    assert not rejected.exists()
    assert original.read_bytes() == upstream.read_bytes() == before
    results = []
    for label, source, expected in [('original', old, 17), ('adapted', new, 0)]:
        types, counter, _, report = parts(source)
        # Use the entire production report block, including the adapter's
        # snapshot declaration/copy; substitute only the environment choice.
        report = report.replace('recomp_env(RENV_PB_UNHANDLED_ALL)', 'all')
        harness = ('#include <stdint.h>\n#include <stdio.h>\n#include <string.h>\n'
                   'struct {uint32_t unhandled_total;}s_gpu;\n' + types + counter +
                   '\nstatic void report(int all){int i,j;\n' + report + '\n}\n' + FIXTURE)
        path = work / (label + '.c')
        path.write_text(harness)
        exe = work / (label + '.exe')
        flags = ['-std=c11', '-O2', '-Wall', '-Wextra', '-Werror']
        if args.sanitize:
            flags += ['-fsanitize=address,undefined']
        if args.compiler_kind == 'msvc':
            command = [args.cc, '/nologo', '/std:c11', '/O2', '/W4', '/WX',
                       str(path), '/Fe:' + str(exe), '/Fo:' + str(work / (label + '.obj'))]
        else:
            command = [args.cc, *flags, str(path), '-o', str(exe)]
        subprocess.run(command, cwd=work, check=True)
        result = {'variant': label, 'executable': str(exe), 'expected_exit': expected}
        if args.compile_only:
            result['execution'] = 'pending-target-run'
        else:
            ran = subprocess.run([str(exe)], capture_output=True, text=True)
            (work / (label + '.stdout.log')).write_text(ran.stdout)
            (work / (label + '.stderr.log')).write_text(ran.stderr)
            assert ran.returncode == expected, (label, ran.returncode, ran.stderr[-2000:])
            result.update(execution='verified', exit=ran.returncode)
        results.append(result)
    record = {'result': 'compile-only-target-execution-pending' if args.compile_only else 'pass',
              'source_sha256': expected_hash, 'original_source_unchanged': upstream.read_bytes() == before,
              'only_reporting_changed': True, 'unknown_and_in_place_sources_rejected': True,
              'sanitizers': args.sanitize, 'cases': results,
              'scope': 'Production method counters and report ranking; repeated top-ten/all reports, aligned/unaligned/out-of-range methods. No rendering-equivalence claim.'}
    (work / 'result.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(record, indent=2))


if __name__ == '__main__':
    main()
