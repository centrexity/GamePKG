"""Sanitizer checks of the pinned sprite evaluator and its unsigned plane reads."""
from pathlib import Path
import argparse,json,os,subprocess,tempfile
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--cc',default='cc')
parser.add_argument('--32-bit',dest='bits32',action='store_true')
parser.add_argument('--runtime-libraries',type=Path)
parser.add_argument('--upstream',action='store_true',help='reproduce the original high-plane signed-shift failure')
args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-zelda-sprites-') as temporary:
    root=Path(temporary);binary=root/'sprites';ppu=root/'ppu.c';test=root/'test.c'
    source=(project/'engines/zelda3/upstream/snes/ppu.c').read_text()
    if not args.upstream:
        patch=json.loads((project/'engines/zelda3/adapters/ppu-patches.json').read_text())
        assert patch['file']=='snes/ppu.c' and patch['count']==3
        for item,count in [(patch,3),(patch['unsigned_masks'],1),(patch['mosaic_spans'][0],2),(patch['mosaic_spans'][1],1)]:
            assert item['count']==count and source.count(item['before'])==count,'Pinned PPU adaptation anchor changed'
            source=source.replace(item['before'],item['after'])
    ppu.write_text(source)
    body=(project/'tests/zelda_sprite_limits.c').read_text()
    assert body.count('#include "engines/zelda3/upstream/snes/ppu.c"')==1
    if args.bits32:body='_Static_assert(sizeof(void*)==4, "requires actual 32-bit pointers");\n'+body
    test.write_text(body.replace('#include "engines/zelda3/upstream/snes/ppu.c"','#include "'+str(ppu)+'"'))
    sanitizers=['-m32','-fsanitize=undefined','-fsanitize-undefined-trap-on-error'] if args.bits32 else ['-fsanitize=address,undefined']
    subprocess.run([args.cc,'-std=c11','-O1','-g','-fno-strict-aliasing','-ffunction-sections','-fdata-sections',*sanitizers,'-iquote',str(project/'engines/zelda3/upstream/snes'),'-iquote',str(project/'engines/zelda3/upstream'),str(test),'-Wl,--gc-sections','-lm','-o',str(binary)],check=True)
    if args.bits32:assert binary.read_bytes()[:5]==b'\x7fELF\x01','not an ELF32 process'
    environment=dict(os.environ,ASAN_OPTIONS='detect_leaks=1',UBSAN_OPTIONS='halt_on_error=1')
    if args.runtime_libraries:environment['LD_LIBRARY_PATH']=str(args.runtime_libraries.resolve())
    subprocess.run([str(binary)],check=True,env=environment)
print('Pinned sprite evaluator: 16,512 evaluator cases, 66,048 sprite/blank scanlines and 2,048 background and 12,288 patterned mosaic scanlines passed.')
