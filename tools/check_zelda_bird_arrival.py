"""Exercise unchanged original bird arrival, allocation and fade completion."""
from pathlib import Path
import argparse, os, subprocess, tempfile
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--32-bit',dest='bits32',action='store_true')
parser.add_argument('--cc',default='cc')
parser.add_argument('--runtime-libraries',type=Path)
args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
upstream=project/'engines/zelda3/upstream/src'
def extract(file,signature):
    source=(upstream/file).read_text();start=source.index(signature);brace=source.index('{',start);end=brace+1;depth=1
    while depth:
        depth+=(source[end]=='{')-(source[end]=='}');end+=1
    return source[start:end]
body='''#include <assert.h>
#include <stdio.h>
#include <string.h>
#include "types.h"
#include "features.h"
#include "variables.h"
_Alignas(uint32) uint8 g_ram[131072];
static unsigned sprite_calls;
static void Sprite_Main(void){sprite_calls++;}
'''+extract('ancilla.c','static const uint8 kAncilla_Pflags[68] = {')+';\n'
for signature in ['void Ancilla_SetX(int k, uint16 x)', 'void Ancilla_SetY(int k, uint16 y)',
                  'void Ancilla_SetXY(int k, uint16 x, uint16 y)', 'int Ancilla_AllocInit(uint8 type, uint8 limit)',
                  'int Ancilla_AddAncilla(uint8 a, uint8 y)', 'bool AncillaAdd_CheckForPresence(uint8 a)',
                  'void AddBirdCommon(int k)', 'void AddBirdTravelSomething(uint8 a, uint8 y)']:
    body+=extract('ancilla.c',signature)+'\n'
for signature in ['void BirdTravel_Finish_Doit()', 'void FluteMenu_FadeInAndQuack()']:
    body+=extract('messaging.c',signature)+'\n'
body+='''
static void arrivals(void){
 const uint8 types[]={0,0x12,0x3c,0x13,0x0a,0x27};
 const uint16 coords[]={0,7,8,15,16,63,64,65535};
 unsigned cases=0;
 for(unsigned pattern=0;pattern<46656;pattern++)for(unsigned rotate=0;rotate<5;rotate++)
 for(unsigned wide=0;wide<2;wide++)for(unsigned edge=0;edge<8;edge++){
  memset(g_ram,0xa5,sizeof(g_ram));unsigned bits=pattern;int expected=-1;bool duplicate=false;
  uint8 old_types[6];
  for(unsigned k=0;k<6;k++){old_types[k]=ancilla_type[k]=types[bits%6];bits/=6;duplicate|=old_types[k]==0x27;}
  if(!duplicate){
   for(int k=4;k>=0;k--)if(!old_types[k]){expected=k;break;}
   if(expected<0){int k=rotate;do{if(--k<0)k=4;if(old_types[k]==0x3c || old_types[k]==0x13 || old_types[k]==0x0a){expected=k;break;}}while(k!=0);}
  }
  enhanced_features0=wide?kFeatures0_ExtendScreen64:0;ancilla_alloc_rotate=rotate;
  g_ram[kRam_BugsFixed]=kBugFix_PolyRenderer;
  BG2HOFS_copy2=coords[edge];link_y_coord=coords[7-edge];
  saved_module_for_menu=(uint8)pattern;mapbak_HDMAEN=(uint8)(pattern>>4);
  link_is_on_lower_level=1;link_is_on_lower_level_mirror=2;
  sprite_calls=0;
  BirdTravel_Finish_Doit();
  assert(sprite_calls==1 && overworld_map_state==0 && subsubmodule_index==0 && submodule_index==0);
  assert(main_module_index==(uint8)pattern && HDMAEN_copy==(uint8)(pattern>>4));
  for(int k=0;k<6;k++)assert(ancilla_type[k]==(k==expected?0x27:old_types[k]));
  if(expected<0){
   assert(link_player_handler_state==0xa5 && link_speed_setting==0xa5 && button_mask_b_y==0xa5);
   for(unsigned k=0;k<6;k++)assert(ancilla_L[k]==0xa5 && ancilla_x_lo[k]==0xa5 && ancilla_z[k]==0xa5);
  }else{
   int k=expected;
   assert(link_player_handler_state==0 && link_speed_setting==0 && button_mask_b_y==0x24);
   assert(button_b_frames==0 && link_delay_timer_spin_attack==0 && link_cant_change_direction==0xa4);
   assert(ancilla_floor[k]==1 && ancilla_floor2[k]==2 && ancilla_objprio[k]==0 && ancilla_U[k]==0);
   assert(ancilla_numspr[k]==0x1c && ancilla_L[k]==1 && ancilla_step[k]==2);
   assert(ancilla_z_vel[k]==(wide?58:40) && ancilla_z[k]==(wide?151:205));
   assert(ancilla_y_vel[k]==0 && ancilla_item_to_link[k]==0 && ancilla_aux_timer[k]==1);
   assert(ancilla_x_vel[k]==56 && ancilla_arr3[k]==3 && ancilla_K[k]==0 && ancilla_G[k]==0);
   uint16 x=(uint16)(coords[edge]-16-(wide?64:0)),y=(uint16)(coords[7-edge]-8);
   assert(ancilla_x_lo[k]==(uint8)x && ancilla_x_hi[k]==x/256);
   assert(ancilla_y_lo[k]==(uint8)y && ancilla_y_hi[k]==y/256);
   for(int j=0;j<6;j++)if(j!=k)assert(ancilla_L[j]==0xa5 && ancilla_x_lo[j]==0xa5);
  }
  if(!duplicate)assert((uint8)R14==5);
  cases++;
 }
 printf("%u original bird allocation/arrival/edge-position cases passed\\n",cases);
}
static void fades(void){
 for(unsigned brightness=0;brightness<256;brightness++){
  memset(g_ram,0,sizeof(g_ram));INIDISP_copy=brightness;overworld_map_state=9;
  saved_module_for_menu=7;main_module_index=14;ancilla_type[0]=0x27;sprite_calls=0;
  FluteMenu_FadeInAndQuack();
  assert(INIDISP_copy==(uint8)(brightness+1) && sprite_calls==1);
  assert(overworld_map_state==(brightness==14?0:9));
  assert(main_module_index==(brightness==14?7:14));
 }
 puts("256 original fade-completion states passed");
}
int main(void){arrivals();fades();}
'''
if args.bits32:body='_Static_assert(sizeof(void*)==4,"actual 32-bit process required");\n'+body
with tempfile.TemporaryDirectory(prefix='gamepkg-bird-arrival-') as temporary:
    root=Path(temporary);test=root/'arrival.c';binary=root/'arrival';test.write_text(body)
    flags=['-m32','-fsanitize=undefined','-fsanitize-undefined-trap-on-error'] if args.bits32 else ['-fsanitize=address,undefined']
    subprocess.run([args.cc,'-O1','-std=c11','-Wall','-Wextra','-Werror',*flags,'-iquote',str(upstream),str(test),'-o',str(binary)],check=True)
    if args.bits32:assert binary.read_bytes()[:5]==b'\x7fELF\x01'
    environment=dict(os.environ,ASAN_OPTIONS='detect_leaks=1',UBSAN_OPTIONS='halt_on_error=1')
    if args.runtime_libraries:environment['LD_LIBRARY_PATH']=str(args.runtime_libraries.resolve())
    subprocess.run([str(binary)],check=True,env=environment)
