"""Execute original sprite-header loading over the full signed offset range."""
from pathlib import Path
import argparse,json,os,subprocess,tempfile
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--upstream',action='store_true')
parser.add_argument('--32-bit',dest='bits32',action='store_true')
parser.add_argument('--cc',default='cc')
parser.add_argument('--runtime-libraries',type=Path)
args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
source=(project/'engines/doom/upstream/r_data.c').read_text()
if not args.upstream:
    for patch in json.loads((project/'engines/doom/adapters/sprite-patches.json').read_text()):
        assert source.count(patch['before'])==patch['count']==1
        source=source.replace(patch['before'],patch['after'])
start=source.index('void R_InitSpriteLumps (void)');end=source.index('\n}',start)+2
header=(project/'engines/doom/upstream/i_swap.h').read_text()
for patch in json.loads((project/'engines/doom/adapters/endian-patches.json').read_text()):
    assert header.count(patch['before'])==patch['count']==1
    header=header.replace(patch['before'],patch['after'])
with tempfile.TemporaryDirectory(prefix='gamepkg-doom-sprites-') as temporary:
    root=Path(temporary);(root/'i_swap.h').write_text(header)
    for big in (0,1):
        body='''#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <assert.h>
#undef __BYTE_ORDER__
#define __BYTE_ORDER__ MODEL_ORDER
#include "i_swap.h"
typedef struct {int16_t width,height,leftoffset,topoffset;int32_t columnofs[1];} patch_t;
static patch_t patches[65536];
static int firstspritelump,lastspritelump,numspritelumps;
static int32_t *spritewidth,*spriteoffset,*spritetopoffset;
static int allocations;
static int32_t buffers[3][65538];
#define PU_STATIC 0
#define PU_CACHE 0
#define FRACBITS 16
#define DEH_String(x) (x)
#define printf(...) ((void)0)
static int W_GetNumForName(const char *name){return !strcmp(name,"S_START")?0:65537;}
static void *Z_Malloc(size_t size,int tag,void *owner){
 (void)tag;(void)owner;assert(allocations<3);assert(size==65536*sizeof(int32_t));
 int32_t *buffer=buffers[allocations++];buffer[0]=12345;buffer[65537]=-54321;return buffer+1;
}
static void *W_CacheLumpNum(int lump,int tag){(void)tag;assert(lump>=1&&lump<=65536);return patches+lump-1;}
'''+source[start:end]+'''
static int16_t disk(int value){uint16_t n=(uint16_t)value;return (int16_t)(MODEL_BIG?(uint16_t)((n<<8)|(n>>8)):n);}
int main(void){
 for(int i=0;i<65536;i++)patches[i]=(patch_t){disk(i%32767+1),disk(1),disk(i-32768),disk(32767-i),{0}};
 R_InitSpriteLumps();assert(numspritelumps==65536);assert(allocations==3);
 for(int i=0;i<65536;i++){
  assert((int64_t)spritewidth[i]==(int64_t)(i%32767+1)*65536);
  assert((int64_t)spriteoffset[i]==(int64_t)(i-32768)*65536);
  assert((int64_t)spritetopoffset[i]==(int64_t)(32767-i)*65536);
 }
 for(int i=0;i<3;i++){assert(buffers[i][0]==12345);assert(buffers[i][65537]==-54321);}
 puts("65,536 original sprite headers: widths, signed horizontal/vertical offsets and array guards passed");
}
'''
        body=body.replace('MODEL_ORDER','__ORDER_BIG_ENDIAN__' if big else '__ORDER_LITTLE_ENDIAN__').replace('MODEL_BIG',str(big))
        if args.bits32:body='_Static_assert(sizeof(void*)==4,"actual 32-bit process required");\n'+body
        test=root/'test.c';test.write_text(body);binary=root/'test'
        flags=['-m32','-fsanitize=undefined','-fsanitize-undefined-trap-on-error'] if args.bits32 else ['-fsanitize=address,undefined']
        subprocess.run([args.cc,'-std=c11','-O1','-g','-Wall','-Wextra','-Werror',*flags,'-I',str(root),str(test),'-o',str(binary)],check=True)
        if args.bits32:assert binary.read_bytes()[:5]==b'\x7fELF\x01'
        env=dict(os.environ,ASAN_OPTIONS='detect_leaks=1',UBSAN_OPTIONS='halt_on_error=1')
        if args.runtime_libraries:env['LD_LIBRARY_PATH']=str(args.runtime_libraries.resolve())
        subprocess.run([str(binary)],env=env,check=True)
