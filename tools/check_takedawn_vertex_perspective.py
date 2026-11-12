#!/usr/bin/env python3
"""Asset-free maintainer regression for near-view-plane Vulkan reconstruction."""
import argparse
import hashlib
import re
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--toolkit', type=Path, required=True)
    parser.add_argument('--cc', default='cc')
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    source = args.toolkit.resolve() / 'src/d3d/d3d8_vsh_glsl.c'
    adapter = project / 'engines/takedawn/media-bridge/adapt_vertex_perspective.cmake'
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    before = digest(source)
    with tempfile.TemporaryDirectory(prefix='gamepkg-vertex-perspective.') as folder:
        work = Path(folder)
        output = work / source.name

        def adapt(src, dst):
            return subprocess.run(['cmake', '-DINPUT=' + str(src), '-DOUTPUT=' + str(dst),
                                   '-P', str(adapter)], capture_output=True, text=True)

        result = adapt(source, output)
        assert result.returncode == 0, result.stdout + result.stderr
        assert digest(output) == 'd8e0be1fede0df1482d81c8f987c51fa5b621225562b2e0bc9efa637462ddd4d'
        unknown = work / 'unknown.c'
        unknown.write_bytes(source.read_bytes() + b'\n/* unknown preimage */\n')
        rejected = work / 'rejected.c'
        assert adapt(unknown, rejected).returncode != 0 and not rejected.exists()
        assert adapt(output, rejected).returncode != 0 and not rejected.exists()
        assert adapt(source, source).returncode != 0
        alias = work / 'alias.c'
        alias.symlink_to(source)
        assert adapt(source, alias).returncode != 0
        assert digest(source) == before

        # Run the real emitter, including its unchanged non-screen-space branch.
        emitter = work / 'emit.c'
        emitter.write_text('''#include "d3d8_glsl.h"
#include <stdio.h>
#include <stdlib.h>
int main(int argc,char **argv){NV2AVshProgram p={0};char text[65536];
if(d3d8_vsh_generate_glsl(&p,argc>1?D3D8_GLSL_SCREEN_SPACE:0,text,sizeof text)<=0)return 1;
fputs(text,stdout);return 0;}
''')
        executable = work / 'emit'
        subprocess.run([args.cc, '-std=c11', '-O2', '-I' + str(source.parent),
                        str(output), str(emitter), '-o', str(executable)], check=True)
        shader = subprocess.check_output([str(executable), 'screen'], text=True)
        plain = subprocess.check_output([str(executable)], text=True)
        original_emitter = work / 'emit-original'
        subprocess.run([args.cc, '-std=c11', '-O2', '-I' + str(source.parent),
                        str(source), str(emitter), '-o', str(original_emitter)], check=True)
        assert subprocess.check_output([str(original_emitter)], text=True) == plain
        anchor = '    float pw = '
        assert shader.count(anchor) == 1 and anchor not in plain
        expression = shader.split(anchor, 1)[1].split(';', 1)[0]
        # Evaluate the exact emitted expression with binary32 C equivalents.
        # Independent oracle: normal W is unchanged; saturated RCC reconstructs
        # a signed inverse ceiling, including either zero sign and subnormals.
        expression = expression.replace('abs(', 'fabsf(').replace('oPos.w', 'w')
        # GLSL unsuffixed floating constants are binary32; C defaults to double.
        expression = re.sub(r'\b\d+\.\d*(?:e[+-]?\d+)?', lambda m: m[0] + 'f', expression)
        fixture = work / 'numeric.c'
        fixture.write_text(r'''
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <math.h>
#include <float.h>
#include <assert.h>
static uint32_t floatBitsToUint(float f){uint32_t u;memcpy(&u,&f,4);return u;}
static float uintBitsToFloat(uint32_t u){float f;memcpy(&f,&u,4);return f;}
static float reconstruct(float w){return EXPRESSION;}
static void check(uint32_t bits){
 float w=uintBitsToFloat(bits), got=reconstruct(w);
 const float cap=1.84467e19f, minimum=1.0f/cap;
 if(fabsf(w)>=minimum || isnan(w))assert(floatBitsToUint(got)==bits);
 else {
  assert(signbit(got)==signbit(w));
  assert(isfinite(got) && got!=0.0f);
  assert(fabsf(fabsf(got)*cap-1.0f)<2*FLT_EPSILON);
 }
}
int main(void){
 assert(sizeof(float)==4 && FLT_RADIX==2 && FLT_MANT_DIG==24);
 const float t=1.0f/1.84467e19f;
 uint32_t edge=floatBitsToUint(t);
 for(uint32_t i=0;i<128;i++)for(uint32_t sign=0;sign<2;sign++){
  check((edge-64+i)|(sign<<31));check(i|(sign<<31));
 }
 uint32_t u=0x12345678;
 for(unsigned i=0;i<1000000;i++){u=u*1664525u+1013904223u;check(u);}
 check(0);check(0x80000000);check(0x7f800000);check(0xff800000);
 /* Recorded screen-space magnitude: reconstruct the pre-division scale
    instead of carrying its enormous value into homogeneous clipping. */
 float original=2.48252238e22f;
 float fixed=original*reconstruct(0.0f);
 assert(isfinite(fixed) && fixed>1000 && fixed<2000);
 puts("PASS: real GLSL emitter, signed zero/subnormal/threshold cases, 1000000 binary32 values");
}
'''.replace('EXPRESSION', expression))
        numeric = work / 'numeric'
        subprocess.run([args.cc, '-std=c11', '-O2', '-Wall', '-Wextra', '-Werror',
                        str(fixture), '-lm', '-o', str(numeric)], check=True)
        subprocess.run([str(numeric)], check=True)
    print('PASS: unknown/repeated sources and source overwrite aliases rejected')


if __name__ == '__main__':
    main()
