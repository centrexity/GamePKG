"""Qualify original shoulder cycling and currency branches with synthetic RAM only."""
from pathlib import Path
import argparse,os,subprocess,tempfile
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--32-bit',dest='bits32',action='store_true')
parser.add_argument('--cc',default='cc')
parser.add_argument('--runtime-libraries',type=Path)
args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
source=(project/'engines/zelda3/upstream/src/hud.c').read_text()
def function(signature):
    start=source.index(signature);brace=source.index('{',start);depth=1;end=brace+1
    while depth:
        if source[end]=='{':depth+=1
        if source[end]=='}':depth-=1
        end+=1
    return source[start:end]
constants=source[source.index('enum {'):source.index('};')+2]
runtime=(project/'engines/zelda3/upstream/src/zelda_rtl.h').read_text()
button_start=runtime.index('enum {\n  kJoypadL_A')
buttons=runtime[button_start:runtime.index('};',button_start)+2]
currency_start=source.index('  uint16 a = link_rupees_actual;')
currency_end=source.index('  if (link_bomb_filler)',currency_start)
body='''#include <assert.h>
#include <stdio.h>
#include <string.h>
#include "hud.h"
#include "features.h"
#include "variables.h"
_Alignas(uint32) uint8 g_ram[131072];
static unsigned equipped_updates,box_updates;
void Hud_UpdateEquippedItem(void){equipped_updates++;}
static void Hud_UpdateItemBox(void){box_updates++;}
'''+constants+'\n'+buttons+'\n'
for signature in ['static int Hud_GetItemPosition(int item) {','static void Hud_GotoPrevItem(uint8 *item, uint8 first_item_index) {','static void Hud_GotoNextItem(uint8 *item, uint8 first_item_index) {','static bool Hud_DoWeHaveThisItem(uint8 item) {','void Hud_HandleItemSwitchInputs() {','static int MaxRupees() {']:
    body+=function(signature)+'\n'
body+='static void currency(void){\n'+source[currency_start:currency_end]+'\n}\n'
body+='''
static uint32 ownership(unsigned pattern) {
 if(pattern==0)return 0;
 if(pattern==1)return 0xfffff;
 if(pattern<=21)return 1u<<(pattern-2);
 if(pattern==22)return 0x55555;
 if(pattern==23)return 0xaaaaa;
 if(pattern==24)return 15;
 if(pattern==25)return 0xf0000;
 return (0x9e3779b9u*pattern)&0xfffff;
}
static void inventory_cases(void){
 assert(kHudItemCount==20 && kNewStyleInventory==0);
 unsigned cases=0;
 for(unsigned order=0;order<4;order++)for(unsigned pattern=0;pattern<32;pattern++)
 for(unsigned current=0;current<=20;current++)for(unsigned pressed=0;pressed<4;pressed++)
 for(unsigned held=0;held<4;held++)for(unsigned flags=0;flags<4;flags++){
  unsigned grid[20];
  for(unsigned pos=0;pos<20;pos++)grid[pos]=order==1?20-pos:order==2?(pos+7)%20+1:order==3?(pos*7)%20+1:pos+1;
  memset(hud_inventory_order,0,24);
  if(order)for(unsigned pos=0;pos<20;pos++)hud_inventory_order[pos]=grid[pos];
  uint32 owned=ownership(pattern);
  for(unsigned item=1;item<=20;item++)(&link_item_bow)[item-1]=(owned&(1u<<(item-1)))?item%3+1:0;
  hud_cur_item=current;hud_cur_item_l=held&1?11:0;hud_cur_item_r=held&2?12:0;
  filtered_joypad_L=(pressed&1?kJoypadL_L:0)|(pressed&2?kJoypadL_R:0);
  enhanced_features0=kFeatures0_CarryMoreRupees|(flags&1?kFeatures0_SwitchLR:0)|(flags&2?kFeatures0_SwitchLRLimit:0);
  sound_effect_2=7;flag_update_hud_in_nmi=255;equipped_updates=box_updates=0;
  int direction=0;
  if(flags&1){
   if((pressed&1)&&!(held&1))direction=held&2?1:-1;
   else if((pressed&2)&&!(held&2))direction=1;
  }
  unsigned expected=current;
  if(direction){
   int origin=direction>0?-1:0;
   if(current)for(unsigned pos=0;pos<20;pos++)if(grid[pos]==current)origin=pos;
   for(int distance=1;distance<=20;distance++){
    unsigned pos=(origin+direction*distance+40)%20,item=grid[pos];
    if((owned&(1u<<(item-1)))&&(!(flags&2)||pos<4)){expected=item;break;}
   }
  }
  Hud_HandleItemSwitchInputs();
  unsigned changed=expected!=current;
  assert(hud_cur_item==expected);
  assert(sound_effect_2==(changed?32:7));
  assert(flag_update_hud_in_nmi==(changed?0:255));
  assert(equipped_updates==changed && box_updates==changed);
  cases++;
 }
 printf("%u original inventory selection/notification cases passed\\n",cases);
}
static void currency_cases(void){
 unsigned cases=0;
 const unsigned goals[]={0,1,998,999,1000,9998,9999,10000,10002};
 const unsigned delays[]={0,1,7,8,255};
 for(unsigned large=0;large<2;large++)for(unsigned actual=0;actual<=10001;actual++)
 for(unsigned g=0;g<sizeof(goals)/sizeof(*goals);g++)for(unsigned d=0;d<5;d++)for(unsigned busy=0;busy<2;busy++){
  enhanced_features0=large?kFeatures0_CarryMoreRupees:0;
  unsigned limit=large?9999:999,expected_actual=actual,expected_goal=goals[g];
  if(actual<goals[g]){expected_actual=actual+1;if(expected_actual>limit)expected_actual=expected_goal=limit;}
  else if(actual>goals[g])expected_actual=actual-1;
  unsigned sound=busy?7:0,delay=delays[d];
  if(actual==goals[g]||busy)delay=0;
  else{if(!(delay%8))sound=41;delay=(delay+1)%256;}
  link_rupees_actual=actual;link_rupees_goal=goals[g];
  rupee_sfx_sound_delay=delays[d];sound_effect_1=busy?7:0;
  assert(MaxRupees()==(int)limit);currency();
  assert(link_rupees_actual==expected_actual && link_rupees_goal==expected_goal);
  assert(rupee_sfx_sound_delay==delay && sound_effect_1==sound);
  cases++;
 }
 printf("%u original currency increment/cap/sound cases passed\\n",cases);
}
int main(void){inventory_cases();currency_cases();}
'''
if args.bits32:body='_Static_assert(sizeof(void*)==4,"actual 32-bit process required");\n'+body
with tempfile.TemporaryDirectory(prefix='gamepkg-inventory-ref-') as temporary:
    root=Path(temporary);test=root/'inventory.c';binary=root/'inventory';test.write_text(body)
    flags=['-m32','-fsanitize=undefined','-fsanitize-undefined-trap-on-error'] if args.bits32 else ['-fsanitize=address,undefined']
    subprocess.run([args.cc,'-O1','-std=c11','-Wall','-Wextra','-Werror',*flags,'-iquote',str(project/'engines/zelda3/upstream/src'),str(test),'-o',str(binary)],check=True)
    if args.bits32:assert binary.read_bytes()[:5]==b'\x7fELF\x01'
    environment=dict(os.environ,ASAN_OPTIONS='detect_leaks=1',UBSAN_OPTIONS='halt_on_error=1')
    if args.runtime_libraries:environment['LD_LIBRARY_PATH']=str(args.runtime_libraries.resolve())
    subprocess.run([str(binary)],check=True,env=environment)
