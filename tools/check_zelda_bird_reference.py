"""Verify original bird selection/cancel dispatch with synthetic RAM and drawing hooks."""
from pathlib import Path
import argparse,os,subprocess,tempfile
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--32-bit',dest='bits32',action='store_true')
parser.add_argument('--cc',default='cc')
parser.add_argument('--runtime-libraries',type=Path)
args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
source=(project/'engines/zelda3/upstream/src/messaging.c').read_text()
def function(signature):
    start=source.index(signature);brace=source.index('{',start);depth=1;end=brace+1
    while depth:
        if source[end]=='{':depth+=1
        if source[end]=='}':depth-=1
        end+=1
    return source[start:end]
constants='\n'.join(line for line in source.splitlines() if line.startswith('static const uint8 kBirdTravel_'))
assert len(constants.splitlines())==5
body='''#include <assert.h>
#include <stdio.h>
#include <string.h>
#include "types.h"
#include "features.h"
#include "variables.h"
_Alignas(uint32) uint8 g_ram[131072];
static unsigned calls,draws,visibility,events[8],event_count,transport_calls,matching;
static uint16 expected_x[9],expected_y[9],destination;
static bool WorldMap_CalculateOamCoordinates(Point16U *point){
 assert(calls<9);
 assert(link_x_coord_spexit==expected_x[calls] && link_y_coord_spexit==expected_y[calls]);
 point->x=link_x_coord_spexit%200+8;point->y=link_y_coord_spexit%160+8;
 return (visibility&(1u<<calls++))!=0;
}
static void WorldMap_AddSprite(int spr,uint8 big,uint8 flags,uint8 ch,uint16 x,uint16 y){
 unsigned index=calls-1;
 assert(x==expected_x[index]%200+8-(spr==16?4:0));
 assert(y==expected_y[index]%160+8-(spr==16?4:0));
 assert(big==(spr==16?2:0));
 if(spr==16)assert(flags==0x3e && ch==0);
 else {assert(spr>=0&&spr<8);assert(spr==(int)((frame_counter&16?8:7)-index));static const uint8 chars[8]={0x7f,0x79,0x6c,0x6d,0x6e,0x6f,0x7c,0x7d};assert(ch==chars[spr]);assert(flags==((unsigned)spr==birdtravel_var1[0]?0x30+(frame_counter&6):0x32));}
 draws++;
}
static void event(unsigned value){assert(event_count<8);events[event_count++]=value;}
static void FluteMenu_LoadTransport(void){transport_calls++;event(1);overworld_screen_index=destination;}
static void FluteMenu_LoadSelectedScreenPalettes(void){event(2);}
static void DecompressAnimatedOverworldTiles(int tile){unsigned t=overworld_screen_index&0xbf;assert(tile==((t==3||t==5||t==7)?0x58:0x5a));event(3);}
static void Overworld_SetFixedColAndScroll(void){event(4);}
static void InitializeTilesets(void){event(5);}
static void Overworld_LoadOverlays2(void){event(6);}
static bool ZeldaIsPlayingMusicTrack(uint8 track){assert(track==(overworld_music[(uint8)overworld_screen_index]&15));event(7);return matching;}
'''+constants+'\n'+function('void FluteMenu_HandleSelection() {')+'\n'+function('void FluteMenu_LoadSelectedScreen() {')+'''
static void setup_points(unsigned frame){
 calls=draws=0;frame_counter=frame;link_x_coord_spexit=0x3456;link_y_coord_spexit=0x6789;
 unsigned n=0;
 if(frame&16){expected_x[n]=0x3456;expected_y[n++]=0x6789;}
 for(int i=7;i>=0;i--){expected_x[n]=(uint16)(kBirdTravel_x_hi[i]*256+kBirdTravel_x_lo[i]);expected_y[n++]=(uint16)(kBirdTravel_y_hi[i]*256+kBirdTravel_y_lo[i]);}
 memset(bird_travel_x_lo,0xa5,8);
}
static void acceptance(void){
 const unsigned delays[]={0,1,16,64,255};unsigned cases=0;
 for(unsigned enabled=0;enabled<2;enabled++)for(unsigned d=0;d<5;d++)for(unsigned buttons=0;buttons<65536;buttons++){
  setup_points(0);visibility=0;some_menu_ctr=delays[d];
  enhanced_features0=enabled?kFeatures0_CancelBirdTravel:0;
  joypad1L_last=buttons&255;joypad1H_last=buttons>>8;filtered_joypad_H=0;
  birdtravel_var1[0]=5;overworld_map_state=6;sound_effect_2=7;
  int accepted=!delays[d]&&(((buttons&255)|(buttons>>8))&0xc0);
  FluteMenu_HandleSelection();
  assert(overworld_map_state==(accepted?7:6));
  assert(some_menu_ctr==(accepted?(enabled?(buttons&255):0):(delays[d]?delays[d]-1:0)));
  assert(calls==(accepted?0:8) && draws==0);
  assert(link_x_coord_spexit==0x3456 && link_y_coord_spexit==0x6789);
  assert(birdtravel_var1[0]==5 && sound_effect_2==7);
  for(unsigned i=0;i<8;i++)assert(bird_travel_x_lo[i]==(accepted?0xa5:kBirdTravel_x_lo[i]));
  cases++;
 }
 printf("%u original bird-menu acceptance/debounce cases passed\\n",cases);
}
static void navigation(void){
 const unsigned masks[]={0,511,1,0x155};unsigned cases=0;
 for(unsigned selected=0;selected<8;selected++)for(unsigned input=0;input<16;input++)
 for(unsigned frame=0;frame<32;frame++)for(unsigned v=0;v<4;v++)for(unsigned delay=0;delay<2;delay++){
  setup_points(frame);visibility=masks[v];some_menu_ctr=delay?2:0;
  joypad1L_last=joypad1H_last=0;filtered_joypad_H=input;
  enhanced_features0=kFeatures0_CancelBirdTravel;birdtravel_var1[0]=selected;sound_effect_2=7;overworld_map_state=6;
  int expected=(int)selected;if(input&10)expected--;if(input&5)expected++;expected=(expected+8)%8;
  FluteMenu_HandleSelection();
  assert(birdtravel_var1[0]==expected);assert(sound_effect_2==((input&15)?32:7));
  assert(some_menu_ctr==(delay?1:0) && overworld_map_state==6);
  unsigned count=(frame&16)?9:8,visible=0;
  for(unsigned i=0;i<count;i++)visible+=(visibility>>i)&1;
  assert(calls==count && draws==visible);
  assert(link_x_coord_spexit==0x3456 && link_y_coord_spexit==0x6789);
  for(unsigned i=0;i<8;i++)assert(bird_travel_y_hi[i]==kBirdTravel_y_hi[i]);
  cases++;
 }
 printf("%u original destination/marker/coordinate-restore cases passed\\n",cases);
}
static void load_dispatch(void){
 unsigned cases=0;
 for(unsigned i=0;i<256;i++)overworld_music[i]=(uint8)(i*7+0x35);
 for(unsigned counter=0;counter<256;counter++)for(unsigned screen=0;screen<256;screen++)for(unsigned same=0;same<2;same++){
  some_menu_ctr=counter;overworld_screen_index=screen;destination=screen^0x41;matching=same;
  event_count=transport_calls=0;overworld_map_state=7;submodule_index=8;
  save_ow_event_info[0x3b]=save_ow_event_info[0x7b]=255;
  save_dung_info[267]=save_dung_info[40]=65535;
  overworld_palette_aux_or_main=0xabcd;hud_palette=7;dung_draw_width_indicator=0xabcd;
  FluteMenu_LoadSelectedScreen();
  unsigned cancel=(counter&0x40)!=0,final_screen=cancel?screen:destination;
  assert(transport_calls==!cancel && overworld_screen_index==final_screen);
  unsigned offset=0;if(!cancel)assert(events[offset++]==1);
  for(unsigned e=2;e<=7;e++)assert(events[offset++]==e);
  assert(event_count==offset);
  assert(save_ow_event_info[0x3b]==0xdf && save_ow_event_info[0x7b]==0xdf);
  assert(save_dung_info[267]==0xff7f && save_dung_info[40]==0xfeff);
  assert(overworld_palette_aux_or_main==0 && hud_palette==0 && dung_draw_width_indicator==0xab00);
  assert(overworld_map_state==8 && submodule_index==7 && sound_effect_2==16);
  unsigned music=overworld_music[final_screen];
  assert(sound_effect_ambient==music/16 && music_control==(same?0xf3:music%16));
  cases++;
 }
 printf("%u original cancel/transport/palette/music cleanup cases passed\\n",cases);
}
int main(void){acceptance();navigation();load_dispatch();}
'''
if args.bits32:body='_Static_assert(sizeof(void*)==4,"actual 32-bit process required");\n'+body
with tempfile.TemporaryDirectory(prefix='gamepkg-bird-ref-') as temporary:
    root=Path(temporary);test=root/'bird.c';binary=root/'bird';test.write_text(body)
    flags=['-m32','-fsanitize=undefined','-fsanitize-undefined-trap-on-error'] if args.bits32 else ['-fsanitize=address,undefined']
    subprocess.run([args.cc,'-O1','-std=c11','-Wall','-Wextra','-Werror',*flags,'-iquote',str(project/'engines/zelda3/upstream/src'),str(test),'-o',str(binary)],check=True)
    if args.bits32:assert binary.read_bytes()[:5]==b'\x7fELF\x01'
    environment=dict(os.environ,ASAN_OPTIONS='detect_leaks=1',UBSAN_OPTIONS='halt_on_error=1')
    if args.runtime_libraries:environment['LD_LIBRARY_PATH']=str(args.runtime_libraries.resolve())
    subprocess.run([str(binary)],check=True,env=environment)
