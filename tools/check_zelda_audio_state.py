"""Compare non-MSU snapshot hooks using the original sound processor and DSP."""
from pathlib import Path
import argparse,os,subprocess,tempfile
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--32-bit',dest='bits32',action='store_true');parser.add_argument('--cc',default='cc');parser.add_argument('--runtime-libraries',type=Path)
args=parser.parse_args();project=Path(__file__).resolve().parents[1];upstream=project/'engines/zelda3/upstream'
def function(source,signature):
 start=source.index(signature);brace=source.index('{',start);end=brace+1;depth=1
 while depth:
  depth+=(source[end]=='{')-(source[end]=='}');end+=1
 return source[start:end]
original=(upstream/'src/audio.c').read_text();adapter=(upstream.parent/'adapters/engine.c').read_text()
restore=function(original,'void ZeldaRestoreMusicAfterLoad_Locked(').split('  MsuPlayer *mp =')[0]+'}\n'
restore=restore.replace('ZeldaRestoreMusicAfterLoad_Locked','original_restore')
save=function(original,'void ZeldaSaveMusicStateToRam_Locked(').split('  msu_volume =')[0]+'}\n'
save=save.replace('ZeldaSaveMusicStateToRam_Locked','original_save')
body='''#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "types.h"
#include "spc_player.h"
static struct {SpcPlayer *player;} g_zenv;
static struct {uint8 ports[4];} g_apu_write;
static uint8 ports[4];
'''+restore+save+function(adapter,'void ZeldaRestoreMusicAfterLoad_Locked(')+'\n'+function(adapter,'void ZeldaSaveMusicStateToRam_Locked(')+'''
static SpcPlayer *player(void){
 SpcPlayer *p=SpcPlayer_Create();Dsp *d=p->dsp;memset(p,0,sizeof(*p));p->dsp=d;
 SpcPlayer_Initialize(p);SpcPlayer_CopyVariablesToRam(p);return p;
}
static void equal_players(SpcPlayer *a,SpcPlayer *b){
 SpcPlayer ca=*a,cb=*b;ca.dsp=cb.dsp=NULL;
 assert(memcmp(&ca,&cb,sizeof(ca))==0);
}
int main(void){
 unsigned cases=0;const uint8 timers[]={0,1,31,63,64};
 for(unsigned value=0;value<256;value++)for(unsigned reset=0;reset<2;reset++)for(unsigned t=0;t<5;t++){
  SpcPlayer *a=player(),*b=player();
  for(unsigned i=0;i<4;i++){
   a->ram[0x410+i]=b->ram[0x410+i]=(uint8)(value+i*71);
   a->input_ports[i]=b->input_ports[i]=0xa5;
   g_apu_write.ports[i]=ports[i]=0x5a;
  }
  a->timer_cycles=b->timer_cycles=timers[t];
  g_zenv.player=a;original_restore(reset);
  g_zenv.player=b;ZeldaRestoreMusicAfterLoad_Locked(reset);
  equal_players(a,b);assert(memcmp(g_apu_write.ports,ports,4)==0);
  // Latest queued command bytes must survive even if not yet pushed to input_ports.
  for(unsigned i=0;i<4;i++)g_apu_write.ports[i]=ports[i]=(uint8)(value+i*37);
  g_zenv.player=a;original_save();g_zenv.player=b;ZeldaSaveMusicStateToRam_Locked();
  equal_players(a,b);assert(memcmp(b->ram+0x410,ports,4)==0);
  dsp_free(a->dsp);dsp_free(b->dsp);free(a);free(b);cases++;
 }
 printf("%u original/adapter non-MSU audio restore/save states passed\\n",cases);
}
'''
if args.bits32:body='_Static_assert(sizeof(void*)==4,"actual 32-bit process required");\n'+body
with tempfile.TemporaryDirectory(prefix='gamepkg-audio-state-') as temporary:
 root=Path(temporary);test=root/'state.c';binary=root/'state';test.write_text(body)
 flags=['-m32','-fsanitize=undefined','-fsanitize-undefined-trap-on-error'] if args.bits32 else ['-fsanitize=address,undefined']
 common=[args.cc,'-O1','-std=c11',*flags,'-iquote',str(upstream/'src'),'-iquote',str(upstream)]
 # Original complete translation units retain their existing warning policy;
 # the new adapter/reference harness is compiled with strict warnings.
 objects=[]
 for name in ['src/spc_player.c','snes/dsp.c']:
  obj=root/(Path(name).stem+'.o');objects.append(str(obj))
  subprocess.run([*common,'-c',str(upstream/name),'-o',str(obj)],check=True)
 subprocess.run([*common,'-Wall','-Wextra','-Werror',str(test),*objects,'-o',str(binary)],check=True)
 if args.bits32:assert binary.read_bytes()[:5]==b'\x7fELF\x01'
 environment=dict(os.environ,ASAN_OPTIONS='detect_leaks=1',UBSAN_OPTIONS='halt_on_error=1')
 if args.runtime_libraries:environment['LD_LIBRARY_PATH']=str(args.runtime_libraries.resolve())
 subprocess.run([str(binary)],check=True,env=environment)
