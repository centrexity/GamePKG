"""Check signed disk-word conversion using actual pinned DOOM endian macros."""
from pathlib import Path
import argparse, json, os, subprocess, tempfile
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--upstream',action='store_true')
parser.add_argument('--32-bit',dest='bits32',action='store_true')
parser.add_argument('--cc',default='cc')
parser.add_argument('--runtime-libraries',type=Path)
args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
header=(project/'engines/doom/upstream/i_swap.h').read_text()
patch_file=project/'engines/doom/adapters/endian-patches.json'
if not args.upstream:
    for patch in json.loads(patch_file.read_text()):
        assert header.count(patch['before'])==patch['count']==1
        header=header.replace(patch['before'],patch['after'])
with tempfile.TemporaryDirectory(prefix='gamepkg-doom-words-') as temporary:
    root=Path(temporary);(root/'i_swap.h').write_text(header)
    for big in (0,1):
        test=root/'test.c';binary=root/'test'
        test.write_text('''#include <stdint.h>
#include <assert.h>
#include <stdio.h>
#undef __BYTE_ORDER__
#define __BYTE_ORDER__ '''+('__ORDER_BIG_ENDIAN__' if big else '__ORDER_LITTLE_ENDIAN__')+'''
#include "i_swap.h"
static uint32_t disk32(uint32_t value) {
    uint8_t bytes[4]; for(unsigned i=0;i<4;i++)bytes[i]=(uint8_t)(value>>(i*8));
    uint32_t result=0;for(unsigned i=0;i<4;i++)result|=(uint32_t)bytes[i]<<(MODEL_BIG?(3-i)*8:i*8);
    return result;
}
int main(void) {
    for(uint32_t value=0;value<65536;value++) {
        uint16_t disk=MODEL_BIG?(uint16_t)((value<<8)|(value>>8)):(uint16_t)value;
        int32_t signed_expected=value<32768?(int32_t)value:(int32_t)value-65536;
        assert(SHORT((int16_t)disk)==signed_expected);
        for(unsigned position=0;position<3;position++) {
            uint32_t word=(value<<(position*8))|0x80000000u;
            int32_t input=(int32_t)disk32(word);
            if((uint32_t)LONG(input)!=word) {
                fprintf(stderr,"signed disk word %08x converted to %08x (big=%d, long=%zu)\\n",word,(uint32_t)LONG(input),MODEL_BIG,sizeof(long));
                return 1;
            }
            assert((uint32_t)LONG(disk32(word))==word);
        }
    }
    const uint32_t edges[]={0,1,0x7fffffffu,0x80000000u,0xffffffffu,0x12345680u,0x80abcdefu};
    for(unsigned i=0;i<sizeof(edges)/sizeof(*edges);i++)assert((uint32_t)LONG((int32_t)disk32(edges[i]))==edges[i]);
    puts("65,536 signed shorts and 196,615 signed/unsigned word cases passed");
}
'''.replace('MODEL_BIG',str(big)))
        if args.bits32:
            test.write_text('_Static_assert(sizeof(void*)==4, "actual 32-bit process required");\n'+test.read_text())
        sanitizers=['-m32','-fsanitize=undefined','-fsanitize-undefined-trap-on-error'] if args.bits32 else ['-fsanitize=address,undefined']
        subprocess.run([args.cc,'-std=c11','-O1','-Wall','-Wextra','-Werror',*sanitizers,'-I',str(root),str(test),'-o',str(binary)],check=True)
        if args.bits32:assert binary.read_bytes()[:5]==b'\x7fELF\x01'
        environment=dict(os.environ,ASAN_OPTIONS='detect_leaks=1',UBSAN_OPTIONS='halt_on_error=1')
        if args.runtime_libraries:environment['LD_LIBRARY_PATH']=str(args.runtime_libraries.resolve())
        subprocess.run([str(binary)],check=True,env=environment)
