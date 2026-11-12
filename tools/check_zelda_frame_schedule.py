"""Compare the actual adapter step scheduler with the unchanged original branch."""
from pathlib import Path
import argparse,os,subprocess,tempfile
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--32-bit',dest='bits32',action='store_true');parser.add_argument('--cc',default='cc');parser.add_argument('--runtime-libraries',type=Path)
args=parser.parse_args();project=Path(__file__).resolve().parents[1];upstream=project/'engines/zelda3/upstream/src'
def function(source,signature):
 start=source.index(signature);brace=source.index('{',start);end=brace+1;depth=1
 while depth:
  depth+=(source[end]=='{')-(source[end]=='}');end+=1
 return source[start:end]
original=(upstream/'zelda_rtl.c').read_text()
start=original.index('  int run_what;',original.index('bool ZeldaRunFrame(int inputs)'))
end=original.index('\n  if (g_emu_runframe',start)
body='''#include <assert.h>
#include <stdio.h>
#include <string.h>
#include "types.h"
#include "features.h"
#include "variables.h"
_Alignas(uint32) uint8 g_ram[131072];
static bool initialized,stepped;
uint32 g_wanted_zelda_features;
static uint32 video_width=256,video_flags;
static uint8 framebuffer[4];static int16 samples[2];
static unsigned captured_run,captured_buttons,run_calls,push_calls,draw_calls,audio_calls;
static bool music;
static bool ZeldaIsMusicPlaying(void){return music;}
static void ZeldaRunFrameInternal(uint16 buttons,int what){captured_buttons=buttons;captured_run=what;run_calls++;}
static void ZeldaPushApuState(void){push_calls++;}
static void ZeldaDrawPpuFrame(uint8 *buffer,int pitch,uint32 flags){assert(buffer==framebuffer && pitch==1024 && flags==video_flags);draw_calls++;}
static void ZeldaRenderAudio(int16 *buffer,int count,int channels){assert(buffer==samples && count==800 && channels==2);audio_calls++;}
static void EmuSyncMemoryRegion(void *p,size_t n){assert(p==&g_ram[kRam_CrystalRotateCounter] && n==1);}
'''+function(original,'static int IncrementCrystalCountdown(')+'\nstatic int original_schedule(void){\n'+original[start:end]+'\nreturn run_what;\n}\n'
body+=function((project/'engines/zelda3/adapters/engine.c').read_text(),'int step(uint32 buttons)')
body+='''
int main(void){
 unsigned cases=0;initialized=true;
 const unsigned bugs[]={0,kBugFix_Latest,kBugFix_Latest+1};
 for(unsigned b=0;b<3;b++)for(unsigned ready=0;ready<2;ready++)for(unsigned active=0;active<2;active++)
 for(unsigned stack=0;stack<2;stack++)for(unsigned counter=0;counter<256;counter++)for(unsigned trigger=0;trigger<256;trigger++){
  animated_tile_data_src=ready?1:0;g_ram[kRam_BugsFixed]=ready&&bugs[b]<kBugFix_Latest?kBugFix_Latest:bugs[b];
  is_nmi_thread_active=active;thread_other_stack=stack?0x1f31:0x1000;virq_trigger=trigger;
  g_ram[kRam_CrystalRotateCounter]=counter;
  int expected=original_schedule();unsigned expected_counter=g_ram[kRam_CrystalRotateCounter];
  g_ram[kRam_BugsFixed]=bugs[b];g_ram[kRam_CrystalRotateCounter]=counter;
  enhanced_features0=0;g_wanted_zelda_features=0x123456;music=(counter&1)!=0;
  stepped=false;run_calls=push_calls=draw_calls=audio_calls=0;
  assert(step(0xffff)==0 && stepped);
  assert(captured_run==(unsigned)expected);
  assert(g_ram[kRam_CrystalRotateCounter]==expected_counter);
  assert(g_ram[kRam_BugsFixed]==(ready&&bugs[b]<kBugFix_Latest?kBugFix_Latest:bugs[b]));
  assert(enhanced_features0==(ready?g_wanted_zelda_features:0));
  assert(g_ram[kRam_APUI00]==music && captured_buttons==0xff0f);
  assert(run_calls==1 && push_calls==1 && draw_calls==1 && audio_calls==1);cases++;
 }
 initialized=false;stepped=false;run_calls=push_calls=draw_calls=audio_calls=0;
 assert(step(0)==-1 && !stepped && !run_calls && !push_calls && !draw_calls && !audio_calls);
 printf("%u original/adapter frame-scheduling cases passed\\n",cases);
}
'''
if args.bits32:body='_Static_assert(sizeof(void*)==4,"actual 32-bit process required");\n'+body
with tempfile.TemporaryDirectory(prefix='gamepkg-frame-schedule-') as temporary:
 root=Path(temporary);test=root/'schedule.c';binary=root/'schedule';test.write_text(body)
 flags=['-m32','-fsanitize=undefined','-fsanitize-undefined-trap-on-error'] if args.bits32 else ['-fsanitize=address,undefined']
 subprocess.run([args.cc,'-O1','-std=c11','-Wall','-Wextra','-Werror',*flags,'-iquote',str(upstream),str(test),'-o',str(binary)],check=True)
 if args.bits32:assert binary.read_bytes()[:5]==b'\x7fELF\x01'
 environment=dict(os.environ,ASAN_OPTIONS='detect_leaks=1',UBSAN_OPTIONS='halt_on_error=1')
 if args.runtime_libraries:environment['LD_LIBRARY_PATH']=str(args.runtime_libraries.resolve())
 subprocess.run([str(binary)],check=True,env=environment)
