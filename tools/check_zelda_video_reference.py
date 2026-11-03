"""Compare all actual adapter presentation/enhancement flags with the original parser."""
from pathlib import Path
import os
import subprocess
import tempfile
import argparse
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--32-bit',dest='bits32',action='store_true')
parser.add_argument('--cc',default='cc')
parser.add_argument('--runtime-libraries',type=Path)
args=parser.parse_args()
project = Path(__file__).resolve().parents[1]
adapter = (project / 'engines/zelda3/adapters/engine.c').read_text()
function = adapter.split('int configure_video(', 1)[1].split('__attribute__((export_name("step")))', 1)[0]
original = (project / 'engines/zelda3/upstream/src/config.c').read_text()
config_source=original
original = original.split('      if (g_config.extended_aspect_ratio && !nospr)', 1)[1].split('      return true;', 1)[0]
original = 'if (g_config.extended_aspect_ratio && !nospr)' + original
ppu_header = (project / 'engines/zelda3/upstream/snes/ppu.h').read_text()
render_constants = 'enum {\n  kPpuRenderFlags_NewRenderer' + ppu_header.split('enum {\n  kPpuRenderFlags_NewRenderer',1)[1].split('};',1)[0] + '};\n'
main = (project / 'engines/zelda3/upstream/src/main.c').read_text()
render_original = 'g_ppu_render_flags = g_config.new_renderer' + main.split('g_ppu_render_flags = g_config.new_renderer',1)[1].split(';',1)[0] + ';'
source = '''#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include "features.h"
static bool initialized, stepped;
static uint32 video_width=256, video_height=224, video_flags;
static uint32 g_ppu_render_flags;
uint32 g_wanted_zelda_features;
static struct { int extraLeftRight; } ppu;
static struct { __typeof__(ppu) *ppu; } g_zenv={&ppu};
static struct { unsigned extended_aspect_ratio, features0; bool new_renderer, enhanced_mode7, extend_y, no_sprite_limits; } g_config;
'''
source += render_constants
source += 'int configure_video(' + function
enhancement_keys=['ItemSwitchLR', 'ItemSwitchLRLimit', 'TurnWhileDashing', 'MirrorToDarkworld', 'CollectItemsWithSword', 'BreakPotsWithSword', 'DisableLowHealthBeep', 'ShowMaxItemsInYellow', 'MoreActiveBombs', 'CarryMoreRupees', 'MiscBugFixes', 'GameChangingBugFixes', 'CancelBirdTravel']
expected_flags=[]
for key in enhancement_keys:
    match=__import__('re').search(r'StringEqualsNoCase\(key, "'+key+r'"\)\) \{\s*return ParseBoolBit\(value, &g_config.features0, (kFeatures0_\w+)\)',config_source)
    assert match,key
    expected_flags.append(match[1])
source += 'static unsigned original(unsigned extra, unsigned options) { bool nospr=false, novis=false; g_config.new_renderer=true; g_config.enhanced_mode7=false; g_config.extend_y=(options&1)!=0; g_config.no_sprite_limits=(options&2)!=0; g_config.extended_aspect_ratio=extra; g_config.features0=((options&4)?kFeatures0_DimFlashes:0)|((options&8)?kFeatures0_SkipIntroOnKeypress:0); ' + ''.join('if(options&(1u<<'+str(i+4)+'))g_config.features0|='+flag+';' for i,flag in enumerate(expected_flags)) + original + render_original + 'return g_config.features0;}\n'
source += '''int main(void) {
 assert(configure_video(0,0)==-1);
 initialized=true;
 for(unsigned extra=0;extra<=96;extra++) for(unsigned tall=0;tall<=0x1ffff;tall++) {
  g_wanted_zelda_features=0x8001ffffu;
  volatile unsigned requested_extra=extra, requested_options=tall;
  assert(configure_video(requested_extra,requested_options)==0);
  assert(ppu.extraLeftRight==(int)extra);
  assert(video_width==256+2*extra && video_height==((tall&1)?240u:224u));
  assert(g_wanted_zelda_features==(original(extra,tall)|0x80000000u));
  assert(video_flags==g_ppu_render_flags);
 }
 unsigned width=video_width, height=video_height, flags=g_wanted_zelda_features;
 assert(configure_video(97,0)==-1 && configure_video(0,0x20000)==-1);
 assert(video_width==width && video_height==height && g_wanted_zelda_features==flags);
 stepped=true; assert(configure_video(0,0)==-1);
 assert(video_width==width && video_height==height && g_wanted_zelda_features==flags);
}
'''
with tempfile.TemporaryDirectory(prefix='gamepkg-zelda-video-ref-') as temporary:
    if args.bits32:source='_Static_assert(sizeof(void*)==4, "actual 32-bit process required");\n'+source
    root=Path(temporary); path=root/'video.c'; path.write_text(source); binary=root/'video'
    subprocess.run([args.cc,'-O1','-std=gnu11','-Wall','-Wextra','-Werror',*(['-m32','-fsanitize=undefined','-fsanitize-undefined-trap-on-error'] if args.bits32 else ['-fsanitize=address,undefined']),'-iquote',str(project/'engines/zelda3/upstream/src'),str(path),'-o',str(binary)],check=True)
    if args.bits32:assert binary.read_bytes()[:5]==b'\x7fELF\x01'
    environment=dict(os.environ,ASAN_OPTIONS='detect_leaks=1',UBSAN_OPTIONS='halt_on_error=1')
    if args.runtime_libraries:environment['LD_LIBRARY_PATH']=str(args.runtime_libraries.resolve())
    subprocess.run([str(binary)],check=True,env=environment)
print('All 12,713,984 original presentation/enhancement configurations match visibility and renderer defaults; invalid/late changes preserve state.')
