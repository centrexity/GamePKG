"""Exercise the unchanged original low-health HUD branch with the optional beep flag."""
from pathlib import Path
import os,subprocess,tempfile
import argparse
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--32-bit',dest='bits32',action='store_true')
parser.add_argument('--cc',default='cc')
parser.add_argument('--runtime-libraries',type=Path)
args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
source=(project/'engines/zelda3/upstream/src/hud.c').read_text()
threshold=next(line for line in source.splitlines() if line.startswith('static const uint8 kMaxHealthForLevel[]'))
start=source.index('  if (!flag_is_link_immobilized && !link_hearts_filler &&')
brace=source.index('{',start);depth=1;end=brace+1
while depth:
    if source[end]=='{':depth+=1
    if source[end]=='}':depth-=1
    end+=1
branch=source[start:end]
body='''#include <assert.h>
#include <stdio.h>
#include "features.h"
static uint8 flag_is_link_immobilized,link_hearts_filler,link_health_current,link_health_capacity;
static uint8 link_lowlife_countdown_timer_beep,sound_effect_1;
_Alignas(uint32) static uint8 g_ram[0x700];
'''+threshold+'\nstatic void original(void){\n'+branch+'\n}\n'+'''
int main(void){
 unsigned cases=0;
 const unsigned timers[]={0,1,31,255};
 for(unsigned capacity=0;capacity<=20;capacity++)for(unsigned health=0;health<=255;health++)
 for(unsigned t=0;t<4;t++)for(unsigned immobile=0;immobile<2;immobile++)
 for(unsigned filler=0;filler<2;filler++)for(unsigned busy=0;busy<2;busy++) {
  unsigned emitted[2],next[2];
  for(unsigned disabled=0;disabled<2;disabled++) {
   flag_is_link_immobilized=immobile;link_hearts_filler=filler;
   link_health_current=health;link_health_capacity=capacity*8;
   link_lowlife_countdown_timer_beep=timers[t];sound_effect_1=busy?7:0;
   enhanced_features0=kFeatures0_TurnWhileDashing|(disabled?kFeatures0_DisableLowHealthBeep:0);
   original();emitted[disabled]=sound_effect_1;next[disabled]=link_lowlife_countdown_timer_beep;
  }
  assert(next[0]==next[1]);
  int should_beep=!immobile&&!filler&&health<kMaxHealthForLevel[capacity]&&!timers[t]&&!busy;
  assert(emitted[0]==(should_beep?43u:(busy?7u:0u)));
  assert(emitted[1]==(busy?7u:0u));
  cases++;
 }
 printf("%u original low-health states preserve timing and suppress only the optional beep\\n",cases);
}
'''
with tempfile.TemporaryDirectory(prefix='gamepkg-health-ref-') as temporary:
    if args.bits32:body='_Static_assert(sizeof(void*)==4, "actual 32-bit process required");\n'+body
    root=Path(temporary);test=root/'health.c';binary=root/'health';test.write_text(body)
    subprocess.run([args.cc,'-O1','-std=c11','-Wall','-Wextra','-Werror',*(['-m32','-fsanitize=undefined','-fsanitize-undefined-trap-on-error'] if args.bits32 else ['-fsanitize=address,undefined']),'-iquote',str(project/'engines/zelda3/upstream/src'),str(test),'-o',str(binary)],check=True)
    if args.bits32:assert binary.read_bytes()[:5]==b'\x7fELF\x01'
    environment=dict(os.environ,ASAN_OPTIONS='detect_leaks=1',UBSAN_OPTIONS='halt_on_error=1')
    if args.runtime_libraries:environment['LD_LIBRARY_PATH']=str(args.runtime_libraries.resolve())
    subprocess.run([str(binary)],check=True,env=environment)
