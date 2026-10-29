"""Exercise the actual pinned palette converter with both processor byte orders."""
from pathlib import Path
import json, os, subprocess, tempfile
project = Path(__file__).resolve().parents[1]
source = (project/'engines/doom/upstream/i_video.c').read_text()
for patch in json.loads((project/'engines/doom/adapters/video-patches.json').read_text()):
    assert source.count(patch['before']) == patch['count'] == 1
    source = source.replace(patch['before'], patch['after'])
assert '#include "i_swap.h"' in source
begin = source.index('void cmap_to_fb(')
end = source.index('\nvoid I_InitGraphics', begin)
function = source[begin:end]
with tempfile.TemporaryDirectory(prefix='gamepkg-doom-pixels-') as temporary:
    root = Path(temporary)
    header=(project/'engines/doom/upstream/i_swap.h').read_text()
    for patch in json.loads((project/'engines/doom/adapters/endian-patches.json').read_text()):
        assert header.count(patch['before'])==patch['count']==1
        header=header.replace(patch['before'],patch['after'])
    (root/'i_swap.h').write_text(header)
    for big in (0, 1):
        test = root/f'pixels{big}.c'
        test.write_text('''#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#undef __BYTE_ORDER__
#define __BYTE_ORDER__ ''' + ('__ORDER_BIG_ENDIAN__' if big else '__ORDER_LITTLE_ENDIAN__') + '''
#include "i_swap.h"
struct color { uint8_t r,g,b; } colors[256];
struct { int bits_per_pixel; struct { int offset; } red,green,blue; } s_Fb;
static int fb_scaling;
#define I_Error(...) abort()
''' + function + '''
int main(void) {
    s_Fb.red.offset=16; s_Fb.green.offset=8; s_Fb.blue.offset=0;
    for(unsigned a=0;a<256;a++) for(unsigned b=0;b<256;b++)
    for(unsigned arrangement=0;arrangement<3;arrangement++) {
        uint8_t rgb[3]={(uint8_t)a,(uint8_t)b,(uint8_t)(a^b)};
        colors[0]=(struct color){rgb[arrangement],rgb[(arrangement+1)%3],rgb[(arrangement+2)%3]};
        for(int bits=16;bits<=32;bits+=16) for(int scale=1;scale<=4;scale++) {
            union { uint32_t align; uint8_t bytes[80]; } buffer;
            memset(buffer.bytes,0xa5,sizeof buffer.bytes);
            uint8_t input=0; s_Fb.bits_per_pixel=bits; fb_scaling=scale;
            cmap_to_fb(buffer.bytes+8,&input,1);
            uint32_t expected=bits==32 ? ((uint32_t)colors[0].r<<16)|((uint32_t)colors[0].g<<8)|colors[0].b
                : ((colors[0].r&248u)<<8)|((colors[0].g&252u)<<3)|(colors[0].b>>3);
            unsigned width=bits/8;
            for(int pixel=0;pixel<scale;pixel++) {
                uint32_t stored=0;
                if(bits==16) {uint16_t value;memcpy(&value,buffer.bytes+8+pixel*width,2);stored=value;}
                else memcpy(&stored,buffer.bytes+8+pixel*width,4);
                for(unsigned byte=0;byte<width;byte++) {
                    unsigned shift=MODEL_BIG ? (width-1-byte)*8 : byte*8;
                    assert(((stored>>shift)&255)==((expected>>(byte*8))&255));
                }
            }
            for(unsigned i=0;i<8;i++)assert(buffer.bytes[i]==0xa5);
            for(unsigned i=8+scale*width;i<sizeof buffer.bytes;i++)assert(buffer.bytes[i]==0xa5);
        }
    }
    puts("1,572,864 palette/scaling cases passed");
}
'''.replace('MODEL_BIG', str(big)))
        binary=root/f'pixels{big}'
        subprocess.run(['cc','-std=c11','-O1','-Wall','-Wextra','-Werror','-fsanitize=address,undefined','-I',str(root),str(test),'-o',str(binary)],check=True)
        subprocess.run([str(binary)],check=True,env=dict(os.environ,ASAN_OPTIONS='detect_leaks=1',UBSAN_OPTIONS='halt_on_error=1'))
print('Actual DOOM palette conversion passed modeled little- and big-endian stores for RGB565 and 32-bit pixels.')
