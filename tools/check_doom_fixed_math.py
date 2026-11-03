"""Qualify the actual DOOM fixed-point implementation against mathematical references."""
from pathlib import Path
import argparse,json,os,subprocess,tempfile
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--upstream',action='store_true')
parser.add_argument('--32-bit',dest='bits32',action='store_true')
parser.add_argument('--cc',default='cc')
parser.add_argument('--runtime-libraries',type=Path)
args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
source=(project/'engines/doom/upstream/m_fixed.c').read_text()
if not args.upstream:
    for patch in json.loads((project/'engines/doom/adapters/fixed-patches.json').read_text()):
        assert source.count(patch['before'])==patch['count']==1
        source=source.replace(patch['before'],patch['after'])
with tempfile.TemporaryDirectory(prefix='gamepkg-doom-fixed-') as temporary:
    root=Path(temporary);(root/'fixed.c').write_text(source)
    body='''#include <stdint.h>
#include <limits.h>
#include <math.h>
#include <assert.h>
#include <stdio.h>
#include "fixed.c"
static void check(int32_t a,int32_t b){
 long double product=floorl((long double)a*(long double)b/65536.0L);
 long double residue=fmodl(product,4294967296.0L);
 if(residue<0)residue+=4294967296.0L;
 if(residue>INT32_MAX)residue-=4294967296.0L;
 assert(FixedMul(a,b)==(int32_t)residue);
 long double ma=fabsl((long double)a),mb=fabsl((long double)b);
 int32_t expected;
 if(floorl(ma/16384.0L)>=mb)expected=(a<0)!=(b<0)?INT32_MIN:INT32_MAX;
 else expected=(int32_t)truncl((long double)a*65536.0L/(long double)b);
 assert(FixedDiv(a,b)==expected);
}
int main(void){
 const int32_t edges[]={INT32_MIN,INT32_MIN+1,-1073741824,-65537,-65536,-65535,-16385,-16384,-16383,-2,-1,0,1,2,16383,16384,16385,65535,65536,65537,1073741824,INT32_MAX-1,INT32_MAX};
 unsigned count=0;
 for(unsigned i=0;i<sizeof(edges)/sizeof(*edges);i++)for(unsigned j=0;j<sizeof(edges)/sizeof(*edges);j++){check(edges[i],edges[j]);count++;}
 for(int a=-32768;a<=32767;a++)for(unsigned j=0;j<sizeof(edges)/sizeof(*edges);j++){check(a,edges[j]);count++;}
 uint32_t state=0x12345678u;
 for(unsigned i=0;i<500000;i++){
  state^=state<<13;state^=state>>17;state^=state<<5;int32_t a=(int32_t)state;
  state^=state<<13;state^=state>>17;state^=state<<5;check(a,(int32_t)state);count++;
  int32_t threshold=(int32_t)(fabsl((long double)a)/16384.0L);
  for(int delta=-1;delta<=1;delta++) {
   check(a,threshold+delta);check(a,-(threshold+delta));count+=2;
  }
 }
 printf("%u fixed multiplication/division pairs passed\\n",count);
}
'''
    if args.bits32:body='_Static_assert(sizeof(void*)==4,"actual 32-bit process required");\n'+body
    test=root/'test.c';test.write_text(body);binary=root/'test'
    flags=['-m32','-fsanitize=undefined','-fsanitize-undefined-trap-on-error'] if args.bits32 else ['-fsanitize=address,undefined']
    subprocess.run([args.cc,'-std=c11','-O1','-g','-Wall','-Wextra','-Werror',*flags,'-iquote',str(project/'engines/doom/upstream'),str(test),'-lm','-o',str(binary)],check=True)
    if args.bits32:assert binary.read_bytes()[:5]==b'\x7fELF\x01'
    env=dict(os.environ,ASAN_OPTIONS='detect_leaks=1',UBSAN_OPTIONS='halt_on_error=1')
    if args.runtime_libraries:env['LD_LIBRARY_PATH']=str(args.runtime_libraries.resolve())
    subprocess.run([str(binary)],env=env,check=True)
