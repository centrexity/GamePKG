"""Exercise original map vertex loading and every staged map fixed-coordinate expression."""
from pathlib import Path
import argparse,json,os,re,subprocess,tempfile
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--upstream',action='store_true')
parser.add_argument('--32-bit',dest='bits32',action='store_true')
parser.add_argument('--cc',default='cc')
parser.add_argument('--runtime-libraries',type=Path)
args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
source=(project/'engines/doom/upstream/p_setup.c').read_text()
if not args.upstream:
    for patch in json.loads((project/'engines/doom/adapters/map-patches.json').read_text()):
        assert source.count(patch['before'])==patch['count']==1
        source=source.replace(patch['before'],patch['after'])
start=source.index('void P_LoadVertexes (int lump)')
end=source.index('\n}',start)+2
function=source[start:end]
expressions=[]
for line in source.splitlines():
    if re.search(r'= .*SHORT\(.*(?:FRACBITS|65536|<<\s*16)',line) or re.search(r'bmaporg[xy] =',line):
        rhs=line.split('=',1)[1].strip().removesuffix(';')
        rhs=re.sub(r'SHORT\([^)]*\)', 'SHORT(disk)',rhs)
        rhs=re.sub(r'blockmaplump\[[01]\]', 'native',rhs)
        expressions.append((rhs,'li->angle' in line))
assert len(expressions)==15,len(expressions)
with tempfile.TemporaryDirectory(prefix='gamepkg-doom-map-') as temporary:
    root=Path(temporary)
    for big in (0,1):
        checks='\n'.join(('assert((uint32_t)('+expr+')==(uint32_t)((uint64_t)(uint32_t)value*65536u));' if angle else 'assert((int64_t)('+expr+')==(int64_t)value*65536);') for expr,angle in expressions)
        body='''#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <assert.h>
typedef uint8_t byte;
typedef struct {int16_t x,y;} mapvertex_t;
typedef struct {int32_t x,y;} vertex_t;
static mapvertex_t input[65536];
static vertex_t output[65536];
static vertex_t *vertexes;
static int numvertexes;
#define PU_LEVEL 0
#define PU_STATIC 0
#define FRACBITS 16
static int16_t read_short(int16_t x) {
    uint16_t n=(uint16_t)x;
    if(MODEL_BIG)n=(uint16_t)((n<<8)|(n>>8));
    return (int16_t)n;
}
#define SHORT(x) read_short(x)
static int W_LumpLength(int lump) {(void)lump;return sizeof(input);}
static void *Z_Malloc(size_t n,int tag,void *owner) {(void)tag;(void)owner;assert(n==sizeof(output));return output;}
static void *W_CacheLumpNum(int lump,int tag) {(void)lump;(void)tag;return input;}
static void W_ReleaseLumpNum(int lump) {(void)lump;}
'''+function+'''
int main(void) {
    for(int value=-32768;value<=32767;value++) {
        uint16_t bits=(uint16_t)value;
        int16_t disk=(int16_t)(MODEL_BIG?(uint16_t)((bits<<8)|(bits>>8)):bits);
        int16_t native=(int16_t)value;
        input[value+32768]=(mapvertex_t){disk,disk};
        CHECKS
    }
    P_LoadVertexes(0);
    assert(numvertexes==65536);
    for(int value=-32768;value<=32767;value++) {
        assert((int64_t)vertexes[value+32768].x==(int64_t)value*65536);
        assert((int64_t)vertexes[value+32768].y==(int64_t)value*65536);
    }
    puts("65,536 original vertices and 983,040 map expressions passed");
}
'''
        body=body.replace('MODEL_BIG',str(big)).replace('CHECKS',checks)
        if args.bits32:body='_Static_assert(sizeof(void*)==4,"actual 32-bit pointers required");\n'+body
        test=root/'test.c';test.write_text(body);binary=root/'test'
        flags=['-m32','-fsanitize=undefined','-fsanitize-undefined-trap-on-error'] if args.bits32 else ['-fsanitize=address,undefined']
        subprocess.run([args.cc,'-std=c11','-O1','-g','-Wall','-Wextra','-Werror',*flags,str(test),'-o',str(binary)],check=True)
        if args.bits32:assert binary.read_bytes()[:5]==b'\x7fELF\x01'
        env=dict(os.environ,ASAN_OPTIONS='detect_leaks=1',UBSAN_OPTIONS='halt_on_error=1')
        if args.runtime_libraries:env['LD_LIBRARY_PATH']=str(args.runtime_libraries.resolve())
        subprocess.run([str(binary)],env=env,check=True)
